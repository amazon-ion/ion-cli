pub mod check;
pub mod validate;

use crate::commands::command_namespace::IonCliNamespace;
use crate::commands::schema::check::CheckCommand;
use crate::commands::schema::validate::ValidateCommand;
use crate::commands::IonCliCommand;
use anyhow::Context;
use clap::{Arg, ArgAction, ArgMatches, ValueHint};
use ion_rs::Element;
use ion_schema::authority::{DocumentAuthority, FileSystemDocumentAuthority};
use ion_schema::schema::Schema;
use ion_schema::system::SchemaSystem;
use ion_schema::types::TypeDefinition;
use std::fs;
use std::path::Path;
use std::sync::Arc;

pub struct SchemaNamespace;

impl IonCliNamespace for SchemaNamespace {
    fn name(&self) -> &'static str {
        "schema"
    }

    fn about(&self) -> &'static str {
        "The 'schema' command is a namespace for commands that are related to Ion Schema."
    }

    fn subcommands(&self) -> Vec<Box<dyn IonCliCommand>> {
        vec![
            Box::new(CheckCommand),
            Box::new(ValidateCommand),
            // TODO: Filter values command?
            // TODO: Compare types command?
            // TODO: Canonical representation of types command?
        ]
    }
}

/// A type that encapsulates the arguments for loading schemas and types.
///
/// This allows users to specify file authorities, schema files, schema ids, inline schemas, and types.
///
/// See [CheckCommand] and [ValidateCommand] for example usages.
struct IonSchemaCommandInput {
    schema_system: SchemaSystem,
    schema: Arc<Schema>,
    type_definition: Option<TypeDefinition>,
}

impl IonSchemaCommandInput {
    fn read_from_args(args: &ArgMatches) -> anyhow::Result<Self> {
        // Extract the user provided document authorities/ directories
        let mut authorities: Vec<Box<dyn DocumentAuthority>> = vec![];
        args.get_many::<String>("authority")
            .unwrap_or_default()
            .map(Path::new)
            .map(FileSystemDocumentAuthority::new)
            .for_each(|a| authorities.push(Box::new(a)));

        // Create a new schema system from given document authorities
        let mut schema_system = SchemaSystem::new(authorities);

        // Load the appropriate schema
        let mut empty_schema_version = None;
        let mut schema = if args.contains_id("schema-id") {
            let schema_id = args.get_one::<String>("schema").unwrap();
            schema_system.load_schema(schema_id)?
        } else if args.contains_id("schema-file") {
            let file_name = args.get_one::<String>("schema-file").unwrap();
            let content = fs::read(file_name)?;
            schema_system.new_schema(&content, "user-provided-schema")?
        } else if args.contains_id("schema-text") {
            let content = args.get_one::<&str>("schema-text").unwrap();
            schema_system.new_schema(content.as_bytes(), "user-provided-schema")?
        } else {
            let version = match args.get_one::<String>("empty-schema") {
                Some(version) if version == "1.0" => "$ion_schema_1_0",
                _ => "$ion_schema_2_0",
            };
            empty_schema_version = Some(version);
            schema_system
                .new_schema(version.as_bytes(), "empty-schema")
                .expect("Creating an empty schema should be effectively infallible.")
        };

        // Get the type definition, if the command uses the type-ref arg and a value is provided.
        // Clap ensures that if `type` is required, the user must have provided it, so we don't
        // have to check the case where the command uses the arg but no value is provided.
        let mut type_definition = None;
        if let Ok(Some(type_name_or_inline_type)) = args.try_get_one::<String>("type-ref") {
            // We allow an inline type when there's an empty schema.
            // The easiest way to determine whether this is an inline type or a type name
            // is to just try to get it from the schema. If nothing is found, then we'll attempt
            // to treat it as an inline type.
            type_definition = schema.get_type(type_name_or_inline_type);

            if type_definition.is_none() && empty_schema_version.is_some() {
                let version = empty_schema_version.unwrap();
                // There is no convenient way to add a type to an existing schema, so we'll
                // construct a new one.

                // Create a symbol element so that ion-rs handle escaping any special characters.
                let type_name = Element::symbol(type_name_or_inline_type);

                let new_schema = format!(
                    r#"
                    {version}
                    type::{{
                      name: {type_name},
                      type: {type_name_or_inline_type}
                    }}
                    "#
                );
                // And finally update the schema and type.
                schema = schema_system.new_schema(new_schema.as_bytes(), "new-schema")?;
                type_definition = schema.get_type(type_name_or_inline_type);
            }

            // Validate that the user didn't pass in an invalid type
            type_definition
                .as_ref()
                .with_context(|| format!("Type not found {}", type_name_or_inline_type))?;
        }

        Ok(IonSchemaCommandInput {
            schema_system,
            schema,
            type_definition,
        })
    }

    // If this ever gets used, the `expect` will cause a compiler error so the developer will
    // know to come remove this.
    #[expect(dead_code)]
    fn get_schema_system(&self) -> &SchemaSystem {
        &self.schema_system
    }

    fn get_schema(&self) -> Arc<Schema> {
        self.schema.clone()
    }

    /// Guaranteed to be Some if the command uses the `type-ref` argument and that argument is required.
    fn get_type(&self) -> Option<&TypeDefinition> {
        self.type_definition.as_ref()
    }

    fn type_arg() -> Arg {
        Arg::new("type-ref")
            .required(true)
            .value_name("type")
            .help("An ISL type name or, if no schema is specified, an inline type definition.")
    }

    fn schema_args() -> Vec<Arg> {
        let schema_options_header = "Selecting a schema";
        let schema_options_group_name = "schema-group";
        vec![
            Arg::new("empty-schema")
                .help_heading(schema_options_header)
                .group(schema_options_group_name)
                .long("empty")
                // This is the default if no schema is specified, so we don't need a short flag.
                .action(ArgAction::Set)
                .value_name("version")
                .value_parser(["1.0", "2.0"])
                .default_value("2.0")
                .help("An empty schema document for the specified Ion Schema version."),
            Arg::new("schema-file")
                .help_heading(schema_options_header)
                .group(schema_options_group_name)
                .long("schema-file")
                .short('f')
                .action(ArgAction::Set)
                .value_hint(ValueHint::FilePath)
                .help("A schema file"),
            Arg::new("schema-text")
                .help_heading(schema_options_header)
                .group(schema_options_group_name)
                .long("schema-text")
                .action(ArgAction::Set)
                .help("The Ion text contents of a schema document."),
            Arg::new("schema-id")
                .help_heading(schema_options_header)
                .group(schema_options_group_name)
                .long("id")
                .requires("authority")
                .action(ArgAction::Set)
                .help("The ID of a schema to load from one of the configured authorities."),
            Arg::new("authority")
                .help_heading(schema_options_header)
                .long("authority")
                .short('A')
                .required(false)
                .action(ArgAction::Append)
                .value_name("directory")
                .value_hint(ValueHint::DirPath)
                .help(
                    "The root(s) of the file system authority(s). Authorities are only required if your \
                    schema needs to import a type from another schema or if you are loading a schema using \
                    the --id option.",
                ),
        ]
    }
}
