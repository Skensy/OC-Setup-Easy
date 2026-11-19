fn main() {
    println!("cargo:rerun-if-changed=native/window.m");
    cc::Build::new()
        .file("native/window.m")
        .flag("-fobjc-arc")
        .flag("-fblocks")
        .compile("window");
    for framework in ["Cocoa", "WebKit"] {
        println!("cargo:rustc-link-lib=framework={framework}");
    }
}
