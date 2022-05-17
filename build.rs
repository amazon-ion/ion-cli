use cmake::Config;
use std::env;
use std::fs::create_dir_all;
use std::path::Path;
use std::process;

fn main() {
    // Cargo requires that build scripts only modify the contents of the folder $OUT_DIR.
    // Here we construct a directory inside $OUT_DIR to store the output of the ion-c build process.
    let out_dir = env::var("OUT_DIR").unwrap();
    let ion_c_release_dir = format!("{}/ion-c/build/release", &out_dir);
    let ion_c_release_dir = ion_c_release_dir.as_str();
    let ion_c_release_path = Path::new(ion_c_release_dir);
    println!("ion-c build directory: {}", ion_c_release_dir);

    // Create the ion-c build directory if necessary
    if !ion_c_release_path.is_dir() {
        println!("Creating build directory {}", ion_c_release_dir);
        if let Err(error) = create_dir_all(ion_c_release_path) {
            eprintln!("Could not create build directory: {:?}", error);
            process::exit(1);
        }
    }

    // Configure and run CMake
    Config::new("ion-c")
        .define("CMAKE_BUILD_TYPE", "Release")
        .out_dir(&ion_c_release_path)
        .build();

    // Output lines that start with "cargo:" are interpreted by Cargo. See the docs for details:
    // https://doc.rust-lang.org/cargo/reference/build-scripts.html#outputs-of-the-build-script

    // The `ion` executable statically links to the `ion-c` CLI. The following output tells Cargo
    // which libraries to link against and in which directories they can be found.

    // ion_events library
    println!(
        "cargo:rustc-link-search=native={}/build/tools/events",
        ion_c_release_dir
    );
    println!("cargo:rustc-link-lib=static=ion_events_static");

    // ion_c library
    println!(
        "cargo:rustc-link-search=native={}/build/ionc",
        ion_c_release_dir
    );
    println!("cargo:rustc-link-lib=static=ionc_static");

    // decNumber library
    println!(
        "cargo:rustc-link-search=native={}/build/decNumber",
        ion_c_release_dir
    );
    println!("cargo:rustc-link-lib=static=decNumber_static");

    // C++ library
    let target = env::var("TARGET").unwrap();
    if target.contains("apple") {
        // macOS users use libc++
        println!("cargo:rustc-link-lib=dylib=c++");
    } else if target.contains("linux") {
        // GCC users
        println!("cargo:rustc-link-lib=dylib=stdc++");
    } else {
        // TODO support Windows/Unixes/etc. correctly
        unimplemented!(
            "Linking C++ is not yet supported on this platform {}",
            target
        );
    }

    // ion-c CLI library
    println!(
        "cargo:rustc-link-search=native={}/build/tools/cli/",
        ion_c_release_dir
    );
    println!("cargo:rustc-link-lib=static=ion_cli_main");

    // Only rebuild ion-c if that submodule directory is updated
    println!("cargo:rerun-if-changed=./ion-c");
    // ...or if this build script is changed.
    println!("cargo:rerun-if-changed=build.rs");
}
