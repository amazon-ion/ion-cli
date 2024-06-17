# Code generation projects

This directory contains 2 projects that are used in tests for code generation and serve as an example of
how to use `ion-cli` code generator under the `generate` subcommand with an existing project.

## Table of contents

* [/input](#input)
* [/schema](#schema)
* [/java](#java)
    * [Gradle build process](#gradle-build-process)
    * [Tests](#tests)
    * [How to run the tests?](#how-to-run-the-tests)
* [/rust](#rust)
    * [Cargo build process](#cargo-build-process)
    * [Tests](#tests-1)
    * [How to run the tests?](#how-to-run-the-tests-1)

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

_Note: Code generation subcommand `generate` is under a feature flag. It is available
through `brew install ion-cli --HEAD` or `cargo install ion-cli --all-features`._

### Tests

The tests for the generated code are defined in `CodeGenTests.java`. It has the following tests:

- Tests for getter and setters of the generated code
- Roundtrip test for bad input Ion files which should result in Exception while reading.
- Roundtrip test for good input Ion files. Roundtrip has following steps:
    - Roundtrip test first read an Ion file into the generated model using `readFrom` API of the model
    - Then writes that model using `writeTo` API of the model.
    - Compares the written Ion data and original input Ion data.

### How to run the tests?

Here are the steps to follow for running tests:

1. Install ion-cli with either `brew install ion-cli --HEAD` or `cargo install ion-cli --all-features`.
    1. If you installed with brew then your executable is there in `ion` and you don't need to set up `ION_CLI`
       environment variable.
    2. If you installed with `cargo` then your executable would be in `$HOME/.cargo/bin` and you need to setup the
       environment variable `ION_CLI` to point to the executable's path. If you need latest commits from cargo which are
       not released yet, then do `cargo install ion-cli --all-features --git https://github.com/amazon-ion/ion-cli.git`.
2. All the tests uses an environment variable `ION_INPUT` which has the path to input Ion files. So if you want to
   test out this project locally set the environment variable `ION_INPUT` to point to `code-gen-projects/input.`_
3. `cd code-gen-projects/java/code-gen-demo`
4. Finally, to run the tests, just do:

```bash
ION_INPUT=../../input ./gradlew test
```

_Note: If you have used `cargo` and have to setup `ION_CLI` then
use `ION_CLI=$HOME/.cargo/bin/ion ION_INPUT=../../input ./gradlew test`._

At any point if gradle complains about error to write to the output directory then it might be because there is already
generated code in that directory(i.e. `code-gen-projects/java/code-gen-demo/build/generated/java/*`). So removing that
directory and then trying out (i.e. remove `generated/java` directory) should make it work.

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

_Note: Code generation subcommand `generate` is under a feature flag. It is available
through `brew install ion-cli --HEAD` or `cargo install ion-cli --all-features`._

### Tests

The tests for the generated code are defined in `tests` module in `lib.rs`. It has the following tests:

- Roundtrip test for bad input Ion files which should result in Exception while reading.
- Roundtrip test for good input Ion files. Roundtrip has following steps:
    - Roundtrip test first read an Ion file into the generated model using `readFrom` API of the model
    - Then writes that model using `writeTo` API of the model.
    - Compares the written Ion data and original input Ion data.

### How to run the tests?

Here are the steps to follow for running tests:

1. Install ion-cli with either `brew install ion-cli --HEAD` or `cargo install ion-cli --all-features`.
    1. If you installed with brew then your executable is there in `ion` and you need to setup the
       environment variable `ION_CLI` to point to the executable's path.
    2. If you installed with `cargo` then your executable would be in `$HOME/.cargo/bin` and you need to setup the
       environment variable `ION_CLI` to point to the executable's path. If you need latest commits from cargo which are
       not released yet, then do `cargo install ion-cli --all-features --git https://github.com/amazon-ion/ion-cli.git`.
2. `cd code-gen-projects/rust/code-gen-demo`
3. Finally, to run the tests, just do:

```bash
cargo test
```

_Note: If you have used `cargo` and have to setup `ION_CLI` then
use `ION_CLI=$HOME/.cargo/bin/ion cargo test`._