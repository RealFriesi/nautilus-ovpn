//! Nautilus menu-provider extension: adds a "Mit VPN verbinden" context menu
//! entry to `.ovpn` files and activates them as volatile NetworkManager VPN
//! connections, staging the config (and any companion cert/key files) out of
//! GVFS/SMB mounts first so `nm-openvpn` can actually read them.

mod ffi;
mod nm_dbus;
mod ovpn_parser;
mod staging;

use std::ffi::{CStr, CString};
use std::os::raw::c_int;
use std::ptr;
use std::sync::OnceLock;

use gobject_sys::{GInterfaceInfo, GTypeInfo, GTypeModule};
use libc::c_void;

/// The dynamically-registered `GType` for our provider, filled in by
/// `nautilus_module_initialize` and handed out by `nautilus_module_list_types`.
static PROVIDER_TYPE: OnceLock<glib_sys::GType> = OnceLock::new();

fn log(msg: impl AsRef<str>) {
    println!("[nautilus-openvpn] {}", msg.as_ref());
}

fn log_err(msg: impl AsRef<str>) {
    eprintln!("[nautilus-openvpn] {}", msg.as_ref());
}

// ---------------------------------------------------------------------
// GObject type registration
// ---------------------------------------------------------------------

unsafe extern "C" fn provider_class_init(
    _class: glib_sys::gpointer,
    _class_data: glib_sys::gpointer,
) {
}

unsafe extern "C" fn provider_iface_init(
    iface: glib_sys::gpointer,
    _iface_data: glib_sys::gpointer,
) {
    let iface = iface as *mut ffi::NautilusMenuProviderInterface;
    (*iface).get_file_items = Some(get_file_items_trampoline);
    (*iface).get_background_items = None;
}

/// Registers `NautilusOvpnProvider` as a dynamic type owned by `module`,
/// implementing the `NautilusMenuProvider` interface.
unsafe fn register_provider_type(module: *mut GTypeModule) {
    let type_info = GTypeInfo {
        class_size: std::mem::size_of::<ffi::NautilusOvpnProviderClass>() as u16,
        base_init: None,
        base_finalize: None,
        class_init: Some(provider_class_init),
        class_finalize: None,
        class_data: ptr::null(),
        instance_size: std::mem::size_of::<ffi::NautilusOvpnProvider>() as u16,
        n_preallocs: 0,
        instance_init: None,
        value_table: ptr::null(),
    };

    let type_name = CString::new("NautilusOvpnProvider").expect("static string");
    let provider_type = gobject_sys::g_type_module_register_type(
        module,
        gobject_sys::G_TYPE_OBJECT,
        type_name.as_ptr(),
        &type_info,
        0,
    );

    let iface_info = GInterfaceInfo {
        interface_init: Some(provider_iface_init),
        interface_finalize: None,
        interface_data: ptr::null_mut(),
    };
    gobject_sys::g_type_module_add_interface(
        module,
        provider_type,
        ffi::nautilus_menu_provider_get_type(),
        &iface_info,
    );

    if PROVIDER_TYPE.set(provider_type).is_err() {
        log_err("provider type was already registered");
    }
}

// ---------------------------------------------------------------------
// NautilusMenuProvider::get_file_items
// ---------------------------------------------------------------------

unsafe extern "C" fn free_boxed_uri(data: *mut c_void, _closure: *mut c_void) {
    drop(Box::from_raw(data as *mut String));
}

unsafe extern "C" fn on_menu_item_activate(
    _item: *mut ffi::NautilusMenuItem,
    user_data: *mut c_void,
) {
    let uri = (*(user_data as *const String)).clone();
    log(format!("connecting to {uri}"));

    // Do the staging + D-Bus work off the main thread so the Nautilus UI
    // (and its GTK main loop) never blocks on network/filesystem I/O.
    std::thread::spawn(move || {
        let staged = match staging::stage_ovpn_file(&uri) {
            Ok(staged) => staged,
            Err(e) => {
                log_err(format!("staging failed: {e}"));
                return;
            }
        };

        let config_path = match staged.config_path.to_str() {
            Some(p) => p.to_string(),
            None => {
                log_err("staged config path is not valid UTF-8");
                return;
            }
        };

        let runtime = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                log_err(format!("failed to start tokio runtime: {e}"));
                return;
            }
        };

        match runtime.block_on(nm_dbus::activate_vpn(&staged.session_id, &config_path)) {
            Ok(()) => log(format!(
                "VPN connection activated (session {})",
                staged.session_id
            )),
            Err(e) => log_err(e),
        }
    });
}

unsafe extern "C" fn get_file_items_trampoline(
    _provider: *mut ffi::NautilusMenuProvider,
    files: *mut glib_sys::GList,
) -> *mut glib_sys::GList {
    if files.is_null() || glib_sys::g_list_length(files) != 1 {
        return ptr::null_mut();
    }

    let file_info = (*files).data as *mut ffi::NautilusFileInfo;
    if file_info.is_null() {
        return ptr::null_mut();
    }

    let uri_ptr = ffi::nautilus_file_info_get_uri(file_info);
    if uri_ptr.is_null() {
        return ptr::null_mut();
    }
    let uri = CStr::from_ptr(uri_ptr).to_string_lossy().into_owned();
    glib_sys::g_free(uri_ptr as *mut c_void);

    if !uri.to_ascii_lowercase().ends_with(".ovpn") {
        return ptr::null_mut();
    }

    let name = CString::new("OvpnConnect::connect").expect("static string");
    let label = CString::new("Mit VPN verbinden").expect("static string");
    let tip = CString::new("OpenVPN-Verbindung als flüchtige NetworkManager-Verbindung starten")
        .expect("static string");
    let icon = CString::new("network-vpn").expect("static string");

    let item =
        ffi::nautilus_menu_item_new(name.as_ptr(), label.as_ptr(), tip.as_ptr(), icon.as_ptr());
    if item.is_null() {
        log_err("nautilus_menu_item_new returned NULL");
        return ptr::null_mut();
    }

    let boxed_uri = Box::into_raw(Box::new(uri));
    let signal_name = CString::new("activate").expect("static string");

    let handler: unsafe extern "C" fn() = std::mem::transmute::<
        unsafe extern "C" fn(*mut ffi::NautilusMenuItem, *mut c_void),
        unsafe extern "C" fn(),
    >(on_menu_item_activate);

    ffi::g_signal_connect_data(
        item as *mut c_void,
        signal_name.as_ptr(),
        Some(handler),
        boxed_uri as *mut c_void,
        Some(free_boxed_uri),
        0,
    );

    glib_sys::g_list_append(ptr::null_mut(), item as *mut c_void)
}

// ---------------------------------------------------------------------
// Nautilus module entry points
// ---------------------------------------------------------------------

/// # Safety
/// Called by Nautilus with a valid `GTypeModule*` while loading this shared
/// object as an extension module.
#[no_mangle]
pub unsafe extern "C" fn nautilus_module_initialize(module: *mut GTypeModule) {
    log("initializing extension module");
    register_provider_type(module);
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
    static TYPE_SLOT: OnceLock<[glib_sys::GType; 1]> = OnceLock::new();

    match PROVIDER_TYPE.get() {
        Some(&provider_type) => {
            let slot = TYPE_SLOT.get_or_init(|| [provider_type]);
            *types = slot.as_ptr();
            *num_types = 1;
        }
        None => {
            log_err("list_types called before initialize");
            *types = ptr::null();
            *num_types = 0;
        }
    }
}
