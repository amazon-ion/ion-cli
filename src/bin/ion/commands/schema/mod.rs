pub mod load;
pub mod validate;

use crate::commands::command_namespace::IonCliNamespace;
use crate::commands::IonCliCommand;

use crate::commands::schema::load::LoadCommand;
use crate::commands::schema::validate::ValidateCommand;

pub struct SchemaNamespace;

impl IonCliNamespace for SchemaNamespace {
    fn name(&self) -> &'static str {
        "schema"
    }

    fn about(&self) -> &'static str {
        "The 'schema' command is a namespace for commands that are related to Ion Schema."
    }

    fn subcommands(&self) -> Vec<Box<dyn IonCliCommand>> {
        vec![Box::new(LoadCommand), Box::new(ValidateCommand)]
    }
}
