//! Where the C++ runtime of the H.264 decoder is, written into the binary when the build
//! environment names it. A system that has it on its own library path needs nothing here.
//! On Windows, the program's mark and name, which Explorer, the taskbar and search show.

fn main() {
    println!("cargo:rerun-if-env-changed=SURCO_CXX_RUNTIME");
    if let Ok(directory) = std::env::var("SURCO_CXX_RUNTIME") {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{directory}");
    }
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=assets/icon.ico");
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/icon.ico").set("FileDescription", "Surco").set("ProductName", "Surco");
        resource.compile().expect("the program's mark and name are written into it");
    }
}
