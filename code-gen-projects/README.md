# Code generation projects

This directory contains 2 projects that are used in tests for code generation and serve as an example of
how to use `ion-cli` code generator under the `generate` subcommand with an existing project.

## Table of contents

* [/input](#input)
* [/schema](#schema)
* [/java](#java)
    * [Gradle build process](#gradle-build-process)
* [/rust](#rust)
    * [Cargo build process](#cargo-build-process)

## /input

This directory contains some good and bad test Ion files based on corresponding schema in `/schema`.

## /schema

This directory contains all the schema files used in testing code generation with `ion-cli` `generate` subcommand.

## /java

This directory contains a Java project called `code-gen-demo` which is a gradle project which has tests that uses the
generated code based
on schema file provided in `/schema` and test Ion file provided in `/input`.

### Gradle build process

To generate code as part of the build process of this project, a gradle build task is defined inside `build.gradle.kts`.
This task performs following steps:

- Gets the executable path for `ion-cli` through an environment variable `ION_CLI`. If the environment variable is not
  set then it uses the local executable named `ion`.
- Sets the schema directory as `/schema` which will be used by `generate` subcommand to generate code for the schema
  files inside it.
- Sets the path to output directory where the code will be generated and sets it as source directory.
- It runs the `ion-cli` `generate` subcommand with the set schema directory and a namespace where the code will be
  generated.

Following is a sample build task you can add in an existing gradle project to generate code for your schemas,

```
val ionSchemaSourceCodeDir = "YOUR_SOURCE_SCHEMA_DIRECTORY"
val generatedIonSchemaModelDir = "${layout.buildDirectory.get()}/generated/java"
sourceSets {
    main {
        java.srcDir(generatedIonSchemaModelDir)
    }
}


tasks {
    val ionCodegen = create<Exec>("ionCodegen") {
        inputs.files(ionSchemaSourceCodeDir)
        outputs.file(generatedIonSchemaModelDir)

        val ionCli = System.getenv("ION_CLI") ?: "ion"

        commandLine(ionCli)
            .args(
                "beta", "generate",
                "-l", "java",
                "-n", "NAMESPACE_FOR_GENERATED_CODE",
                "-d", ionSchemaSourceCodeDir,
                "-o", generatedIonSchemaModelDir,
            )
            .workingDir(rootProject.projectDir)
    }

    withType<JavaCompile> {
        options.encoding = "UTF-8"
        if (JavaVersion.current() != JavaVersion.VERSION_1_8) {
            options.release.set(8)
        }
        dependsOn(ionCodegen)
    }
}
```
_Note: Code generation subcommand `generate` is under a feature flag. It is available through `brew install ion-cli --HEAD` or `cargo install ion-cli --all-features`._

## /rust

This directory contains a Rust project called `code-gen-demo` which is a cargo project which has tests that uses the
generated code based
on schema file provided in `/schema` and test Ion file provided in `/input`.

### Cargo build process

To generate code as part of the build process of this cargo project, a cargo build script is defined in `build.rs`.
This task performs following steps:

- Gets the executable path for `ion-cli` through an environment variable `ION_CLI`. If the environment variable is not
  set then it uses the local executable named `ion`.
- Sets the schema directory as `/schema` which will be used by `generate` subcommand to generate code for the schema
  files inside it.
- Sets the path to output directory where the code will be generated (e.g. `OUT_DIR`).
- It runs the `ion-cli` `generate` subcommand with the set schema directory and a namespace where the code will be
  generated.

Following is sample build script you can add in your existing cargo project to generate code using `ion-cli`.

```rust
fn main() {
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_dir = env::var("OUT_DIR").unwrap();

    // Invokes the ion-cli executable using environment variable ION_CLI if present, otherwise uses local executable named `ion`
    let ion_cli = env::var("ION_CLI").unwrap_or("ion".to_string());
    println!("cargo:warn=Running command: {}", ion_cli);
    let mut cmd = std::process::Command::new(ion_cli);
    cmd.arg("beta")
        .arg("generate")
        .arg("-l")
        .arg("rust")
        .arg("-d")
        .arg("YOUR_SOURCE_SCHEMA_DIRECTORY")
        .arg("-o")
        .arg(&out_dir);

    println!("cargo:warn=Running: {:?}", cmd);

    let output = cmd.output().expect("failed to execute process");

    println!("status: {}", output.status);
    io::stdout().write_all(&output.stdout).unwrap();
    io::stderr().write_all(&output.stderr).unwrap();

    assert!(output.status.success());

    println!("cargo:rerun-if-changed=input/");
    println!("cargo:rerun-if-changed=schema/");
}
```
_Note: Code generation subcommand `generate` is under a feature flag. It is available through `brew install ion-cli --HEAD` or `cargo install ion-cli --all-features`._
