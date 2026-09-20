use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;

use minijinja::{context, Environment};
use serde::Deserialize;

use crate::credentials::VpnCredentials;
use crate::staging::StagedConfig;

const DEFAULT_CONFIG: &str = r#"# Terminal launcher for nautilus-ovpn.
#
# The program and each entry in args are templates. The OpenVPN command is
# rendered separately and inserted as {{ command }}. Paths in the command
# template are already shell-quoted.

[[terminal]]
program = "ptyxis"
args = ["--title", "{{ title }}", "--", "bash", "-lc", "{{ command }}"]

[[terminal]]
program = "ghostty"
args = ["--title", "{{ title }}", "-e", "bash", "-lc", "{{ command }}"]

[[terminal]]
program = "kitty"
args = ["--title", "{{ title }}", "bash", "-lc", "{{ command }}"]

[[terminal]]
program = "alacritty"
args = ["--title", "{{ title }}", "-e", "bash", "-lc", "{{ command }}"]

[[terminal]]
program = "gnome-terminal"
args = ["--title", "{{ title }}", "--", "bash", "-lc", "{{ command }}"]

[[terminal]]
program = "konsole"
args = ["--title", "{{ title }}", "-e", "bash", "-lc", "{{ command }}"]

[[terminal]]
program = "xfce4-terminal"
args = ["--title", "{{ title }}", "-x", "bash", "-lc", "{{ command }}"]

[[terminal]]
program = "tilix"
args = ["--title", "{{ title }}", "-e", "bash", "-lc", "{{ command }}"]

[[terminal]]
program = "foot"
args = ["--title", "{{ title }}", "bash", "-lc", "{{ command }}"]

[[terminal]]
program = "xterm"
args = ["-T", "{{ title }}", "-e", "bash", "-lc", "{{ command }}"]

[openvpn]
command = """
set -o pipefail
pkexec openvpn{% if legacy_auth %} --providers legacy default{% endif %} --cd {{ staging_dir }} --config {{ config_path }}{% if auth_path %} --auth-user-pass {{ auth_path }}{% endif %}{% if private_key_password_path %} --askpass {{ private_key_password_path }}{% endif %} 2>&1 | tee {{ log_path }}
status=$?
echo
read -rp {{ finished_prompt }}
exit "$status"
"""
"#;

#[derive(Deserialize)]
struct Config {
    terminal: Vec<TerminalConfig>,
    openvpn: OpenVpnConfig,
}

#[derive(Deserialize)]
struct TerminalConfig {
    program: String,
    args: Vec<String>,
}

