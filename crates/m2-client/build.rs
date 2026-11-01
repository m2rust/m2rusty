//! Lets the binary find libneoeter_python_abi.so without LD_LIBRARY_PATH.

fn main() {
    if let Ok(dir) = std::env::var("DEP_NEOETER_PYTHON_ABI_LIB_DIR") {
        println!("cargo:rustc-link-arg-bins=-Wl,-rpath,{dir}");
    }
}
