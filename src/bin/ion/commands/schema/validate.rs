use crate::commands::schema::validate::InputGrouping::{FileHandles, Lines, TopLevelValues};
use crate::commands::schema::IonSchemaCommandInput;
use crate::commands::{CommandIo, IonCliCommand, WithIonCliArgument};
use crate::input_grouping::InputGrouping;
use anyhow::Result;
use clap::builder::ArgPredicate;
use clap::{Arg, ArgAction, ArgMatches, Command};
use ion_rs::{ion_sexp, AnyEncoding, ElementReader, Reader, SequenceWriter, TextFormat, Writer};
use ion_rs::{v1_0, Element, IonResult, ValueWriter};
use ion_schema::result::ValidationResult;
use ion_schema::AsDocumentHint;
use std::io::BufRead;

pub struct ValidateCommand;

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
            .after_help(
                "All Ion Schema types are defined in the context of a schema document, so it is necessary to always \
                have a schema document, even if that schema document is an implicit, empty schema. If a schema is \
                not specified, the default is an implicit, empty Ion Schema 2.0 document."
            )
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
                    .help("Return a non-zero exit code when a value is invalid for the given type.")
            )
            .arg(
                Arg::new("quiet")
                    .short('q')
                    .long("quiet")
                    .help("Suppresses the violations output.")
                    .default_value("false")
                    .action(ArgAction::SetTrue)
            )
    }

    fn run(&self, _command_path: &mut Vec<String>, args: &ArgMatches) -> Result<()> {
        let ion_schema_input = IonSchemaCommandInput::read_from_args(args)?;
        let type_ref = ion_schema_input.get_type().unwrap();

        let grouping = InputGrouping::read_from_args(args);

        let quiet = args.get_flag("quiet");

        let mut all_valid = true;

        CommandIo::new(args).for_each_input(|output, input| {
            // Output always uses 'lines' format so that we can have one output line per grouped input.
            // If the user wants something different, use 'ion cat' to change it.
            let mut writer = Writer::new(v1_0::Text.with_format(TextFormat::Lines), output)?;

            match grouping {
                FileHandles => {
                    let reader = Reader::new(AnyEncoding, input.into_source())?;
                    let document: Vec<_> = reader.into_elements().collect::<IonResult<_>>()?;
                    let result = type_ref.validate(document.as_document());
                    all_valid &= result.is_ok();
                    if !quiet {
                        write_validation_result(result, writer.value_writer())?;
                    }
                }
                Lines => {
                    for line in input.into_source().lines() {
                        let document = Element::read_all(line?)?;
                        let result = type_ref.validate(document.as_document());
                        all_valid &= result.is_ok();
                        if !quiet {
                            write_validation_result(result, writer.value_writer())?;
                        }
                    }
                }
                TopLevelValues => {
                    let reader = Reader::new(AnyEncoding, input.into_source())?;
                    for value in reader.into_elements() {
                        let result = type_ref.validate(&value?);
                        all_valid &= result.is_ok();
                        if !quiet {
                            write_validation_result(result, writer.value_writer())?;
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

/// Writes the validation result
///
/// Current format is an s-expression that starts with the symbol 'valid' or 'invalid'.
/// If invalid, then it also contains an s-expression describing each violation.
/// This output is not (yet?) intended to be stable.
fn write_validation_result<W: ValueWriter>(
    validation_result: ValidationResult,
    writer: W,
) -> IonResult<()> {
    match validation_result {
        Ok(_) => writer.write_sexp(vec![&Element::symbol("valid")]),
        Err(violation) => {
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
    }
}

/// Transposes a borrowed vec of owned elements into an owned vec of borrowed elements.
fn vec_of_refs(the_vec: &Vec<Element>) -> Vec<&Element> {
    the_vec.iter().collect()
}
