use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

pub struct VpnCredentials {
    pub username: String,
    pub password: String,
}

pub fn get_credentials(
    config_uri: &str,
    config_path: &Path,
) -> Result<Option<VpnCredentials>, String> {
    if let Some(credentials) = lookup_stored_credentials(config_uri)? {
        return Ok(Some(credentials));
    }

    let Some(prompt) = prompt_for_credentials(config_path)? else {
        return Ok(None);
    };

    if prompt.save {
        store_credentials(config_uri, config_path, &prompt.credentials)?;
    }

    Ok(Some(prompt.credentials))
}

struct CredentialPrompt {
    credentials: VpnCredentials,
    save: bool,
}

fn lookup_stored_credentials(config_uri: &str) -> Result<Option<VpnCredentials>, String> {
    let output = Command::new("secret-tool")
        .arg("lookup")
        .arg("application")
        .arg("nautilus-ovpn")
        .arg("config-uri")
        .arg(config_uri)
        .output();

    let output = match output {
        Ok(output) => output,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("failed to run secret-tool lookup: {e}")),
    };

    if !output.status.success() || output.stdout.is_empty() {
        return Ok(None);
    }

    let secret = String::from_utf8(output.stdout)
        .map_err(|e| format!("stored VPN credentials are not valid UTF-8: {e}"))?;
    let Some((username, password)) = secret.trim_end_matches('\n').split_once('\n') else {
        return Ok(None);
    };

    if username.is_empty() || password.is_empty() {
        return Ok(None);
    }

    Ok(Some(VpnCredentials {
        username: username.to_string(),
        password: password.to_string(),
    }))
}

fn prompt_for_credentials(config_path: &Path) -> Result<Option<CredentialPrompt>, String> {
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

fn store_credentials(
    config_uri: &str,
    config_path: &Path,
    credentials: &VpnCredentials,
) -> Result<(), String> {
    let label = format!(
        "nautilus-ovpn {}",
        config_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("OpenVPN")
    );
    let mut child = Command::new("secret-tool")
        .arg("store")
        .arg("--label")
        .arg(label)
        .arg("application")
        .arg("nautilus-ovpn")
        .arg("config-uri")
        .arg(config_uri)
        .stdin(Stdio::piped())
        .spawn();

    let child = match child.as_mut() {
        Ok(child) => child,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err("secret-tool is required to save VPN credentials".to_string())
        }
        Err(e) => return Err(format!("failed to run secret-tool store: {e}")),
    };

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "failed to open secret-tool stdin".to_string())?;
    stdin
        .write_all(format!("{}\n{}", credentials.username, credentials.password).as_bytes())
        .map_err(|e| format!("failed to write VPN credentials to secret-tool: {e}"))?;
    drop(stdin);

    let status = child
        .wait()
        .map_err(|e| format!("failed to wait for secret-tool store: {e}"))?;
    if !status.success() {
        return Err(format!("secret-tool store failed with status {status}"));
    }

    Ok(())
}
