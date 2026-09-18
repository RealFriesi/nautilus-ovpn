//! Handles copying an `.ovpn` file (and its companion files, e.g. from an
//! SMB/GVFS share) into a local, `nm-openvpn`-readable staging directory,
//! and applies the legacy-provider patch to the staged copy.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use gio::prelude::*;

use crate::ovpn_parser;

/// Extensions of companion files that are copied alongside the `.ovpn` file.
const COMPANION_EXTENSIONS: &[&str] = &["ovpn", "crt", "key", "p12", "pem", "txt"];

pub struct StagedConfig {
    /// The staging directory, e.g. `/tmp/nm-ovpn-<uuid>`.
    pub dir: PathBuf,
    /// Full path to the patched `.ovpn` file inside `dir`.
    pub config_path: PathBuf,
    /// Unique session id used both for the staging dir name and the
    /// NetworkManager connection id.
    pub session_id: String,
}

fn log_err(context: &str, err: &impl std::fmt::Display) {
    eprintln!("[nautilus-openvpn] {context}: {err}");
}

/// Copies `source_uri` (and any companion files living next to it) into a
/// freshly created staging directory under `/tmp`, then patches the
/// legacy OpenSSL provider directives into the staged `.ovpn` copy.
pub fn stage_ovpn_file(source_uri: &str) -> Result<StagedConfig, String> {
    let source_file = gio::File::for_uri(source_uri);
    let source_name = source_file
        .basename()
        .ok_or_else(|| "could not determine file name of selected .ovpn file".to_string())?;
    let source_name = source_name.to_string_lossy().to_string();

    let parent_dir = source_file
        .parent()
        .ok_or_else(|| "selected .ovpn file has no parent directory".to_string())?;

    let session_id = uuid::Uuid::new_v4().simple().to_string();
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

    let original_content = fs::read_to_string(&staged_ovpn_path)
        .map_err(|e| format!("failed to read staged .ovpn file: {e}"))?;
    let patched_content = ovpn_parser::patch_legacy_provider(&original_content);
    fs::write(&staged_ovpn_path, patched_content)
        .map_err(|e| format!("failed to write patched .ovpn file: {e}"))?;
    fs::set_permissions(&staged_ovpn_path, fs::Permissions::from_mode(0o644))
        .map_err(|e| format!("failed to chmod patched .ovpn file: {e}"))?;

    Ok(StagedConfig {
        dir: staging_dir,
        config_path: staged_ovpn_path,
        session_id,
    })
}
