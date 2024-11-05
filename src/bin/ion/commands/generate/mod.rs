mod context;
mod generator;
mod result;
mod templates;
mod utils;

mod model;

use crate::commands::generate::generator::CodeGenerator;
use crate::commands::generate::model::NamespaceNode;
use crate::commands::generate::utils::{JavaLanguage, RustLanguage};
use crate::commands::IonCliCommand;
use anyhow::{bail, Result};
use clap::{Arg, ArgAction, ArgMatches, Command, ValueHint};
use colored::Colorize;
use ion_schema::authority::{DocumentAuthority, FileSystemDocumentAuthority};
use ion_schema::system::SchemaSystem;
use std::fs;
use std::path::{Path, PathBuf};

pub struct GenerateCommand;

impl IonCliCommand for GenerateCommand {
    fn name(&self) -> &'static str {
        "generate"
    }

    fn about(&self) -> &'static str {
        "Generates code using given schema file."
    }

    fn is_stable(&self) -> bool {
        false
    }

    fn is_porcelain(&self) -> bool {
        false
    }

    fn configure_args(&self, command: Command) -> Command {
        command
            .arg(
                Arg::new("output")
                    .long("output")
                    .short('o')
                    .help("Output directory [default: current directory]"),
            )
            // `--namespace` is required when Java language is specified for code generation
            .arg(
                Arg::new("namespace")
                    .long("namespace")
                    .short('n')
                    .required_if_eq("language", "java")
                    .help("Provide namespace for generated Java code (e.g. `org.example`)"),
            )
            .arg(
                Arg::new("language")
                    .long("language")
                    .short('l')
                    .required(true)
                    .value_parser(["java", "rust"])
                    .help("Programming language for the generated code"),
            )
            .arg(
                Arg::new("authority")
                    .long("authority")
                    .short('A')
                    .required(true)
                    .action(ArgAction::Append)
                    .value_name("directory")
                    .value_hint(ValueHint::DirPath)
                    .help("The root(s) of the file system authority(s)"),
            )
    }

    fn run(&self, _command_path: &mut Vec<String>, args: &ArgMatches) -> Result<()> {
        // Extract programming language for code generation
        let language: &str = args.get_one::<String>("language").unwrap().as_str();

        // Extract namespace for code generation
        let namespace = args.get_one::<String>("namespace");

        // Extract output path information where the generated code will be saved
        // Create a module `ion_data_model` for storing all the generated code in the output directory
        let binding = match args.get_one::<String>("output") {
            Some(output_path) => PathBuf::from(output_path),
            None => PathBuf::from("./"),
        };

        let output = binding.as_path();

        // Extract the user provided document authorities/ directories
        let authorities: Vec<&String> = args.get_many("authority").unwrap().collect();

        // Set up document authorities vector
        let mut document_authorities: Vec<Box<dyn DocumentAuthority>> = vec![];
        args.get_many::<String>("authority")
            .unwrap_or_default()
            .map(Path::new)
            .map(FileSystemDocumentAuthority::new)
            .for_each(|a| document_authorities.push(Box::new(a)));

        // Create a new schema system from given document authorities
        let mut schema_system = SchemaSystem::new(document_authorities);

        // Generate directories in the output path if the path doesn't exist
        if !output.exists() {
            fs::create_dir_all(output).unwrap();
        }

        println!("Started generating code...");

        // generate code based on schema and programming language
        match language {
            "java" => {
                Self::print_java_code_gen_warnings();
                CodeGenerator::<JavaLanguage>::new(output, namespace.unwrap().split('.').map(|s| NamespaceNode::Package(s.to_string())).collect())
                    .generate_code_for_authorities(&authorities, &mut schema_system)?
            },
            "rust" => {
                Self::print_rust_code_gen_warnings();
                CodeGenerator::<RustLanguage>::new(output)
                    .generate_code_for_authorities(&authorities, &mut schema_system)?
            }
            _ => bail!(
                "Programming language '{}' is not yet supported. Currently supported targets: 'java', 'rust'",
                language
            )
        }

        println!("Code generation complete successfully!");
        println!("All the schema files in authority(s) are generated into a flattened namespace, path to generated code: {}", output.display());
        Ok(())
    }
}

impl GenerateCommand {
    // Prints warning messages for Java code generation
    fn print_java_code_gen_warnings() {
        println!("{}","WARNING: Code generation in Java does not support any `$NOMINAL_ION_TYPES` data type.(For more information: https://amazon-ion.github.io/ion-schema/docs/isl-2-0/spec#built-in-types) Reference issue: https://github.com/amazon-ion/ion-cli/issues/101".yellow().bold());
        println!(
            "{}",
            "Optional fields in generated code are represented with the wrapper class of that primitive data type and are set to `null` when missing."
                .yellow()
                .bold()
        );
        println!("{}", "When the `writeTo` method is used on an optional field and if the field value is set as null then it would skip serializing that field.".yellow().bold());
    }

    // Prints warning messages for Rust code generation
    fn print_rust_code_gen_warnings() {
        println!("{}","WARNING: Code generation in Rust does not yet support any `$NOMINAL_ION_TYPES` data type.(For more information: https://amazon-ion.github.io/ion-schema/docs/isl-2-0/spec#built-in-types) Reference issue: https://github.com/amazon-ion/ion-cli/issues/101".yellow().bold());
        println!("{}","Code generation in Rust does not yet support optional/required fields. It does not have any checks added for this on read or write methods. Reference issue: https://github.com/amazon-ion/ion-cli/issues/106".yellow().bold());
    }
}
