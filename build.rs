use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=LIBCLANG_PATH");

    let nautilus = pkg_config::Config::new()
        .probe("libnautilus-extension-4")
        .expect("Nautilus development files are required to build this extension");

    let mut builder = bindgen::Builder::default()
        .header_contents("wrapper.h", "#include <nautilus-extension.h>")
        .allowlist_function(
            "^(g_object_get_type|g_type_module_(add_interface|register_type)|nautilus_(menu_provider_get_type|module_(initialize|shutdown|list_types)))$",
        )
        .allowlist_type(
            "^(GInterfaceInfo|GList|GType|GTypeFlags|GTypeInfo|GTypeModule|GObject|GObjectClass|NautilusFileInfo|NautilusMenuProvider|NautilusMenuProviderInterface)$",
        )
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()));

    for include_path in nautilus.include_paths {
        builder = builder.clang_arg(format!("-I{}", include_path.display()));
    }

    let bindings = builder
        .generate()
        .expect("failed to generate Nautilus and GLib bindings with bindgen");
    let output = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set"));
    bindings
        .write_to_file(output.join("nautilus_bindings.rs"))
        .expect("failed to write generated Nautilus bindings");
}
