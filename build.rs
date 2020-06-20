use cmake::Config;
use std::fs::create_dir_all;
use std::path::Path;
use std::process;

const BUILD_DIR: &str = "./ion-c/build/release";

fn main() {
    let build_dir = Path::new(BUILD_DIR);    

    // Create the ion-c build directory if necessary
    if !build_dir.is_dir() {
        println!("Creating build directory {}", build_dir.display());
        if let Err(error) = create_dir_all(build_dir) {
            eprintln!("Could not create build directory: {:?}", error);
            process::exit(1);
        }
    }

    // Configure and run CMake
    Config::new("ion-c")
        .define("CMAKE_BUILD_TYPE", "Release")
        .out_dir("./ion-c/build/release")
        .build();

    // Output lines that start with "cargo:" are interpreted by Cargo. See the docs for details:
    // https://doc.rust-lang.org/cargo/reference/build-scripts.html#outputs-of-the-build-script

    // Only rebuild ion-c if that submodule directory is updated
    println!("cargo:rereun-if-changed={}", build_dir.display());
    // ...or if this build script is changed.
    println!("cargo:rereun-if-changed=build.rs");
}
