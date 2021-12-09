// use std::cell::RefCell;
// use std::cmp::min;
// use std::fmt::{Display, Write};
// use std::fs::File;
// use std::io;
// use std::io::BufWriter;
// use std::ops::Range;
// use std::rc::Rc;
// use std::str::{from_utf8_unchecked, FromStr};
//
// use anyhow::{bail, Context, Result};
// use clap::{App, Arg, ArgMatches};
// use colored::Colorize;
// use ion_rs::{BinaryIonCursor, IonType, Reader, SymbolTable, SystemEventHandler};
// use ion_rs::result::IonResult;
// use ion_rs::text::writer::TextWriter;
// use ion_rs::value::reader::element_reader;
// use memmap::MmapOptions;
// use ion_schema_rust::authority::{DocumentAuthority, FileSystemDocumentAuthority};
// use std::path::Path;
// use ion_schema_rust::system::SchemaSystem;
//
// const ABOUT: &str = "Loads an Ion Schema file and returns a result message showing a successful load message when there were no errors found. Otherwise, shows an error message if there were any invalid schema syntax found during the load process";
//
// // Creates a `clap` (Command Line Arguments Parser) configuration for the `load` command.
// // This function is invoked by the `inspect` command's parent, `schema`, so it can describe its
// // child commands.
// pub fn app() -> App<'static, 'static> {
//     App::new("load")
//         .about(ABOUT)
//         .arg(
//             // Any number of input files can be specified by repeating the "-s" or "--schema" flags.
//             // Unlabeled positional arguments will also be considered input file names.
//             Arg::with_name("schema")
//                 .long("schema")
//                 .short("s")
//                 .required(true)
//                 .takes_value(true)
//                 .value_name("SCHEMA")
//                 .help("The Ion Schema file to load with the ISL type that needs to be validated"),
//         )
//         .arg(
//             Arg::with_name("value")
//                 .long("value")
//                 .short("v")
//                 .required(true)
//                 .takes_value(true)
//                 .value_name("value")
//                 .help("The Ion value to be validated"),
//         )
//         .arg(
//             Arg::with_name("directories")
//                 .long("directory")
//                 .short("d")
//                 .min_values(1)
//                 .takes_value(true)
//                 .multiple(true)
//                 .value_name("DIRECTORY")
//                 .required(true)
//                 .help("One or more directories that will be searched for the requested schema"),
//         )
// }
//
// // This function is invoked by the `load` command's parent, `schema`.
// pub fn run(_command_name: &str, matches: &ArgMatches<'static>) -> Result<()> {
//     // Extract the user provided authorities
//     let authorities: Vec<_> = matches.values_of("directories").unwrap().collect();
//
//     // Extract schema file provided by user
//     let schema_id = matches.value_of("schema").unwrap();
//
//     // Extract Ion value provided by user
//     let value = matches.value_of("value").unwrap();
//     let owned_element =  element_reader()
//         .read_all(value.as_bytes())
//         .expect("parsing failed unexpectedly");
//
//     // Set up authorities vector
//     let mut document_authorities: Vec<Box<dyn DocumentAuthority>> = vec![];
//
//     for authority in authorities {
//         document_authorities.push(Box::new(FileSystemDocumentAuthority::new(Path::new(
//             authority,
//         ))))
//     }
//
//     // Create a new schema system from given document authorities
//     let mut schema_system = SchemaSystem::new(document_authorities);
//
//     // load schema
//     let schema = schema_system.load_schema(schema_id);
//
//    //  get teh type defined within the schema file
//     let type_ref = schema.unwrap().get_types().next().expect("Loaded schema was empty");
//     let validation_result = type_ref.validate(owned_element);
//
//     if validation_result.is_ok() {
//         eprintln!("value: {:?} is valid according to schema {:?}", owned_element,schema_id);
//     } else {
//         eprintln!("{:?}", validation_result.unwrap_err());
//     }
//
//     Ok(())
// }
//
