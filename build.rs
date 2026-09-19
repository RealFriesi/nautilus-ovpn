use std::{env, path::PathBuf};

fn main() {
    let deps = system_deps::Config::new().probe().unwrap();

    let mut bindings = bindgen::Builder::default()
        .header("src/ffi-wrapper.h")
        .allowlist_function("nautilus_menu_provider_get_type")
        .allowlist_function("nautilus_menu_item_new")
        .allowlist_function("nautilus_file_info_get_uri")
        .allowlist_type("Nautilus(MenuProvider|MenuItem|FileInfo|Menu)")
        .allowlist_type("_Nautilus(MenuProvider|MenuItem|FileInfo|Menu)")
        .blocklist_type("GList")
        .blocklist_type("GType")
        .blocklist_type("GObject")
        .blocklist_type("GObjectClass")
        .blocklist_type("GTypeInterface")
        .blocklist_type("GTypeModule")
        .opaque_type("NautilusFileInfo")
        .opaque_type("NautilusMenu")
        .opaque_type("NautilusMenuItem")
        .opaque_type("_NautilusFileInfo")
        .opaque_type("_NautilusMenu")
        .opaque_type("_NautilusMenuItem")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()));

    for include_path in deps.all_include_paths() {
        bindings = bindings.clang_arg(format!("-I{}", include_path.display()));
    }

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .generate()
        .expect("failed to generate Nautilus bindings")
        .write_to_file(out_path.join("nautilus_bindings.rs"))
        .expect("failed to write Nautilus bindings");
}
