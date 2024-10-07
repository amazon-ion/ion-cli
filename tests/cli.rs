use anyhow::Result;
use assert_cmd::Command;
use ion_rs::Element;
use rstest::*;
use std::fs::File;
use std::io::{Read, Write};
use std::time::Duration;
use tempfile::TempDir;

enum FileMode {
    /// Use `STDIN` or `STDOUT`
    Default,
    /// Use a named file
    Named,
}

enum InputCompress {
    /// no compression
    No,
    /// gzip
    Gz,
    /// zstd
    Zst,
}

struct TestCase<S: AsRef<str>> {
    /// The text of the ion payload to test
    ion_text: S,
    /// The expected Ion
    expected_ion: Element,
}

impl From<(&'static str, &'static str)> for TestCase<&'static str> {
    /// Simple conversion for static `str` slices into a test case
    fn from((ion_text, expected_ion): (&'static str, &'static str)) -> Self {
        let expected_ion = Element::read_one(expected_ion.as_bytes()).unwrap();
        Self {
            ion_text,
            expected_ion,
        }
    }
}

#[rstest]
#[case::simple((
r#"
{
  name: "Fido",

  age: years::4,

  birthday: 2012-03-01T,

  toys: [
    ball,
    rope,
  ],

  weight: pounds::41.2,

  buzz: {{VG8gaW5maW5pdHkuLi4gYW5kIGJleW9uZCE=}},
}
"#,
r#"
{
  name: "Fido",

  age: years::4,

  birthday: 2012-03-01T,

  toys: [
    ball,
    rope,
  ],

  weight: pounds::41.2,

  buzz: {{VG8gaW5maW5pdHkuLi4gYW5kIGJleW9uZCE=}},
}
"#
).into())]
/// Calls the ion CLI binary dump command with a set of arguments the ion-cli is expected to support.
/// This does not verify specific formatting, only basic CLI behavior.
fn run_it<S: AsRef<str>>(
    #[case] test_case: TestCase<S>,
    #[values("", "binary", "text", "pretty")] format_flag: &str,
    #[values(FileMode::Default, FileMode::Named)] input_mode: FileMode,
    #[values(FileMode::Default, FileMode::Named)] output_mode: FileMode,
    #[values(InputCompress::No, InputCompress::Gz, InputCompress::Zst)]
    input_compress: InputCompress,
) -> Result<()> {
    let TestCase {
        ion_text,
        expected_ion,
    } = test_case;

    let temp_dir = TempDir::new()?;
    let input_path = temp_dir.path().join("INPUT.ion");
    let output_path = temp_dir.path().join("OUTPUT.ion");

    let mut cmd = Command::cargo_bin("ion")?;
    cmd.arg("dump").timeout(Duration::new(5, 0));
    if !format_flag.is_empty() {
        cmd.arg("-f");
        cmd.arg(format_flag);
    }
    match output_mode {
        FileMode::Default => {
            // do nothing
        }
        FileMode::Named => {
            // tell driver to output to a file
            cmd.arg("-o");
            cmd.arg(&output_path);
        }
    };

    // prepare input
    let input_bytes = match input_compress {
        InputCompress::Gz => {
            let mut encoder =
                flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
            encoder.write_all(ion_text.as_ref().as_bytes())?;
            encoder.finish()?
        }
        InputCompress::Zst => {
            let mut encoder = zstd::stream::write::Encoder::new(Vec::new(), 1)?;
            encoder.write_all(ion_text.as_ref().as_bytes())?;
            encoder.finish()?
        }
        _ => ion_text.as_ref().as_bytes().to_vec(),
    };

    match input_mode {
        FileMode::Default => {
            // do nothing
            cmd.write_stdin(input_bytes);
        }
        FileMode::Named => {
            // dump our test data to input file
            let mut input_file = File::create(&input_path)?;
            input_file.write_all(&input_bytes)?;
            input_file.flush()?;

            // TODO: test multiple input files

            // make this the input for our driver
            cmd.arg(input_path.to_str().unwrap());
        }
    };

    let assert = cmd.assert();

    let actual_ion = match output_mode {
        FileMode::Default => {
            let output = assert.get_output();
            Element::read_one(&output.stdout)?
        }
        FileMode::Named => {
            let mut output_file = File::open(output_path)?;
            let mut output_buffer = vec![];
            output_file.read_to_end(&mut output_buffer)?;
            Element::read_one(&output_buffer)?
        }
    };

    assert_eq!(expected_ion, actual_ion);
    assert.success();

    Ok(())
}

#[rstest]
#[case(0, "")]
#[case(2, "{foo: bar, abc: [123, 456]}\n{foo: baz, abc: [42.0, 4.3e1]}")]
///Calls ion-cli head with different requested number. Pass the test if the return value equals to the expected value.
fn test_write_all_values(#[case] number: i32, #[case] expected_output: &str) -> Result<()> {
    let mut cmd = Command::cargo_bin("ion")?;
    let test_data = r#"
    {
        foo: bar,
        abc: [123, 456]
    }
    {
        foo: baz,
        abc: [42.0, 43e0]
    }
    {
        foo: bar,
        test: data
    }
    {
        foo: baz,
        type: struct
    }
    "#;
    let temp_dir = TempDir::new()?;
    let input_path = temp_dir.path().join("test.ion");
    let mut input_file = File::create(&input_path)?;
    input_file.write_all(test_data.as_bytes())?;
    input_file.flush()?;
    cmd.args([
        "head",
        "--values",
        &number.to_string(),
        "--format",
        "lines",
        input_path.to_str().unwrap(),
    ]);
    let command_assert = cmd.assert();
    let output = command_assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        Element::read_all(stdout.trim_end())?,
        Element::read_all(expected_output)?
    );
    Ok(())
}

mod code_gen_tests {
    use super::*;
    use std::fs;

    //TODO: Add cargo roundtrip tests once the rust templates are modified based on new code generation model

    #[rstest]
    #[case(
    "SimpleStruct",
    r#"
        type::{
         name: simple_struct,
         fields: {
            name: string,
            id: { type: int, occurs: required },
         }
        }
    "#,
    & ["private int id;", "private java.util.Optional<String> name;"],
    & ["public java.util.Optional<String> getName() {", "public int getId() {"]
    )]
    #[case(
    "Scalar",
    r#"
        type::{
         name: scalar,
         type: string
        }
    "#,
    & ["private String value;"],
    & ["public String getValue() {"]
    )]
    /// Calls ion-cli generate with different schema file. Pass the test if the return value contains the expected properties and accessors.
    fn test_code_generation_in_java(
        #[case] test_name: &str,
        #[case] test_schema: &str,
        #[case] expected_properties: &[&str],
        #[case] expected_accessors: &[&str],
    ) -> Result<()> {
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
        let output_file_path = temp_dir.path().join(format!("{}.java", test_name));
        command_assert.success();
        let contents =
            fs::read_to_string(output_file_path).expect("Can not read generated code file.");
        for expected_property in expected_properties {
            assert!(contents.contains(expected_property));
        }
        for expected_accessor in expected_accessors {
            assert!(contents.contains(expected_accessor));
        }
        Ok(())
    }
}
