use std::{env, path::PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let core_dir = manifest_dir.join("../../vendor/openvpn3");
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());

    cc::Build::new()
        .cpp(true)
        .std("c++20")
        .define("ASIO_STANDALONE", None)
        .define("USE_ASIO", None)
        .include(&core_dir)
        .file("wrapper.cc")
        .compile("openvpn_profile_merge");

    println!("cargo:rustc-link-lib=fmt");

    bindgen::Builder::default()
        .header("wrapper.h")
        .allowlist_function("ovpn_profile_merge.*")
        .allowlist_type("OvpnProfileMergeResult")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("failed to generate OpenVPN profile merge bindings")
        .write_to_file(out_dir.join("bindings.rs"))
        .expect("failed to write OpenVPN profile merge bindings");

    println!("cargo:rerun-if-changed=wrapper.h");
    println!("cargo:rerun-if-changed=wrapper.cc");
    println!(
        "cargo:rerun-if-changed={}",
        core_dir.join("openvpn/options/merge.hpp").display()
    );
}
