use crate::commands::command_namespace::IonCliNamespace;
use crate::commands::IonCliCommand;

use crate::commands::to::json::ToJsonCommand;

pub mod json;

pub struct ToNamespace;

impl IonCliNamespace for ToNamespace {
    fn name(&self) -> &'static str {
        "to"
    }

    fn about(&self) -> &'static str {
        "'to' is a namespace for commands that convert Ion to another data format."
    }

    fn subcommands(&self) -> Vec<Box<dyn IonCliCommand>> {
        vec![Box::new(ToJsonCommand)]
    }
}
