#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    dead_code,
    improper_ctypes
)]

use glib_sys::{GList, GType};
use gobject_sys::{GObject, GObjectClass, GTypeInterface};

include!(concat!(env!("OUT_DIR"), "/nautilus_bindings.rs"));

#[repr(C)]
pub struct NautilusMenuProviderInterface {
    pub g_iface: GTypeInterface,
    pub get_file_items:
        Option<unsafe extern "C" fn(*mut NautilusMenuProvider, *mut GList) -> *mut GList>,
    pub get_background_items: Option<
        unsafe extern "C" fn(*mut NautilusMenuProvider, *mut NautilusFileInfo) -> *mut GList,
    >,
}

#[repr(C)]
pub struct NautilusOvpnProvider {
    pub parent: GObject,
}

#[repr(C)]
pub struct NautilusOvpnProviderClass {
    pub parent_class: GObjectClass,
}
