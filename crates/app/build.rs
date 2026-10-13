//! Where the C++ runtime of the H.264 decoder is, written into the binary when the build
//! environment names it. A system that has it on its own library path needs nothing here.

fn main() {
    println!("cargo:rerun-if-env-changed=SURCO_CXX_RUNTIME");
    if let Ok(directory) = std::env::var("SURCO_CXX_RUNTIME") {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{directory}");
    }
}
