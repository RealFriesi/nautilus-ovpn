use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;

use crate::credentials::VpnCredentials;
use crate::staging::StagedConfig;

// Ordered by preference; each entry pairs the binary name with the args needed
// to make it execute a command instead of opening an interactive shell.
const TERMINALS: &[(&str, &[&str])] = &[
    ("x-terminal-emulator", &["-e"]),
    ("xdg-terminal-exec", &[]),
    ("gnome-terminal", &["--"]),
    ("konsole", &["-e"]),
    ("xfce4-terminal", &["-x"]),
    ("xterm", &["-e"]),
];

fn command_exists(name: &str) -> bool {
    let Some(path_var) = env::var_os("PATH") else {
        return false;
    };
    env::split_paths(&path_var).any(|dir| dir.join(name).is_file())
}

fn find_terminal() -> Option<(&'static str, &'static [&'static str])> {
    TERMINALS
        .iter()
        .find(|(name, _)| command_exists(name))
        .copied()
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub(super) fn launch(
    staged: &StagedConfig,
    config_path: &str,
    credentials: Option<&VpnCredentials>,
) -> Result<(), String> {
    let (terminal, prefix_args) = find_terminal()
        .ok_or_else(|| "no supported terminal emulator found in PATH".to_string())?;

    let mut openvpn_cmd = format!("pkexec openvpn --config {}", shell_quote(config_path));
    if let Some(credentials) = credentials {
        let auth_path = write_auth_file(staged, credentials)?;
        let auth_path = auth_path.to_string_lossy().into_owned();
        openvpn_cmd.push_str(&format!(" --auth-user-pass {}", shell_quote(&auth_path)));
    }

    let full_cmd =
        format!("{openvpn_cmd}; echo; read -p 'Verbindung beendet. Enter zum Schließen druecken.'");

    let mut args: Vec<&str> = prefix_args.to_vec();
    args.extend(["bash", "-c", &full_cmd]);

    Command::new(terminal)
        .args(&args)
        .spawn()
        .map_err(|e| format!("failed to launch {terminal}: {e}"))?;

    Ok(())
}

// Written next to the staged config so pkexec/openvpn can read it; 0600 keeps it owner-only.
fn write_auth_file(staged: &StagedConfig, credentials: &VpnCredentials) -> Result<PathBuf, String> {
    let auth_path = staged.dir.join("auth.txt");
    let contents = format!("{}\n{}\n", credentials.username, credentials.password);
    fs::write(&auth_path, contents)
        .map_err(|e| format!("failed to write auth file {auth_path:?}: {e}"))?;
    fs::set_permissions(&auth_path, fs::Permissions::from_mode(0o600))
        .map_err(|e| format!("failed to chmod auth file {auth_path:?}: {e}"))?;
    Ok(auth_path)
}
