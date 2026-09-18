use std::thread;

use crate::{credentials, network_manager, staging};

pub(super) fn activate(uri: String) {
    crate::log(format!("connecting to {uri}"));

    thread::spawn(move || {
        let staged = match staging::stage_ovpn_file(&uri) {
            Ok(staged) => staged,
            Err(e) => {
                crate::log_err(format!("staging failed: {e}"));
                return;
            }
        };
        crate::log(format!("staged VPN config in {}", staged.dir.display()));

        let config_path = match staged.config_path.to_str() {
            Some(path) => path.to_string(),
            None => {
                crate::log_err("staged config path is not valid UTF-8");
                return;
            }
        };

        let runtime = match tokio::runtime::Runtime::new() {
            Ok(runtime) => runtime,
            Err(e) => {
                crate::log_err(format!("failed to start tokio runtime: {e}"));
                return;
            }
        };

        let credentials = if staged.requires_credentials {
            match runtime.block_on(credentials::get_credentials(&uri, &staged.config_path)) {
                Ok(Some(credentials)) => Some(credentials),
                Ok(None) => {
                    crate::log("VPN activation cancelled");
                    return;
                }
                Err(e) => {
                    crate::log_err(e);
                    return;
                }
            }
        } else {
            None
        };

        match runtime.block_on(network_manager::activate_vpn(
            &staged.session_id,
            &config_path,
            credentials.as_ref(),
        )) {
            Ok(()) => crate::log(format!(
                "VPN connection activated (session {})",
                staged.session_id
            )),
            Err(e) => crate::log_err(e),
        }
    });
}
