use std::env;
use std::path::{Path, PathBuf};

const DOMAIN: &str = "nautilus-ovpn";

pub(crate) fn initialize() {
    let _ = gettextrs::setlocale(gettextrs::LocaleCategory::LcAll, "");
    let locale_directory = env::var_os("NAUTILUS_OVPN_LOCALEDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/usr/share/locale"));
    bind_locale_directory(&locale_directory);
    gettextrs::textdomain(DOMAIN).expect("failed to set gettext text domain");
}

pub(crate) fn translate(message: &str) -> String {
    gettextrs::gettext(message)
}

pub(crate) fn bind_locale_directory(path: &Path) {
    gettextrs::bindtextdomain(DOMAIN, path).expect("failed to bind gettext locale directory");
}
