# nautilus-ovpn

Nautilus (GNOME Files) extension that adds context-menu actions for `.ovpn`
files: connect via NetworkManager (volatile connection) or launch OpenVPN in a
terminal via `pkexec`.

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
to your Nautilus extensions directory (e.g.
`~/.local/share/nautilus-python/extensions` is for Python extensions — for
this native extension use `~/.local/share/nautilus/extensions-4` or the
system-wide `/usr/lib/x86_64-linux-gnu/nautilus/extensions-4`, depending on
your distribution), then restart Nautilus (`nautilus -q`).

## Runtime dependencies

- `openvpn` and the `NetworkManager-openvpn`/`network-manager-openvpn` plugin
  for the "Verbinde über NetworkManager" action.
- `openvpn` and `pkexec` (polkit) for the "Verbinde im Terminal" action.
- A terminal emulator for the terminal action: `gnome-terminal`, `konsole`,
  `xfce4-terminal`, `xterm`, or anything providing `x-terminal-emulator` /
  `xdg-terminal-exec`.
