pub mod count;
pub mod depth;
pub mod size;

use crate::commands::beta::analyze::count::CountCommand;
use crate::commands::beta::analyze::depth::DepthCommand;
use crate::commands::beta::analyze::size::SizeCommand;
use crate::commands::IonCliCommand;

pub struct AnalyzeNamespace;

impl IonCliCommand for AnalyzeNamespace {
    fn name(&self) -> &'static str {
        "analyze"
    }

    fn about(&self) -> &'static str {
        "The 'analyze' command is a namespace for commands used for Ion stream statistical analysis."
    }
    fn subcommands(&self) -> Vec<Box<dyn IonCliCommand>> {
        vec![
            Box::new(CountCommand),
            Box::new(SizeCommand),
            Box::new(DepthCommand),
        ]
    }
}
