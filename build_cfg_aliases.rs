fn emit_cfg_aliases() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../build_cfg_aliases.rs");

    println!("cargo::rustc-check-cfg=cfg(has_database)");
    if cfg!(any(
        feature = "sqlite",
        feature = "postgres",
        feature = "mysql"
    )) {
        println!("cargo:rustc-cfg=has_database");
    }

    println!("cargo::rustc-check-cfg=cfg(has_http)");
    if cfg!(feature = "http") {
        println!("cargo:rustc-cfg=has_http");
    }
}
