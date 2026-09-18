use std::ffi::{CStr, CString};
use std::os::raw::c_void;
use std::ptr;

use crate::ffi;

pub(super) unsafe extern "C" fn free_boxed_uri(
    data: glib_sys::gpointer,
    _closure: *mut gobject_sys::GClosure,
) {
    drop(Box::from_raw(data as *mut String));
}

unsafe extern "C" fn on_activate_network_manager(
    _item: *mut ffi::NautilusMenuItem,
    user_data: *mut c_void,
) {
    let uri = (*(user_data as *const String)).clone();
    super::activation::activate(uri);
}

unsafe extern "C" fn on_activate_terminal(
    _item: *mut ffi::NautilusMenuItem,
    user_data: *mut c_void,
) {
    let uri = (*(user_data as *const String)).clone();
    super::activation::activate_terminal(uri);
}

unsafe fn build_menu_item(
    name: &str,
    label: &str,
    tip: &str,
    icon: &str,
    uri: &str,
    handler: unsafe extern "C" fn(*mut ffi::NautilusMenuItem, *mut c_void),
) -> *mut ffi::NautilusMenuItem {
    let name = CString::new(name).expect("static string");
    let label = CString::new(label).expect("static string");
    let tip = CString::new(tip).expect("static string");
    let icon = CString::new(icon).expect("static string");

    let item =
        ffi::nautilus_menu_item_new(name.as_ptr(), label.as_ptr(), tip.as_ptr(), icon.as_ptr());
    if item.is_null() {
        crate::log_err("nautilus_menu_item_new returned NULL");
        return ptr::null_mut();
    }

    let boxed_uri = Box::into_raw(Box::new(uri.to_string()));
    let signal_name = CString::new("activate").expect("static string");

    let handler: unsafe extern "C" fn() = std::mem::transmute::<
        unsafe extern "C" fn(*mut ffi::NautilusMenuItem, *mut c_void),
        unsafe extern "C" fn(),
    >(handler);

    gobject_sys::g_signal_connect_data(
        item as *mut gobject_sys::GObject,
        signal_name.as_ptr(),
        Some(handler),
        boxed_uri as *mut c_void,
        Some(free_boxed_uri),
        gobject_sys::G_CONNECT_DEFAULT,
    );

    item
}

pub(super) unsafe extern "C" fn get_file_items_trampoline(
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

    let nm_item = build_menu_item(
        "OvpnConnect::connect_nm",
        "Verbinde über NetworkManager",
        "OpenVPN-Verbindung als flüchtige NetworkManager-Verbindung starten",
        "network-vpn",
        &uri,
        on_activate_network_manager,
    );
    let terminal_item = build_menu_item(
        "OvpnConnect::connect_terminal",
        "Verbinde im Terminal",
        "OpenVPN-Verbindung im Terminal starten",
        "utilities-terminal",
        &uri,
        on_activate_terminal,
    );

    let mut list = ptr::null_mut();
    if !nm_item.is_null() {
        list = glib_sys::g_list_append(list, nm_item as *mut c_void);
    }
    if !terminal_item.is_null() {
        list = glib_sys::g_list_append(list, terminal_item as *mut c_void);
    }
    list
}
