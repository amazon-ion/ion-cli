use crate::commands::IonCliCommand;
use anyhow::{Context, Result};
use clap::{Arg, ArgAction, ArgMatches, Command};
use ion_rs::{v1_0, Element, Sequence, SequenceWriter, StructWriter, TextFormat, Writer};
use ion_schema::authority::{DocumentAuthority, FileSystemDocumentAuthority};
use ion_schema::system::SchemaSystem;
use std::fs;
use std::path::Path;
use std::str::from_utf8;

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
        true // TODO: Should this command be made into plumbing?
    }

    fn configure_args(&self, command: Command) -> Command {
        command
            .arg(
                // Input ion file can be specified by the "-i" or "--input" flags.
                Arg::new("input")
                    .long("input")
                    .short('i')
                    .required(true)
                    .value_name("INPUT_FILE")
                    .help("Input file containing the Ion values to be validated"),
            )
            .arg(
                // Schema file can be specified by the "-s" or "--schema" flags.
                Arg::new("schema")
                    .long("schema")
                    .short('s')
                    .required(true)
                    .value_name("SCHEMA")
                    .help("The Ion Schema file to load"),
            )
            .arg(
                // Directory(s) that will be used as authority(s) for schema system
                Arg::new("directories")
                    .long("directory")
                    .short('d')
                    .action(ArgAction::Append)
                    .value_name("DIRECTORY")
                    .required(true)
                    .help("One or more directories that will be searched for the requested schema"),
            )
            .arg(
                // Schema Type can be specified by the "-t" or "--type" flags.
                Arg::new("type")
                    .long("type")
                    .short('t')
                    .required(true)
                    .value_name("TYPE")
                    .help("Name of schema type from given schema that needs to be used for validation"),
            )
    }

    fn run(&self, _command_path: &mut Vec<String>, args: &ArgMatches) -> Result<()> {
        // Extract the user provided document authorities/ directories
        let authorities: Vec<&String> = args.get_many("directories").unwrap().collect();

        // Extract schema file provided by user
        let schema_id = args.get_one::<String>("schema").unwrap();

        // Extract the schema type provided by user
        let schema_type = args.get_one::<String>("type").unwrap();

        // Extract Ion value provided by user
        let input_file = args.get_one::<String>("input").unwrap();
        let value =
            fs::read(input_file).with_context(|| format!("Could not open '{}'", schema_id))?;
        let elements: Sequence = Element::read_all(value)
            .with_context(|| format!("Could not parse Ion file: '{}'", schema_id))?;

        // Set up document authorities vector
        let mut document_authorities: Vec<Box<dyn DocumentAuthority>> = vec![];

        for authority in authorities {
            document_authorities.push(Box::new(FileSystemDocumentAuthority::new(Path::new(
                authority,
            ))))
        }

        // Create a new schema system from given document authorities
        let mut schema_system = SchemaSystem::new(document_authorities);

        // load schema
        let schema = schema_system.load_schema(schema_id);

        // get the type provided by user from the schema file
        let type_ref = schema?
            .get_type(schema_type)
            .with_context(|| format!("Schema {} does not have type {}", schema_id, schema_type))?;

        // create a text writer to make the output
        let mut writer = Writer::new(v1_0::Text.with_format(TextFormat::Pretty), vec![])?;

        // validate owned_elements according to type_ref
        for owned_element in elements {
            // create a validation report with validation result, value, schema and/or violation
            let mut struct_writer = writer.struct_writer()?;
            let validation_result = type_ref.validate(&owned_element);
            match validation_result {
                Ok(_) => {
                    struct_writer.write("result", "Valid")?;
                    struct_writer.write("value", format!("{}", &owned_element))?;
                    struct_writer.write("schema", schema_id)?;
                }
                Err(error) => {
                    struct_writer.write("result", "Invalid")?;
                    struct_writer.write("violation", format!("{:#?}", error))?;
                }
            }
        }
        println!("Validation report:");
        println!("{}", from_utf8(writer.output()).unwrap());
        Ok(())
    }
}
