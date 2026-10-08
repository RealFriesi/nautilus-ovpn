use std::{mem::size_of, sync::OnceLock};

#[allow(
    dead_code,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals
)]
mod ffi {
    include!(concat!(env!("OUT_DIR"), "/nautilus_bindings.rs"));
}

static EXTENSION_TYPE: OnceLock<ffi::GType> = OnceLock::new();
static EXPORTED_TYPES: OnceLock<[ffi::GType; 1]> = OnceLock::new();

unsafe extern "C" fn get_file_items(
    _provider: *mut ffi::NautilusMenuProvider,
    _files: *mut ffi::GList,
) -> *mut ffi::GList {
    std::ptr::null_mut()
}

unsafe extern "C" fn get_background_items(
    _provider: *mut ffi::NautilusMenuProvider,
    _current_folder: *mut ffi::NautilusFileInfo,
) -> *mut ffi::GList {
    std::ptr::null_mut()
}

unsafe extern "C" fn initialize_menu_provider_interface(
    interface: *mut std::ffi::c_void,
    _data: *mut std::ffi::c_void,
) {
    unsafe {
        let interface = interface.cast::<ffi::NautilusMenuProviderInterface>();
        (*interface).get_file_items = Some(get_file_items);
        (*interface).get_background_items = Some(get_background_items);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn nautilus_module_initialize(module: *mut ffi::GTypeModule) {
    if module.is_null() {
        eprintln!("nautilus-ovpn: Nautilus passed a null GTypeModule");
        return;
    }

    let extension_type = EXTENSION_TYPE.get_or_init(|| unsafe {
        let mut type_info: ffi::GTypeInfo = std::mem::zeroed();
        type_info.class_size = size_of::<ffi::GObjectClass>() as _;
        type_info.instance_size = size_of::<ffi::GObject>() as _;

        let extension_type = ffi::g_type_module_register_type(
            module,
            ffi::g_object_get_type(),
            c"NautilusOvpnExtension".as_ptr(),
            &type_info,
            ffi::GTypeFlags_G_TYPE_FLAG_NONE,
        );

        if extension_type != 0 {
            let mut interface_info: ffi::GInterfaceInfo = std::mem::zeroed();
            interface_info.interface_init = Some(initialize_menu_provider_interface);
            ffi::g_type_module_add_interface(
                module,
                extension_type,
                ffi::nautilus_menu_provider_get_type(),
                &interface_info,
            );
        }

        extension_type
    });

    if *extension_type == 0 {
        eprintln!("nautilus-ovpn: failed to register the extension GObject type");
        return;
    }

    EXPORTED_TYPES.get_or_init(|| [*extension_type]);
}

#[unsafe(no_mangle)]
pub extern "C" fn nautilus_module_shutdown() {}

#[unsafe(no_mangle)]
pub extern "C" fn nautilus_module_list_types(
    types: *mut *const ffi::GType,
    num_types: *mut std::ffi::c_int,
) {
    if types.is_null() || num_types.is_null() {
        eprintln!("nautilus-ovpn: Nautilus passed null output pointers");
        return;
    }

    if let Some(exported_types) = EXPORTED_TYPES.get() {
        unsafe {
            *types = exported_types.as_ptr();
            *num_types = exported_types.len() as std::ffi::c_int;
        }
    } else {
        unsafe {
            *types = std::ptr::null();
            *num_types = 0;
        }
    }
}
