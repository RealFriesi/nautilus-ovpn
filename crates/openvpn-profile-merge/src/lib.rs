use std::ffi::{CStr, CString, NulError};
use std::path::Path;
use std::ptr::NonNull;

#[allow(non_camel_case_types, non_snake_case, non_upper_case_globals)]
mod ffi {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

struct MergeResult(NonNull<ffi::OvpnProfileMergeResult>);

impl Drop for MergeResult {
    fn drop(&mut self) {
        unsafe { ffi::ovpn_profile_merge_result_free(self.0.as_ptr()) };
    }
}

/// Merge a profile and its referenced files using OpenVPN 3 Core.
///
/// OpenVPN's `FOLLOW_FULL` mode permits references outside `reference_dir`.
pub fn merge_profile(profile_content: &str, reference_dir: &Path) -> Result<String, String> {
    let profile_content = CString::new(profile_content).map_err(format_nul_error)?;
    let reference_dir = reference_dir
        .to_str()
        .ok_or_else(|| "profile reference directory is not valid UTF-8".to_string())?;
    let reference_dir = CString::new(reference_dir).map_err(format_nul_error)?;

    let result =
        unsafe { ffi::ovpn_profile_merge(profile_content.as_ptr(), reference_dir.as_ptr()) };
    let result = MergeResult(
        NonNull::new(result)
            .ok_or_else(|| "OpenVPN profile merge allocation failed".to_string())?,
    );

    let status = unsafe { read_result_string(ffi::ovpn_profile_merge_status(result.0.as_ptr()))? };
    if status != "MERGE_SUCCESS" {
        let error =
            unsafe { read_result_string(ffi::ovpn_profile_merge_error(result.0.as_ptr()))? };
        return Err(format!("{status}: {error}"));
    }

    unsafe { read_result_string(ffi::ovpn_profile_merge_content(result.0.as_ptr())) }
}

fn format_nul_error(error: NulError) -> String {
    format!(
        "profile input contains a NUL byte at {}",
        error.nul_position()
    )
}

unsafe fn read_result_string(value: *const std::os::raw::c_char) -> Result<String, String> {
    if value.is_null() {
        return Err("OpenVPN profile merge returned a null string".to_string());
    }
    CStr::from_ptr(value)
        .to_str()
        .map(str::to_owned)
        .map_err(|error| format!("OpenVPN profile merge returned invalid UTF-8: {error}"))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::merge_profile;

    #[test]
    fn inlines_references_and_keeps_existing_inline_blocks() {
        let temp = tempdir().unwrap();
        let profiles = temp.path().join("profiles");
        let shared = temp.path().join("shared");
        fs::create_dir_all(&profiles).unwrap();
        fs::create_dir_all(&shared).unwrap();
        fs::write(shared.join("ca.crt"), "certificate data\n").unwrap();
        fs::write(shared.join("ta.key"), "tls key data\n").unwrap();

        let output = merge_profile(
            "ca ../shared/ca.crt\ntls-auth ../shared/ta.key 1\n<cert>\nexisting certificate\n</cert>\n",
            &profiles,
        )
        .unwrap();

        assert!(output.contains("<ca>\ncertificate data\n</ca>"));
        assert!(output.contains("<tls-auth>\ntls key data\n</tls-auth>"));
        assert!(output.contains("key-direction 1"));
        assert!(output.contains("<cert>\nexisting certificate\n</cert>"));
    }

    #[test]
    fn reports_missing_referenced_file() {
        let temp = tempdir().unwrap();
        let error = merge_profile("ca missing.crt\n", temp.path()).unwrap_err();
        assert!(error.contains("MERGE_REF_FAIL"));
    }

    #[test]
    fn rejects_profile_larger_than_core_limit() {
        let temp = tempdir().unwrap();
        let profile = format!("#{}\n", "a".repeat(262_144));
        assert!(merge_profile(&profile, temp.path()).is_err());
    }
}
