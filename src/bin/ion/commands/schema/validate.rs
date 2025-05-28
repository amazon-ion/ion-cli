use crate::ansi_codes::*;
use crate::commands::schema::validate::InputGrouping::{FileHandles, Lines, TopLevelValues};
use crate::commands::schema::IonSchemaCommandInput;
use crate::commands::{CommandIo, IonCliCommand, WithIonCliArgument};
use crate::input_grouping::InputGrouping;
use crate::output::CommandOutput;
use anyhow::{Error, Result};
use clap::builder::ArgPredicate;
use clap::{Arg, ArgAction, ArgMatches, Command};
use ion_rs::{
    ion_sexp, AnyEncoding, ElementReader, IonError, Reader, SequenceWriter, TextFormat, Writer,
};
use ion_rs::{v1_0, Element, ValueWriter};
use ion_schema::result::ValidationResult;
use ion_schema::violation::Violation;
use ion_schema::AsDocumentHint;
use std::io::{BufRead, Write};
use std::sync::LazyLock;
use termcolor::WriteColor;

pub struct ValidateCommand;

static HELP_EPILOGUE: LazyLock<String> = LazyLock::new(|| {
    format!(
        // '\' at the end of the line indicates that CLAP will handle the line wrapping.
        "\
All Ion Schema types are defined in the context of a schema document, so it is necessary to always \
have a schema document, even if that schema document is an implicit, empty schema. If a schema is \
not specified, the default is an implicit, empty Ion Schema 2.0 document.

{BOLD}{UNDERLINE}Example Usage:{NO_STYLE}

{UNDERLINE}Validating piped Ion data against an inline type definition{NO_STYLE}

~$ echo '{{foo:1}} {{bar:2}}' | ion schema -X validate -T '{{fields:{{foo:int,bar:string}}}}'

(valid )
(invalid (type_mismatched \"expected type String, found Int\" \"(bar)\" ) )

{UNDERLINE}Validating .ion files in a build script{NO_STYLE}

~$ ion schema -X validate -ER -f my_schema.isl my_type **/*.ion

a.ion ... ok
b/a.ion ... ok
b/b.ion ... FAILED
b/c.ion ... ok
c.ion ... FAILED

{ITALIC}NOTE: The output of this command is not intended to be machine-readable.{NO_STYLE}
"
    )
});

impl IonCliCommand for ValidateCommand {
    fn name(&self) -> &'static str {
        "validate"
    }

    fn about(&self) -> &'static str {
        "Validates an Ion value based on a given Ion Schema type."
    }

    fn is_stable(&self) -> bool {
        false
    }

    fn is_porcelain(&self) -> bool {
        true // TODO: This command should be made into plumbing, or we should add a plumbing equivalent.
    }

    fn configure_args(&self, command: Command) -> Command {
        command
            .after_help(HELP_EPILOGUE.as_str())
            // Positional args -- It is a breaking change to change the relative order of these args.
            .arg(IonSchemaCommandInput::type_arg().required(true))
            .with_input()
            // Non-positional args
            .args(IonSchemaCommandInput::schema_args())
            .args(InputGrouping::args())
            .with_output()
            .arg(
                Arg::new("error-on-invalid")
                    .long("error-on-invalid")
                    .short('E')
                    .default_value("false")
                    // "quiet" implies "error-on-invalid" so that the command always has some sort of useful output
                    .default_value_if("quiet", ArgPredicate::IsPresent, "true")
                    .action(ArgAction::SetTrue)
                    .help(
                        "Return a non-zero exit code when a value is invalid for the given type.",
                    ),
            )
            .arg(
                Arg::new("quiet")
                    .group("output-mode")
                    .short('q')
                    .long("quiet")
                    .help("Suppresses the violations output.")
                    .default_value("false")
                    .action(ArgAction::SetTrue),
            )
            .arg(
                Arg::new("report")
                    .group("output-mode")
                    .short('R')
                    .long("report")
                    .help("Prints a human-friendly, test-like report.")
                    .default_value("false")
                    .action(ArgAction::SetTrue),
            )
    }

    fn run(&self, _command_path: &mut Vec<String>, args: &ArgMatches) -> Result<()> {
        let ion_schema_input = IonSchemaCommandInput::read_from_args(args)?;
        let type_ref = ion_schema_input.get_type().unwrap();

        let grouping = InputGrouping::read_from_args(args);

        let quiet = args.get_flag("quiet");
        let report = args.get_flag("report");

        let mut all_valid = true;

        CommandIo::new(args)?.for_each_input(|output, input| {
            let input_name = input.name().to_string();
            // Output always uses 'lines' format so that we can have one output line per grouped input.
            // If the user wants something different, use 'ion cat' to change it.
            let mut writer = Writer::new(v1_0::Text.with_format(TextFormat::Lines), output)?;

            let mut result_writer = if report {
                ResultWriter::Report(input_name)
            } else if quiet {
                ResultWriter::Quiet
            } else {
                ResultWriter::Ion
            };

            match grouping {
                FileHandles => {
                    let document: Result<Vec<_>, _> = Reader::new(AnyEncoding, input.into_source())
                        .and_then(|r| r.into_elements().collect());
                    match document {
                        Ok(document) => {
                            let result = type_ref.validate(document.as_document());
                            all_valid &= result.is_ok();
                            result_writer.write_result(&mut writer, result)?;
                        }
                        Err(error) => {
                            all_valid = false;
                            result_writer.write_result(&mut writer, error)?;
                        }
                    }
                }
                Lines => {
                    for line in input.into_source().lines() {
                        let document = Element::read_all(line?);
                        match document {
                            Ok(document) => {
                                let result = type_ref.validate(document.as_document());
                                all_valid &= result.is_ok();
                                result_writer.write_result(&mut writer, result)?;
                            }
                            Err(error) => {
                                all_valid = false;
                                result_writer.write_result(&mut writer, error)?;
                            }
                        }
                    }
                }
                TopLevelValues => {
                    let reader = Reader::new(AnyEncoding, input.into_source())?;
                    for value in reader.into_elements() {
                        match value {
                            Ok(value) => {
                                let result = type_ref.validate(&value);
                                all_valid &= result.is_ok();
                                result_writer.write_result(&mut writer, result)?;
                            }
                            Err(error) => {
                                all_valid = false;
                                result_writer.write_result(&mut writer, error)?;
                            }
                        }
                    }
                }
            }
            writer.close()?;
            Ok(())
        })?;

        let exit_with_error_when_invalid =
            *args.get_one::<bool>("error-on-invalid").unwrap_or(&false);
        if !all_valid && exit_with_error_when_invalid {
            std::process::exit(1)
        } else {
            Ok(())
        }
    }
}

enum ResultKind {
    Ok,
    ValidationFailed(Violation),
    InputError(Error),
}
impl From<ValidationResult> for ResultKind {
    fn from(value: ValidationResult) -> Self {
        match value {
            Ok(()) => ResultKind::Ok,
            Err(e) => ResultKind::ValidationFailed(e),
        }
    }
}
impl From<IonError> for ResultKind {
    fn from(value: IonError) -> Self {
        ResultKind::InputError(value.into())
    }
}

enum ResultWriter {
    Quiet,
    Ion,
    Report(String),
}
impl ResultWriter {
    fn write_result<R: Into<ResultKind>>(
        &mut self,
        w: &mut Writer<v1_0::Text, &mut CommandOutput<'_>>,
        result: R,
    ) -> Result<()> {
        match self {
            ResultWriter::Quiet => Ok(()),
            ResultWriter::Ion => write_validation_result_ion(result.into(), w.value_writer()),
            ResultWriter::Report(name) => write_validation_report_line(name, w, result.into()),
        }
    }
}

/// Writes a validation result in the "report" style.
///
/// Format is: `<input name> ... <ok|FAILED>`
///
/// This is essentially like the individual lines from `cargo test`.
/// This output format is basically stable, but it is not intended to be machine-readable.
fn write_validation_report_line(
    input_name: &str,
    w: &mut Writer<v1_0::Text, &mut CommandOutput<'_>>,
    result: ResultKind,
) -> Result<()> {
    let output = w.output_mut();
    let (color, status) = match result {
        ResultKind::Ok => (GREEN, "ok".to_string()),
        ResultKind::ValidationFailed(_) => (RED, "FAILED".to_string()),
        ResultKind::InputError(error) => (RED, format!("ERROR: {}", error)),
    };
    if output.supports_color() {
        output.write_fmt(format_args!("{input_name} ... {color}{status}{NO_STYLE}\n"))?;
    } else {
        output.write_fmt(format_args!("{input_name} ... {status}\n"))?;
    }
    Ok(())
}

/// Writes the validation result
///
/// Current format is an s-expression that starts with the symbol 'valid' or 'invalid'.
/// If invalid, then it also contains an s-expression describing each violation.
/// This output is not (yet?) intended to be stable.
fn write_validation_result_ion<W: ValueWriter>(
    validation_result: ResultKind,
    writer: W,
) -> Result<()> {
    match validation_result {
        ResultKind::Ok => writer.write_sexp(vec![&Element::symbol("valid")]),
        ResultKind::InputError(error) => {
            writer.write_sexp(["error".to_string(), format!("{:?}", error)])?;
            Ok(())
        }
        ResultKind::ValidationFailed(violation) => {
            let mut violations: Vec<_> = vec![Element::symbol("invalid")];
            violation
                .flattened_violations()
                .iter()
                .map(|v| {
                    ion_sexp!(
                        Element::symbol(v.code().to_string())
                        Element::string(v.message().as_str())
                        Element::string(v.ion_path().to_string().as_str())
                    )
                })
                .map(ion_rs::Element::from)
                .for_each(|s| violations.push(s));

            writer.write_sexp(vec_of_refs(&violations))
        }
    }?;
    Ok(())
}

/// Transposes a borrowed vec of owned elements into an owned vec of borrowed elements.
fn vec_of_refs(the_vec: &[Element]) -> Vec<&Element> {
    the_vec.iter().collect()
}
