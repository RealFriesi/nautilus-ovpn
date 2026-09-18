//! Raw C bindings to `libnautilus-extension-4`.
//!
//! `gtk-rs` does not ship bindings for the Nautilus extension ABI, so the
//! required types and functions are declared here by hand, matching the
//! headers in `/usr/include/nautilus/libnautilus-extension/`.

#![allow(non_camel_case_types, non_snake_case, dead_code)]

use glib_sys::{GList, GType};
use gobject_sys::{GObject, GObjectClass, GTypeInterface};
use libc::{c_char, c_void};

/// Opaque handle to a `NautilusFileInfo` (a `GObject`-based interface).
#[repr(C)]
pub struct NautilusFileInfo {
    _private: [u8; 0],
}

/// Opaque handle to a `NautilusMenu`.
#[repr(C)]
pub struct NautilusMenu {
    _private: [u8; 0],
}

/// Opaque handle to a `NautilusMenuItem` (a concrete `GObject` subclass).
#[repr(C)]
pub struct NautilusMenuItem {
    _private: [u8; 0],
}

/// Opaque handle to the `NautilusMenuProvider` interface implementor.
/// Our own provider instance is laid out as a plain `GObject`.
#[repr(C)]
pub struct NautilusMenuProvider {
    _private: [u8; 0],
}

/// `struct _NautilusMenuProviderInterface` from `nautilus-menu-provider.h`.
#[repr(C)]
pub struct NautilusMenuProviderInterface {
    pub g_iface: GTypeInterface,
    pub get_file_items: Option<
        unsafe extern "C" fn(provider: *mut NautilusMenuProvider, files: *mut GList) -> *mut GList,
    >,
    pub get_background_items: Option<
        unsafe extern "C" fn(
            provider: *mut NautilusMenuProvider,
            current_folder: *mut NautilusFileInfo,
        ) -> *mut GList,
    >,
}

/// Our concrete extension instance. It has no state beyond the base
/// `GObject`; all per-invocation state lives on the stack / in staged files.
#[repr(C)]
pub struct NautilusOvpnProvider {
    pub parent: GObject,
}

#[repr(C)]
pub struct NautilusOvpnProviderClass {
    pub parent_class: GObjectClass,
}

extern "C" {
    // nautilus-menu-provider.h
    pub fn nautilus_menu_provider_get_type() -> GType;

    // nautilus-menu.h
    pub fn nautilus_menu_item_new(
        name: *const c_char,
        label: *const c_char,
        tip: *const c_char,
        icon: *const c_char,
    ) -> *mut NautilusMenuItem;

    // nautilus-file-info.h
    pub fn nautilus_file_info_get_uri(file_info: *mut NautilusFileInfo) -> *mut c_char;
    pub fn nautilus_file_info_get_parent_uri(file_info: *mut NautilusFileInfo) -> *mut c_char;
}

extern "C" {
    // glib-object.h: used to hook up NautilusMenuItem::activate.
    pub fn g_signal_connect_data(
        instance: *mut c_void,
        detailed_signal: *const c_char,
        c_handler: Option<unsafe extern "C" fn()>,
        data: *mut c_void,
        destroy_data: Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>,
        connect_flags: u32,
    ) -> u64;
}