#[derive(Deserialize)]
struct OpenVpnConfig {
    command: String,
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub(super) fn launch(
    staged: &StagedConfig,
    config_path: &str,
    credentials: Option<&VpnCredentials>,
) -> Result<(), String> {
    let config = load_config()?;
    let terminal = select_terminal(&config.terminal)?;
    let log_path = prepare_log_file(staged)?;
    let auth_path = if staged.requires_credentials {
        credentials
            .map(|credentials| write_auth_file(staged, credentials))
            .transpose()?
    } else {
        None
    };
    let private_key_password_path = credentials
        .filter(|credentials| !credentials.private_key_password.is_empty())
        .map(|credentials| write_private_key_password_file(staged, credentials))
        .transpose()?;

    let command = render(
        &config.openvpn.command,
        context! {
            legacy_auth => credentials.is_some_and(|credentials| credentials.legacy_auth),
            staging_dir => shell_quote(&staged.dir.to_string_lossy()),
            config_path => shell_quote(config_path),
            auth_path => auth_path
                .as_ref()
                .map(|path| shell_quote(&path.to_string_lossy()))
                .unwrap_or_default(),
            private_key_password_path => private_key_password_path
                .as_ref()
                .map(|path| shell_quote(&path.to_string_lossy()))
                .unwrap_or_default(),
            log_path => shell_quote(&log_path.to_string_lossy()),
            finished_prompt => shell_quote(&crate::i18n::translate(
                "Connection ended. Press Enter to close.",
            )),
        },
    )?;
    let args = terminal
        .args
        .iter()
        .map(|template| {
            render(
                template,
                context! {
                    title => staged.display_name.clone(),
                    command => command.clone(),
                },
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    Command::new(&terminal.program)
        .args(args)
        .spawn()
        .map_err(|error| {
            format!(
                "failed to launch configured terminal {}: {error}",
                terminal.program
            )
        })?;
    Ok(())
}

fn select_terminal(terminals: &[TerminalConfig]) -> Result<&TerminalConfig, String> {
    terminals
        .iter()
        .find(|terminal| command_exists(&terminal.program))
        .ok_or_else(|| {
            let programs = terminals
                .iter()
                .map(|terminal| terminal.program.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            format!("none of the configured terminal programs are available in PATH: {programs}")
        })
}

fn command_exists(name: &str) -> bool {
    let Some(path_var) = env::var_os("PATH") else {
        return false;
    };
    env::split_paths(&path_var).any(|directory| directory.join(name).is_file())
}

fn load_config() -> Result<Config, String> {
    let path = config_path()?;
    if !path.exists() {
        let parent = path
            .parent()
            .ok_or_else(|| format!("configuration path {path:?} has no parent directory"))?;
        fs::create_dir_all(parent).map_err(|error| {
            format!("failed to create configuration directory {parent:?}: {error}")
        })?;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700)).map_err(|error| {
            format!("failed to chmod configuration directory {parent:?}: {error}")
        })?;
        fs::write(&path, DEFAULT_CONFIG).map_err(|error| {
            format!("failed to create terminal configuration {path:?}: {error}")
        })?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("failed to chmod terminal configuration {path:?}: {error}"))?;
        crate::log(format!(
            "created terminal configuration at {}",
            path.display()
        ));
    }

    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read terminal configuration {path:?}: {error}"))?;
    toml::from_str(&contents)
        .map_err(|error| format!("invalid terminal configuration {path:?}: {error}"))
}

fn config_path() -> Result<PathBuf, String> {
    let config_home = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .ok_or_else(|| "could not determine XDG configuration directory".to_string())?;
    Ok(config_home.join("nautilus-ovpn").join("terminal.toml"))
}

fn render(template: &str, context: minijinja::Value) -> Result<String, String> {
    let mut environment = Environment::new();
    environment
        .add_template("template", template)
        .map_err(|error| format!("invalid terminal template: {error}"))?;
    environment
        .get_template("template")
        .expect("template was added successfully")
        .render(context)
        .map_err(|error| format!("failed to render terminal template: {error}"))
}

fn prepare_log_file(staged: &StagedConfig) -> Result<PathBuf, String> {
    let log_path = staged.dir.join("openvpn.log");
    fs::write(&log_path, "")
        .map_err(|error| format!("failed to create OpenVPN log file {log_path:?}: {error}"))?;
    fs::set_permissions(&log_path, fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("failed to chmod OpenVPN log file {log_path:?}: {error}"))?;
    Ok(log_path)
}

fn write_auth_file(staged: &StagedConfig, credentials: &VpnCredentials) -> Result<PathBuf, String> {
    let auth_path = staged.dir.join("auth.txt");
    let contents = format!("{}\n{}\n", credentials.username, credentials.password);
    fs::write(&auth_path, contents)
        .map_err(|error| format!("failed to write auth file {auth_path:?}: {error}"))?;
    fs::set_permissions(&auth_path, fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("failed to chmod auth file {auth_path:?}: {error}"))?;
    Ok(auth_path)
}

fn write_private_key_password_file(
    staged: &StagedConfig,
    credentials: &VpnCredentials,
) -> Result<PathBuf, String> {
    let password_path = staged.dir.join("private-key-password.txt");
    fs::write(
        &password_path,
        format!("{}\n", credentials.private_key_password),
    )
    .map_err(|error| {
        format!("failed to write private key password file {password_path:?}: {error}")
    })?;
    fs::set_permissions(&password_path, fs::Permissions::from_mode(0o600)).map_err(|error| {
        format!("failed to chmod private key password file {password_path:?}: {error}")
    })?;
    Ok(password_path)
}

#[cfg(test)]
mod tests {
    use super::{render, select_terminal, Config, TerminalConfig, DEFAULT_CONFIG};
    use minijinja::context;

    #[test]
    fn default_command_adds_legacy_provider_only_when_requested() {
        let config: Config = toml::from_str(DEFAULT_CONFIG).expect("default config is valid TOML");
        let with_legacy = render(
            &config.openvpn.command,
            context! {
                legacy_auth => true,
                staging_dir => "'/tmp/vpn'",
                config_path => "'/tmp/vpn/config.ovpn'",
                auth_path => "",
                private_key_password_path => "",
                log_path => "'/tmp/vpn/openvpn.log'",
                finished_prompt => "'Connection ended. Press Enter to close.'",
            },
        )
        .expect("default command renders");
        let without_legacy = render(
            &config.openvpn.command,
            context! {
                legacy_auth => false,
                staging_dir => "'/tmp/vpn'",
                config_path => "'/tmp/vpn/config.ovpn'",
                auth_path => "",
                private_key_password_path => "",
                log_path => "'/tmp/vpn/openvpn.log'",
            },
        )
        .expect("default command renders");

        assert!(with_legacy.contains("--providers legacy default"));
        assert!(!without_legacy.contains("--providers legacy default"));
        assert!(!without_legacy.contains("--auth-user-pass"));
    }

    #[test]
    fn selects_the_first_terminal_available_in_path() {
        let terminals = vec![
            TerminalConfig {
                program: "nautilus-ovpn-missing-terminal".to_string(),
                args: Vec::new(),
            },
            TerminalConfig {
                program: "sh".to_string(),
                args: Vec::new(),
            },
        ];

        let terminal = select_terminal(&terminals).expect("sh is available in the test PATH");
        assert_eq!(terminal.program, "sh");
    }
}
