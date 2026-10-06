# nautilus-ovpn

Nautilus (GNOME Files) extension that adds a context-menu action for `.ovpn`
files and manages an OpenVPN 3 Linux session over the system D-Bus.

The extension embeds referenced profile files directly into a temporary OpenVPN
3 D-Bus configuration, collects credentials in GTK dialogs, and starts no
terminal, shell, or OpenVPN CLI process.

This uses the OpenVPN 3 Linux D-Bus services directly and does not require
NetworkManager integration.

## Note

This project was originally developed for personal use, but you are welcome to
use it and adapt it to your own needs. It was developed with the assistance of
AI.

## Requirements

### Build dependencies

You need a Rust toolchain (edition 2021) with `cargo`, plus the development
headers and pkg-config metadata for `bindgen`, GLib, GTK4, and
`libnautilus-extension-4`, plus C++20, standalone Asio, and fmt for the
OpenVPN profile-merger bindings.

#### Ubuntu / Debian

```sh
sudo apt-get update
sudo apt-get install -y --no-install-recommends \
  build-essential \
  pkg-config \
  libclang-dev \
  libasio-dev \
  libfmt-dev \
  libglib2.0-dev \
  libgtk-4-dev \
  libnautilus-extension-dev \
  gettext
```

#### Fedora

```sh
sudo dnf install -y \
  gcc \
  pkgconf-pkg-config \
  clang-devel \
  asio-devel \
  fmt-devel \
  glib2-devel \
  gtk4-devel \
  nautilus-devel \
  gettext
```

### Runtime dependencies

- OpenVPN 3 Linux and its system-bus services
- A Secret Service keyring (for example GNOME Keyring)

## Build and install

```sh
git submodule update --init
cargo build --workspace --release
```

The compiled shared library is `target/release/libnautilus_ovpn.so`.
The default UI language is English. German translations are provided as a
gettext catalog and must be installed alongside the shared library. Copy both
the library and the locale catalog, then restart Nautilus:

```sh
nautilus -q
```

On Fedora:

```sh
sudo install -Dm755 target/release/libnautilus_ovpn.so \
  /usr/lib64/nautilus/extensions-4/libnautilus_ovpn.so
sudo install -Dm644 target/locale/de/LC_MESSAGES/nautilus-ovpn.mo \
  /usr/share/locale/de/LC_MESSAGES/nautilus-ovpn.mo
```

On Debian/Ubuntu, the usual location is:

```sh
/usr/lib/x86_64-linux-gnu/nautilus/extensions-4
```

Install the shared library and catalog on Debian/Ubuntu with:

```sh
sudo install -Dm755 target/release/libnautilus_ovpn.so \
  /usr/lib/x86_64-linux-gnu/nautilus/extensions-4/libnautilus_ovpn.so
sudo install -Dm644 target/locale/de/LC_MESSAGES/nautilus-ovpn.mo \
  /usr/share/locale/de/LC_MESSAGES/nautilus-ovpn.mo
```

For a source checkout, create the catalog before installing it:

```sh
mkdir -p target/locale/de/LC_MESSAGES
msgfmt po/de.po -o target/locale/de/LC_MESSAGES/nautilus-ovpn.mo
```

If the locale files are installed below a different prefix, set
`NAUTILUS_OVPN_LOCALEDIR` to that prefix's `share/locale` directory before
starting Nautilus.

## How the extension works

For each selected configuration, the extension reads the `.ovpn` file and its
referenced companion files directly through GIO. It computes an XXH3-128 profile
hash from the profile contents for Secret Service lookup; the source URI is not
part of the keyring identity. No temporary profile directory is created.

The activation process:

1. reads the selected `.ovpn` file and companion files directly from their source
2. embeds external certificates and keys into the in-memory D-Bus payload
3. imports the profile with `single_use=true` and `persistent=false`
4. creates a session through `net.openvpn.v3.sessions.NewTunnel`
5. handles `AttentionRequired` and the session input queue using GTK dialogs
6. calls `Ready`, `Connect`, and, on request, `Disconnect`
7. releases the in-memory profile payload after the session finishes

## Credentials

When OpenVPN requests input, the extension checks the Secret Service before
opening a GTK dialog. Passwords, proxy credentials, and private-key passphrases
can be saved. Dynamic challenge/TOTP inputs are never saved. OpenVPN's input
type, group, ID, description, and hidden-input flag drive the prompt and reply.

GTK dialogs run on Nautilus' GLib main context. D-Bus work runs on a Tokio
worker and exchanges prompt requests and replies through asynchronous channels.
No terminal-emulator configuration is created or consulted.

## Hooks

Optional Rhai hooks can be placed in
`$XDG_CONFIG_HOME/nautilus-ovpn/hooks/` (normally
`~/.config/nautilus-ovpn/hooks/`). `pre-connect.rhai` runs before the D-Bus
`Connect` call; returning `false`, a non-zero integer, or throwing an error
aborts the connection. `post-disconnect.rhai` runs after the session ends.

Hooks receive a mutable `context` with `config` and `dbus_payload` maps. Use
`context.get_config(key)`, `context.set_config(key, value)`,
`context.get_dbus_payload(key)`, and `context.set_dbus_payload(key, value)`.
Payload mutation is prepared for future D-Bus options; it does not yet change
the parameters sent to OpenVPN 3.

Example `pre-connect.rhai`:

```rhai
let profile = context.get_config("profile_name");
context.set_dbus_payload("audit_profile", profile);
true
```
