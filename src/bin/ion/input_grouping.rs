use clap::{Arg, ArgAction, ArgMatches};

/// Flags for determining how a command should group/split its input values.
///
/// The choices are
/// * `FileHandles` (default)
/// * `Lines` (`-L`)
/// * `TopLevelValues` (`-T`)
///
/// Default is `FileHandles` because that is the default behavior for commands that do not support
/// these options.
///
/// To add this to a command:
/// ```
/// # use clap::Command;
/// fn configure_args(&self, command: Command) -> Command {
///     command.args(InputGrouping::args())
/// }
/// ```
/// To read the value in the command:
/// ```
/// # use clap::ArgMatches;
/// fn run(&self, _command_path: &mut Vec<String>, args: &ArgMatches) -> Result<()> {
///
///     let grouping = InputGrouping::read_from_args(args);
///
///     // ...
///
///     Ok(())
/// }
/// ```
#[derive(Copy, Clone)]
pub(crate) enum InputGrouping {
    FileHandles,
    Lines,
    TopLevelValues,
}

impl InputGrouping {
    pub(crate) fn args() -> impl Iterator<Item = Arg> {
        vec![
            Arg::new("group-by-lines")
                .group("input-grouping-mode")
                .short('L')
                .help("Interpret each line as a separate input.")
                .action(ArgAction::SetTrue),
            Arg::new("group-by-values")
                .group("input-grouping-mode")
                .short('T')
                .help("Interpret each top level value as a separate input.")
                .action(ArgAction::SetTrue),
        ]
        .into_iter()
    }

    pub(crate) fn read_from_args(args: &ArgMatches) -> InputGrouping {
        if args.get_flag("group-by-lines") {
            InputGrouping::Lines
        } else if args.get_flag("group-by-values") {
            InputGrouping::TopLevelValues
        } else {
            InputGrouping::FileHandles
        }
    }
}
