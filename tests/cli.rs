use anyhow::Result;
use assert_cmd::Command;
use ion_rs::element::Element;
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

    match input_mode {
        FileMode::Default => {
            // do nothing
            cmd.write_stdin(ion_text.as_ref());
        }
        FileMode::Named => {
            // dump our test data to input file
            let mut input_file = File::create(&input_path)?;
            input_file.write(ion_text.as_ref().as_bytes())?;
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
///Calls ion-cli beta head with different requested number. Pass the test if the return value equals to the expected value.
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
    input_file.write(test_data.as_bytes())?;
    input_file.flush()?;
    cmd.args([
        "beta",
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
    assert_eq!(stdout.trim_end(), expected_output);
    Ok(())
}
