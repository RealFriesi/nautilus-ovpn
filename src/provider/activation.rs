use std::thread;

use crate::staging::StagedConfig;
use crate::{credentials, staging};

pub(super) fn activate_terminal(uri: String) {
    crate::log(format!("starting OpenVPN terminal for {uri}"));

    // The activation work is intentionally moved into a background thread so the
    // Nautilus menu callback returns promptly while OpenVPN setup continues.
    thread::spawn(move || {
        let Some((staged, config_path, credentials)) = stage_and_authenticate(&uri) else {
            return;
        };

        match super::terminal::launch(&staged, &config_path, credentials.as_ref()) {
            Ok(()) => crate::log(format!(
                "VPN terminal started (session {})",
                staged.session_id
            )),
            Err(e) => crate::log_err(e),
        }
    });
}

/// Stages the `.ovpn` file and obtains connection options. Returns `None` if
/// staging failed or the user cancelled the options prompt (already logged).
fn stage_and_authenticate(
    uri: &str,
) -> Option<(StagedConfig, String, Option<credentials::VpnCredentials>)> {
    let staged = match staging::stage_ovpn_file(uri) {
        Ok(staged) => staged,
        Err(e) => {
            crate::log_err(format!("staging failed: {e}"));
            return None;
        }
    };
    crate::log(format!("staged VPN config in {}", staged.dir.display()));

    let config_path = match staged.config_path.to_str() {
        Some(path) => path.to_string(),
        None => {
            crate::log_err("staged config path is not valid UTF-8");
            return None;
        }
    };

    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(e) => {
            crate::log_err(format!("failed to start tokio runtime: {e}"));
            return None;
        }
    };

    match runtime.block_on(credentials::get_credentials(
        uri,
        &staged.config_path,
        staged.requires_credentials,
    )) {
        Ok(Some(credentials)) => Some((staged, config_path, Some(credentials))),
        Ok(None) => {
            crate::log("VPN activation cancelled");
            None
        }
        Err(e) => {
            crate::log_err(e);
            None
        }
    }
}
