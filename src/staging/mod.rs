mod parser;

use std::fmt::Write;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use gio::prelude::*;
use sha2::{Digest, Sha256};

/// Extensions of companion files that are copied alongside the `.ovpn` file.
const COMPANION_EXTENSIONS: &[&str] = &["ovpn", "crt", "key", "p12", "pem", "txt"];

pub struct StagedConfig {
    /// The staging directory, e.g. `/tmp/nm-ovpn-<uuid>`.
    pub dir: PathBuf,
    /// Full path to the patched `.ovpn` file inside `dir`.
    pub config_path: PathBuf,
    /// Whether the config contains `auth-user-pass` without a credential file.
    pub requires_credentials: bool,
    /// Stable ID derived from the selected configuration URI and contents.
    pub session_id: String,
    /// Human-readable name used for the terminal.
    pub display_name: String,
}

fn log_err(context: &str, err: &impl std::fmt::Display) {
    eprintln!("[nautilus-openvpn] {context}: {err}");
}

fn stable_session_id(source_uri: &str, content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(source_uri.as_bytes());
    hasher.update([0]);
    hasher.update(content.as_bytes());

    let mut id = String::with_capacity(64);
    for byte in hasher.finalize() {
        write!(&mut id, "{byte:02x}").expect("writing to a String cannot fail");
    }
    id
}

fn display_name(source_path: Option<&Path>, source_name: &str) -> String {
    let fallback = Path::new(source_name)
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or(source_name);

    let Some(path) = source_path else {
        return fallback.to_string();
    };
    let Some(parent) = path.parent() else {
        return fallback.to_string();
    };
    if parent == Path::new("/") {
        return fallback.to_string();
    }

    parent
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or(fallback)
        .to_string()
}

/// Copies `source_uri` and companion files into a local staging directory.
pub fn stage_ovpn_file(source_uri: &str) -> Result<StagedConfig, String> {
    let source_file = gio::File::for_uri(source_uri);
    let source_name = source_file
        .basename()
        .ok_or_else(|| "could not determine file name of selected .ovpn file".to_string())?;
    let source_name = source_name.to_string_lossy().to_string();

    let parent_dir = source_file
        .parent()
        .ok_or_else(|| "selected .ovpn file has no parent directory".to_string())?;

    let source_path = source_file.path();
    let display_name = display_name(source_path.as_deref(), &source_name);
    let original_content = source_file
        .load_contents(gio::Cancellable::NONE)
        .map_err(|e| format!("failed to read selected .ovpn file: {e}"))?
        .0;
    let original_content = String::from_utf8(original_content.to_vec())
        .map_err(|e| format!("selected .ovpn file is not valid UTF-8: {e}"))?;
    let session_id = stable_session_id(source_uri, &original_content);
    let staging_dir = PathBuf::from(format!("/tmp/nm-ovpn-{session_id}"));

    if staging_dir.exists() {
        fs::remove_dir_all(&staging_dir)
            .map_err(|e| format!("failed to clean staging dir {staging_dir:?}: {e}"))?;
    }
    fs::create_dir_all(&staging_dir)
        .map_err(|e| format!("failed to create staging dir {staging_dir:?}: {e}"))?;
    fs::set_permissions(&staging_dir, fs::Permissions::from_mode(0o755))
        .map_err(|e| format!("failed to chmod staging dir {staging_dir:?}: {e}"))?;

    let enumerator = parent_dir
        .enumerate_children(
            "standard::name,standard::type",
            gio::FileQueryInfoFlags::NONE,
            gio::Cancellable::NONE,
        )
        .map_err(|e| format!("failed to list source directory: {e}"))?;

    for info_result in enumerator {
        let info = match info_result {
            Ok(info) => info,
            Err(e) => {
                log_err("failed to read directory entry", &e);
                continue;
            }
        };

        if info.file_type() != gio::FileType::Regular {
            continue;
        }

        let name = info.name();
        let name_str = name.to_string_lossy();
        let matches_companion = COMPANION_EXTENSIONS.iter().any(|ext| {
            name_str
                .rsplit_once('.')
                .map(|(_, e)| e.eq_ignore_ascii_case(ext))
                .unwrap_or(false)
        });
        if !matches_companion {
            continue;
        }

        let src_child = parent_dir.child(&name);
        let dest_path = staging_dir.join(&*name_str);
        let dest_file = gio::File::for_path(&dest_path);

        if let Err(e) = src_child.copy(
            &dest_file,
            gio::FileCopyFlags::OVERWRITE,
            gio::Cancellable::NONE,
            None::<&mut dyn FnMut(i64, i64)>,
        ) {
            log_err(&format!("failed to copy {name_str}"), &e);
            continue;
        }

        if let Err(e) = fs::set_permissions(&dest_path, fs::Permissions::from_mode(0o644)) {
            log_err(&format!("failed to chmod {dest_path:?}"), &e);
        }
    }

    let staged_ovpn_path = staging_dir.join(&source_name);
    if !staged_ovpn_path.exists() {
        return Err(format!(
            "staged .ovpn file not found at {staged_ovpn_path:?} (copy step failed?)"
        ));
    }

    let requires_credentials = parser::requires_auth_user_pass_prompt(&original_content);
    fs::set_permissions(&staged_ovpn_path, fs::Permissions::from_mode(0o644))
        .map_err(|e| format!("failed to chmod staged .ovpn file: {e}"))?;

    Ok(StagedConfig {
        dir: staging_dir,
        config_path: staged_ovpn_path,
        requires_credentials,
        session_id,
        display_name,
    })
}

#[cfg(test)]
mod tests {
    use super::{display_name, stable_session_id};
    use std::path::Path;

    #[test]
    fn session_id_is_stable_and_changes_with_config_content() {
        let id = stable_session_id("file:///vpn/example.ovpn", "client\n");
        assert_eq!(
            id,
            stable_session_id("file:///vpn/example.ovpn", "client\n")
        );
        assert_ne!(
            id,
            stable_session_id("file:///vpn/example.ovpn", "client\nremote other\n")
        );
    }

    #[test]
    fn display_name_prefers_parent_directory_except_at_root() {
        assert_eq!(
            display_name(Some(Path::new("/vpn/work/example.ovpn")), "example.ovpn"),
            "work"
        );
        assert_eq!(
            display_name(Some(Path::new("/example.ovpn")), "example.ovpn"),
            "example"
        );
    }
}
