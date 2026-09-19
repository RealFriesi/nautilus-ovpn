# nautilus-ovpn

Nautilus (GNOME Files) extension that adds a context-menu action for `.ovpn`
files to launch OpenVPN in a terminal via `pkexec`.

## Build dependencies

You need a Rust toolchain (`cargo`, edition 2021) plus the following system
packages providing headers/pkg-config files for `bindgen`, GLib, GTK4 and
`libnautilus-extension-4`.

### Ubuntu / Debian

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

### Fedora

```sh
sudo dnf install -y \
  gcc \
  pkgconf-pkg-config \
  clang-devel \
  glib2-devel \
  gtk4-devel \
  nautilus-devel
```

## Building

```sh
cargo build --release
```

The resulting shared library is `target/release/libnautilus_ovpn.so`. Copy it
to your system Nautilus extensions directory, then restart Nautilus
(`nautilus -q`). On Fedora, use:

```sh
sudo install -Dm755 target/release/libnautilus_ovpn.so \
  /usr/lib64/nautilus/extensions-4/libnautilus_ovpn.so
```

On Debian/Ubuntu, the corresponding directory is usually
`/usr/lib/x86_64-linux-gnu/nautilus/extensions-4`.

For each selected configuration, the extension reuses a staging directory
whose name is derived from the configuration path and contents. Changing
either creates a new directory.

## Runtime dependencies

- `openvpn` and the `NetworkManager-openvpn`/`network-manager-openvpn` plugin
- `openvpn` and `pkexec` (polkit) for the connection action.

The credential dialog stores the selected legacy-authentication setting with
the VPN credentials. A private-key password is optional and only needed when
the PKCS#12 file is protected by one. When enabled, the connection action adds
OpenVPN's `--providers legacy default` option.

The options dialog also opens for configurations without `auth-user-pass` so
legacy authentication and a private-key password can be selected. In that
case, the username and password fields are disabled and no `--auth-user-pass`
argument is passed to OpenVPN. The "Einstellungen speichern" option is enabled
only after it has been selected explicitly. Legacy authentication and saving
are disabled by default. A saved options set may contain an empty username and
password; when it is loaded from the keyring, the next activation starts
automatically after five seconds unless any UI control is changed or the
dialogue is cancelled.

## Terminal configuration

On first use, the extension creates
`$XDG_CONFIG_HOME/nautilus-ovpn/terminal.toml` (normally
`~/.config/nautilus-ovpn/terminal.toml`) with ordered Ptyxis, Ghostty, Kitty,
Alacritty, GNOME Terminal, Konsole, Xfce Terminal, Tilix, Foot and Xterm
launcher templates. The first configured `program` found in `PATH` is used,
so the configuration can support several terminal emulators without built-in
terminal detection. Existing user configurations are not overwritten.

The templates use MiniJinja syntax. `{{ command }}` is the rendered OpenVPN
shell command, `{{ title }}` is the connection name, and the command template
may use `{% if legacy_auth %}...{% endif %}`. Paths passed to the command
template (`staging_dir`, `config_path`, `auth_path`,
`private_key_password_path`, and `log_path`) are already shell-quoted.

For example, to prefer Kitty while retaining the defaults as fallbacks, add
this entry before the other `[[terminal]]` entries:

```toml
[[terminal]]
program = "kitty"
args = ["--title", "{{ title }}", "bash", "-lc", "{{ command }}"]
```
