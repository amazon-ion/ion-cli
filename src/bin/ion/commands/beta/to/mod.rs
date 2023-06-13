use crate::commands::IonCliCommand;

use crate::commands::beta::to::json::ToJsonCommand;

pub mod json;

pub struct ToNamespace;

impl IonCliCommand for ToNamespace {
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
