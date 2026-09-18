use std::thread;

use crate::staging::StagedConfig;
use crate::{credentials, network_manager, staging};

pub(super) fn activate(uri: String) {
    crate::log(format!("connecting to {uri}"));

    thread::spawn(move || {
        let Some((staged, config_path, credentials)) = stage_and_authenticate(&uri) else {
            return;
        };

        match tokio::runtime::Runtime::new()
            .map_err(|e| format!("failed to start tokio runtime: {e}"))
            .and_then(|runtime| {
                runtime.block_on(network_manager::activate_vpn(
                    &staged.session_id,
                    &config_path,
                    credentials.as_ref(),
                ))
            }) {
            Ok(()) => crate::log(format!(
                "VPN connection activated (session {})",
                staged.session_id
            )),
            Err(e) => crate::log_err(e),
        }
    });
}

pub(super) fn activate_terminal(uri: String) {
    crate::log(format!("connecting to {uri} in a terminal"));

    thread::spawn(move || {
        let Some((staged, config_path, credentials)) = stage_and_authenticate(&uri) else {
            return;
        };

        match super::terminal::launch(&staged, &config_path, credentials.as_ref()) {
            Ok(()) => crate::log(format!(
                "VPN terminal session started (session {})",
                staged.session_id
            )),
            Err(e) => crate::log_err(e),
        }
    });
}

/// Stages the `.ovpn` file and, if required, obtains credentials. Returns `None`
/// if staging failed or the user cancelled the credential prompt (already logged).
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

    if !staged.requires_credentials {
        return Some((staged, config_path, None));
    }

    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(e) => {
            crate::log_err(format!("failed to start tokio runtime: {e}"));
            return None;
        }
    };

    match runtime.block_on(credentials::get_credentials(uri, &staged.config_path)) {
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
