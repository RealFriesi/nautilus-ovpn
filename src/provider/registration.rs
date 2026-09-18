use std::ffi::CString;
use std::ptr;
use std::sync::OnceLock;

use gobject_sys::{GInterfaceInfo, GTypeInfo, GTypeModule};

use crate::ffi;

static PROVIDER_TYPE: OnceLock<glib_sys::GType> = OnceLock::new();

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
    (*iface).get_file_items = Some(super::menu::get_file_items_trampoline);
    (*iface).get_background_items = None;
}

pub(crate) unsafe fn initialize(module: *mut GTypeModule) {
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
        crate::log_err("provider type was already registered");
    }
}

pub(crate) unsafe fn list_types(
    types: *mut *const glib_sys::GType,
    num_types: *mut std::os::raw::c_int,
) {
    static TYPE_SLOT: OnceLock<[glib_sys::GType; 1]> = OnceLock::new();

    match PROVIDER_TYPE.get() {
        Some(&provider_type) => {
            let slot = TYPE_SLOT.get_or_init(|| [provider_type]);
            *types = slot.as_ptr();
            *num_types = 1;
        }
        None => {
            crate::log_err("list_types called before initialize");
            *types = ptr::null();
            *num_types = 0;
        }
    }
}
