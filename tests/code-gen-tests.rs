use anyhow::Result;
use assert_cmd::Command;
use rstest::rstest;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use tempfile::TempDir;

/// Returns a new [PathBuf] instance with the absolute path of the "code-gen-projects" directory.
fn code_gen_projects_path() -> PathBuf {
    PathBuf::from_iter([env!("CARGO_MANIFEST_DIR"), "code-gen-projects"])
}

#[test]
fn roundtrip_tests_for_generated_code_gradle() -> Result<()> {
    // run the gradle project defined under `code-gen-projects`,
    // this project runs the code generator in its build process and generates code,
    // this project also has some predefined tests for the generated code,
    // so simply running the tests on this project builds the project, generates code and runs tests

    // absolute paths for gradle project and executables
    let ion_executable = env!("CARGO_BIN_EXE_ion");
    let ion_input = code_gen_projects_path().join("input");
    let test_project_path = code_gen_projects_path().join("java").join("code-gen-demo");

    let gradle_executable_name = if cfg!(windows) {
        "gradlew.bat"
    } else {
        "gradlew"
    };

    let gradle_executable = test_project_path.join(gradle_executable_name);

    // Clean and Test
    let gradle_output = std::process::Command::new(gradle_executable)
        .current_dir(test_project_path)
        .env("ION_CLI", ion_executable)
        .env("ION_INPUT", ion_input)
        .arg("clean")
        .arg("test")
        .output()
        .expect("failed to execute Gradle targets 'clean' and 'test'");

    println!("status: {}", gradle_output.status);
    std::io::stdout().write_all(&gradle_output.stdout).unwrap();
    std::io::stderr().write_all(&gradle_output.stderr).unwrap();

    assert!(gradle_output.status.success());
    Ok(())
}

#[test]
fn roundtrip_tests_for_generated_code_cargo() -> Result<()> {
    // run the cargo project defined under `code-gen-projects`,
    // this project runs the code generator in its build process and generates code,
    // this project also has some predefined tests for the generated code,
    // so simply running the tests on this project builds the project, generates code and runs tests

    // absolute paths for crate and executables
    let ion_executable = env!("CARGO_BIN_EXE_ion");
    let test_project_path = code_gen_projects_path().join("rust").join("code-gen-demo");
    let cargo_executable = env!("CARGO");

    // Clean
    let cargo_clean_output = std::process::Command::new(cargo_executable)
        .current_dir(&test_project_path)
        .arg("clean")
        .output()
        .expect("failed to execute 'cargo clean'");

    println!("Cargo clean status: {}", cargo_clean_output.status);
    std::io::stdout()
        .write_all(&cargo_clean_output.stdout)
        .unwrap();
    std::io::stderr()
        .write_all(&cargo_clean_output.stderr)
        .unwrap();

    // Test
    let cargo_test_output = std::process::Command::new(cargo_executable)
        .current_dir(&test_project_path)
        .arg("test")
        .env("ION_CLI", ion_executable)
        .output()
        .expect("failed to execute 'cargo test'");

    println!("Cargo test status: {}", cargo_test_output.status);
    std::io::stdout()
        .write_all(&cargo_test_output.stdout)
        .unwrap();
    std::io::stderr()
        .write_all(&cargo_test_output.stderr)
        .unwrap();

    assert!(cargo_test_output.status.success());
    Ok(())
}

//TODO: Add cargo roundtrip tests once the rust templates are modified based on new code generation model

#[rstest]
#[case::any_element_list(
r#"
        type::{
         name: any_element_list,
         type: list, // this doesn't specify the type for elements in the list with `element` constraint
        }
    "#,
)]
#[case::any_sequence_type(
    r#"
        type::{
         name: any_sequence_type,
         element: int, // this doesn't specify the type of sequence with `type` constraint
        }
    "#
)]
// Currently any struct type is not supported, it requires having a `fields` constraint
#[case::any_struct_type(
    r#"
        type::{
         name: any_struct_type,
         type: struct, // this doesn't specify `fields` of the struct
        }
    "#
)]
/// Calls ion-cli generate with different unsupported schema types. Verify that `generate` subcommand returns an error for these schema types.
fn test_unsupported_schema_types_failures(#[case] test_schema: &str) -> Result<()> {
    let mut cmd = Command::cargo_bin("ion")?;
    let temp_dir = TempDir::new()?;
    let input_schema_path = temp_dir.path().join("test_schema.isl");
    let mut input_schema_file = File::create(input_schema_path)?;
    input_schema_file.write_all(test_schema.as_bytes())?;
    input_schema_file.flush()?;
    cmd.args([
        "-X",
        "generate",
        "--schema",
        "test_schema.isl",
        "--output",
        temp_dir.path().to_str().unwrap(),
        "--language",
        "java",
        "--namespace",
        "org.example",
        "--directory",
        temp_dir.path().to_str().unwrap(),
    ]);
    let command_assert = cmd.assert();
    // Code generation process should return an error for unsupported schema types
    command_assert.failure();
    Ok(())
}
