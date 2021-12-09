use anyhow::{Result};
use clap::{App, Arg, ArgMatches};
use ion_schema_rust::authority::{DocumentAuthority, FileSystemDocumentAuthority};
use std::path::Path;
use ion_schema_rust::system::SchemaSystem;

const ABOUT: &str = "Loads an Ion Schema file using user provided schema id and returns a result message. Shows an error message if there were any invalid schema syntax found during the load process";

// Creates a `clap` (Command Line Arguments Parser) configuration for the `load` command.
// This function is invoked by the `load` command's parent `schema`, so it can describe its
// child commands.
pub fn app() -> App<'static, 'static> {
    App::new("load")
        .about(ABOUT)
        .arg(
            // Input file can be specified by the "-s" or "--schema" flags.
            Arg::with_name("schema")
                .long("schema")
                .short("s")
                .required(true)
                .takes_value(true)
                .value_name("SCHEMA")
                .help("The Ion Schema file to load"),
        )
        .arg(
            // Directory(s) that will be used as authority(s) for schema system
            Arg::with_name("directories")
                .long("directory")
                .short("d")
                .min_values(1)
                .takes_value(true)
                .multiple(true)
                .value_name("DIRECTORY")
                .required(true)
                .help("One or more directories that will be searched for the requested schema"),
        )
}

// This function is invoked by the `load` command's parent `schema`.
pub fn run(_command_name: &str, matches: &ArgMatches<'static>) -> Result<()> {
    // Extract the user provided document authorities/ directories
    let authorities: Vec<_> = matches.values_of("directories").unwrap().collect();

    // Extract schema file provided by user
    let schema_id = matches.value_of("schema").unwrap();

    // Set up document authorities vector
    let mut document_authorities: Vec<Box<dyn DocumentAuthority>> = vec![];

    for authority in authorities {
        document_authorities.push(Box::new(FileSystemDocumentAuthority::new(Path::new(
            authority,
        ))))
    }

    // Create a new schema system from given document authorities
    let mut schema_system = SchemaSystem::new(document_authorities);

    // load given schema
    let schema = schema_system.load_schema(schema_id);

    if schema.is_ok() {
        eprintln!("Schema: {:?} was successfully loaded", schema.unwrap().id());
    } else {
        eprintln!("{:?}", schema.unwrap_err());
    }

    Ok(())
}

