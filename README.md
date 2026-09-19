# nautilus-ovpn

Nautilus (GNOME Files) extension that adds a context-menu action for `.ovpn`
files and starts an OpenVPN connection in a terminal via `pkexec`.

The extension stages the selected configuration and any companion files in a
private temporary directory, prompts for credentials if required, then launches
an available terminal emulator with a rendered OpenVPN command.

This is a direct OpenVPN launcher and no NetworkManager integration is required.

## Requirements

### Build dependencies

You need a Rust toolchain (edition 2021) with `cargo`, plus the development
headers and pkg-config metadata for `bindgen`, GLib, GTK4, and
`libnautilus-extension-4`.

#### Ubuntu / Debian

```sh
sudo apt-get update
sudo apt-get install -y --no-install-recommends \
  build-essential \
  pkg-config \
  libclang-dev \
  libglib2.0-dev \
  libgtk-4-dev \
  libnautilus-extension-dev
```

#### Fedora

```sh
sudo dnf install -y \
  gcc \
  pkgconf-pkg-config \
  clang-devel \
  glib2-devel \
  gtk4-devel \
  nautilus-devel
```

### Runtime dependencies

- `openvpn`
- `pkexec` (polkit) for the elevated OpenVPN launch

## Build and install

```sh
cargo build --release
```

The compiled shared library is `target/release/libnautilus_ovpn.so`.
Copy it into the Nautilus extension directory and restart Nautilus:

```sh
nautilus -q
```

On Fedora:

```sh
sudo install -Dm755 target/release/libnautilus_ovpn.so \
  /usr/lib64/nautilus/extensions-4/libnautilus_ovpn.so
```

On Debian/Ubuntu, the usual location is:

```sh
/usr/lib/x86_64-linux-gnu/nautilus/extensions-4
```

## How the extension works

For each selected configuration, the extension creates a staging directory
whose name is derived from the configuration URI and the file contents. If the
URI or the content changes, a new staging directory is created.

The staging process:

1. reads the selected `.ovpn` file
2. copies companion files such as `.crt`, `.key`, `.p12`, `.pem`, and `.txt`
3. detects whether the config requires `auth-user-pass`
4. asks for credentials and optional extra settings if needed
5. launches a terminal with a generated OpenVPN command

## Credentials and options

The credential dialog stores the selected legacy-authentication setting together
with the VPN credentials. A private-key password is optional and only needed
when the PKCS#12 file is protected by one.

If legacy authentication is enabled, the OpenVPN command adds:

```sh
--providers legacy default
```

The options dialog also opens for configurations without `auth-user-pass`, so
that legacy authentication and a private-key password can still be selected.
In that case, the username and password fields are disabled and no
`--auth-user-pass` argument is passed to OpenVPN.

The "Einstellungen speichern" option is only enabled after it has been selected
explicitly. Legacy authentication and saving are disabled by default. A saved
options set may contain empty username/password values; if it is loaded from the
keyring, the next activation starts automatically after five seconds unless the
user changes a control or cancels the dialog.

## Terminal configuration

On first use, the extension creates
`$XDG_CONFIG_HOME/nautilus-ovpn/terminal.toml` (normally
`~/.config/nautilus-ovpn/terminal.toml`). The generated file contains ordered
launcher templates for Ptyxis, Ghostty, Kitty, Alacritty, GNOME Terminal,
Konsole, Xfce Terminal, Tilix, Foot, and Xterm.

The first configured `program` found in `PATH` is used, so the extension can
work with several supported terminal emulators without hard-coded detection.
Existing user configurations are not overwritten.

The templates use MiniJinja syntax:

- `{{ command }}` is the rendered OpenVPN shell command
- `{{ title }}` is the connection name
- the command template may use `{% if legacy_auth %}...{% endif %}`
- paths passed into the template (`staging_dir`, `config_path`, `auth_path`,
  `private_key_password_path`, and `log_path`) are already shell-quoted

Example: to prefer Kitty while keeping the other templates as fallbacks,
insert this entry before the other `[[terminal]]` entries:

```toml
[[terminal]]
program = "kitty"
args = ["--title", "{{ title }}", "bash", "-lc", "{{ command }}"]
```
