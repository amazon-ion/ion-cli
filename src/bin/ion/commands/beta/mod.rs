pub mod count;
pub mod from;

#[cfg(feature = "beta-subcommands")]
pub mod generate;
pub mod head;
pub mod inspect;
pub mod primitive;
pub mod schema;
pub mod symtab;
pub mod to;

use crate::commands::beta::count::CountCommand;
use crate::commands::beta::from::FromNamespace;
#[cfg(feature = "beta-subcommands")]
use crate::commands::beta::generate::GenerateCommand;
use crate::commands::beta::head::HeadCommand;
use crate::commands::beta::inspect::InspectCommand;
use crate::commands::beta::primitive::PrimitiveCommand;
use crate::commands::beta::schema::SchemaNamespace;
use crate::commands::beta::symtab::SymtabNamespace;
use crate::commands::beta::to::ToNamespace;
use crate::commands::IonCliCommand;

pub struct BetaNamespace;

impl IonCliCommand for BetaNamespace {
    fn name(&self) -> &'static str {
        "beta"
    }

    fn about(&self) -> &'static str {
        "The 'beta' command is a namespace for commands whose interfaces are not yet stable."
    }

    fn subcommands(&self) -> Vec<Box<dyn IonCliCommand>> {
        vec![
            Box::new(CountCommand),
            Box::new(InspectCommand),
            Box::new(PrimitiveCommand),
            Box::new(SchemaNamespace),
            Box::new(HeadCommand),
            Box::new(FromNamespace),
            Box::new(ToNamespace),
            Box::new(SymtabNamespace),
            #[cfg(feature = "beta-subcommands")]
            Box::new(GenerateCommand),
        ]
    }
}
