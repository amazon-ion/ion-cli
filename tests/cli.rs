use anyhow::Result;
use ion_rs::value::owned::OwnedElement;
use ion_rs::value::reader::*;
use rstest::*;
use std::fs::File;
use std::io::{Read, Write};
use std::time::Duration;
use tempfile::TempDir;
use assert_cmd::Command;

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
    expected_ion: OwnedElement,
}

impl From<(&'static str, &'static str)> for TestCase<&'static str> {
    /// Simple conversion for static `str` slices into a test case
    fn from((ion_text, expected_ion): (&'static str, &'static str)) -> Self {
        let expected_ion = element_reader().read_one(expected_ion.as_bytes()).unwrap();
        Self {
            ion_text,
            expected_ion
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
fn run_it<S: AsRef<str>>(
    #[case] test_case: TestCase<S>,
#[values("", "binary", "text", "pretty")] format_flag: &str,
#[values(FileMode::Default, FileMode::Named)] input_mode: FileMode,
#[values(FileMode::Default, FileMode::Named)] output_mode: FileMode
) -> Result<()> {

    let TestCase {
        ion_text,
        expected_ion
    } = test_case;

    let temp_dir = TempDir::new()?;
    let input_path = temp_dir.path().join("INPUT.ion");
    let output_path = temp_dir.path().join("OUTPUT.ion");

    let mut cmd = Command::cargo_bin("ion")?;
    cmd.arg("dump").timeout(Duration::new(5, 0));
    if format_flag != "" {
        cmd.arg("-f");
        cmd.arg(format_flag);
    }
    match output_mode {
        FileMode::Default => {
            // do nothing
        },
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
        },
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
            element_reader().read_one(&output.stdout)?
        }
        FileMode::Named => {
            let mut output_file = File::open(output_path)?;
            let mut output_buffer = vec![];
            output_file.read_to_end(&mut output_buffer)?;
            element_reader().read_one(&output_buffer)?
        }
    };

    assert_eq!(expected_ion, actual_ion);
    assert.success();

    Ok(())
}
