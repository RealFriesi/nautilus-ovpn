//! Nautilus menu-provider extension for activating OpenVPN configurations.

mod credentials;
mod ffi;
mod network_manager;
mod provider;
mod staging;

use std::os::raw::c_int;

pub(crate) fn log(msg: impl AsRef<str>) {
    println!("[nautilus-openvpn] {}", msg.as_ref());
}

pub(crate) fn log_err(msg: impl AsRef<str>) {
    eprintln!("[nautilus-openvpn] {}", msg.as_ref());
}

/// # Safety
/// Called by Nautilus with a valid `GTypeModule*` while loading this shared
/// object as an extension module.
#[no_mangle]
pub unsafe extern "C" fn nautilus_module_initialize(module: *mut gobject_sys::GTypeModule) {
    log("initializing extension module");
    if !gtk::is_initialized() {
        if let Err(error) = gtk::init() {
            log_err(format!("failed to initialize GTK: {error}"));
            return;
        }
    }
    provider::initialize(module);
}

/// # Safety
/// Called by Nautilus when unloading the extension module.
#[no_mangle]
pub unsafe extern "C" fn nautilus_module_shutdown() {
    log("shutting down extension module");
}

/// # Safety
/// Called by Nautilus to enumerate the `GType`s this module provides.
/// `types` and `num_types` must be valid, writable out-parameters.
#[no_mangle]
pub unsafe extern "C" fn nautilus_module_list_types(
    types: *mut *const glib_sys::GType,
    num_types: *mut c_int,
) {
    provider::list_types(types, num_types);
}
