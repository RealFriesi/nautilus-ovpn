use std::path::Path;
use std::process::Command;

use super::VpnCredentials;

pub(super) struct CredentialPrompt {
    pub(super) credentials: VpnCredentials,
    pub(super) save: bool,
}

pub(super) fn prompt_for_credentials(
    config_path: &Path,
) -> Result<Option<CredentialPrompt>, String> {
    let title = format!(
        "VPN-Anmeldung fuer {}",
        config_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("OpenVPN")
    );

    let output = Command::new("zenity")
        .arg("--forms")
        .arg("--title")
        .arg(title)
        .arg("--text")
        .arg("Diese VPN-Verbindung benoetigt Anmeldedaten.")
        .arg("--separator")
        .arg("\n")
        .arg("--add-entry")
        .arg("Benutzername")
        .arg("--add-password")
        .arg("Passwort")
        .arg("--add-combo")
        .arg("Anmeldedaten speichern")
        .arg("--combo-values")
        .arg("Nein|Ja")
        .output();

    let output = match output {
        Ok(output) => output,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err("zenity is required to prompt for VPN credentials".to_string())
        }
        Err(e) => return Err(format!("failed to run zenity credential prompt: {e}")),
    };

    if !output.status.success() {
        return Ok(None);
    }

    let response = String::from_utf8(output.stdout)
        .map_err(|e| format!("credential prompt response is not valid UTF-8: {e}"))?;
    let mut lines = response.trim_end_matches('\n').split('\n');
    let username = lines.next().unwrap_or_default().trim().to_string();
    let password = lines.next().unwrap_or_default().to_string();
    let save = lines.next().unwrap_or("Nein").trim() == "Ja";

    if username.is_empty() || password.is_empty() {
        return Err("VPN username and password must not be empty".to_string());
    }

    Ok(Some(CredentialPrompt {
        credentials: VpnCredentials { username, password },
        save,
    }))
}
