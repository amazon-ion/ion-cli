use cmake::Config;
use std::fs::create_dir_all;
use std::path::Path;
use std::process;

fn main() {
    let build_dir = Path::new("./ion-c/build/release");

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

    // The `ion` executable statically links to the `ion-c` CLI. The following output tells Cargo
    // which libraries to link against and in which directories they can be found.

    // ion_events library
    println!("cargo:rustc-link-search=native=./ion-c/build/release/build/tools/events");
    println!("cargo:rustc-link-lib=static=ion_events_static");

    // ion_c library
    println!("cargo:rustc-link-search=native=./ion-c/build/release/build/ionc");
    println!("cargo:rustc-link-lib=static=ionc_static");

    // decNumber library
    println!("cargo:rustc-link-search=native=./ion-c/build/release/build/decNumber");
    println!("cargo:rustc-link-lib=static=decNumberStatic");

    // C++ library
    println!("cargo:rustc-link-search=native=/usr/lib");
    println!("cargo:rustc-link-lib=c++");

    // ion-c CLI library
    println!("cargo:rustc-link-search=native=./ion-c/build/release/build/tools/cli/");
    println!("cargo:rustc-link-lib=static=ion_cli_main");

    // Only rebuild ion-c if that submodule directory is updated
    println!("cargo:rereun-if-changed={}", build_dir.display());
    // ...or if this build script is changed.
    println!("cargo:rereun-if-changed=build.rs");
}
