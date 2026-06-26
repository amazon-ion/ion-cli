use crate::file_writer::FileWriter;
use crate::input::CommandInput;
use crate::output::{CommandOutput, CommandOutputSpec, HighlightedStreamWriter};
use anyhow::Result;
use anyhow::{bail, Context};
use clap::builder::ValueParser;
use clap::{crate_authors, crate_version, Arg, ArgAction, ArgMatches, Command as ClapCommand};
use ion_rs::{IonEncoding, TextFormat};
use std::fs::File;

/// Local replacement for the `Format` enum that was removed from ion-rs in 1.0.0.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum Format {
    Text(TextFormat),
    Binary,
}
use std::io::IsTerminal;
use std::io::Write;
use termcolor::{ColorChoice, StandardStream, StandardStreamLock};

pub mod cat;
mod command_namespace;
pub mod complaint;
pub mod from;
pub mod generate;
pub mod hash;
pub mod head;
pub mod inspect;
pub mod jq;
pub mod primitive;
pub mod schema;
pub mod stats;
pub mod structural_recursion;
pub mod symtab;
pub mod timestamp_conversion;
pub mod to;

pub(crate) use command_namespace::IonCliNamespace;

/// Behaviors common to all Ion CLI commands, including both namespaces (groups of commands)
/// and the commands themselves.
pub trait IonCliCommand {
    /// Indicates whether this command is stable (as opposed to unstable or experimental).
    /// Namespaces should almost always be stable.
    fn is_stable(&self) -> bool {
        false
    }

    /// Whether the output format is machine-readable.
    ///
    /// Commands that are "plumbing" should default to putting one output (result, value, document)
    /// on each line in a machine-readable format (file name, Ion value(s), integers, booleans)
    /// without any prose or table formatting, etc.
    ///
    /// See https://git-scm.com/book/en/v2/Git-Internals-Plumbing-and-Porcelain#_plumbing_porcelain
    fn is_porcelain(&self) -> bool {
        true
    }

    /// Returns the name of this command.
    ///
    /// This value is used for dispatch (that is: mapping the name provided on the command line
    /// to the appropriate command) and for help messages.
    fn name(&self) -> &'static str;

    /// A brief message describing this command's functionality.
    fn about(&self) -> &'static str;

    /// A long message describing this command's functionality.
    fn long_about(&self) -> Option<&'static str> {
        None
    }

    /// Initializes a [`ClapCommand`] representing this command and its subcommands (if any).
    ///
    /// Commands wishing to customize their `ClapCommand`'s arguments should override
    /// [`Self::configure_args`].
    fn clap_command(&self) -> ClapCommand {
        // Configure a 'base' clap configuration that has the command's name, about message,
        // version, and author.

        let mut base_command = ClapCommand::new(self.name())
            .about(self.about())
            .version(crate_version!())
            .author(crate_authors!())
            .with_decompression_control()
            .arg(
                Arg::new(UNSTABLE_FLAG)
                    .short('X')
                    .long("unstable")
                    .default_value("false")
                    .action(ArgAction::SetTrue)
                    .value_parser(ValueParser::bool())
                    .help("Opt in to using an unstable feature of Ion CLI.")
                    .display_order(usize::MAX)
                    .hide(true),
            );

        if let Some(long_about) = self.long_about() {
            base_command = base_command.long_about(long_about)
        }

        if !self.is_stable() {
            let about = base_command.get_about().map(|x| x.to_string());
            if let Some(about) = about {
                base_command = base_command.about(format!("(UNSTABLE) {}", about))
            }
            base_command = base_command
                .before_help("WARNING: This command is unstable and requires explicit opt-in using '--unstable' or '-X'.");
        }
        if self.is_porcelain() {
            base_command = base_command.after_help(
                "NOTE: The output of this command is not intended to be machine-readable.",
            );
        }

        self.configure_args(base_command)
    }

    /// After initializing a [`ClapCommand`] using [Self::clap_command], subcommands may customize
    /// the `ClapCommand` further by overriding this default implementation.
    fn configure_args(&self, command: ClapCommand) -> ClapCommand {
        // Does nothing by default
        command
    }

    /// The core logic of the command.
    ///
    /// The default implementation assumes this command is a namespace (i.e. a group of subcommands).
    /// It looks for a subcommand in the arguments, then looks up and runs that subcommand.
    ///
    /// Commands should override this implementation.
    fn run(&self, command_path: &mut Vec<String>, args: &ArgMatches) -> anyhow::Result<()>;
}

/// Argument ID for the '--unstable' / '-X' flag
const UNSTABLE_FLAG: &str = "unstable";
/// Argument ID for the '--ion-version' / '-v' flag
const ION_VERSION_ARG_ID: &str = "ion-version";

/// Extension methods for a [`ClapCommand`] which add flags and options that are common to
/// commands in the Ion CLI.
pub trait WithIonCliArgument {
    fn with_input(self) -> Self;
    fn with_output(self) -> Self;
    fn with_format(self) -> Self;
    /// Adds `--color` and `--no-color` flags.
    ///
    /// Only use this for commands that output Ion because the Syntect parsing is incompatible with
    /// other output types.
    fn with_syntax_highlighting(self) -> Self;
    fn with_ion_version(self) -> Self;
    fn with_decompression_control(self) -> Self;
    fn show_unstable_flag(self) -> Self;
}

impl WithIonCliArgument for ClapCommand {
    fn with_input(self) -> Self {
        self.arg(
            Arg::new("input")
                .action(ArgAction::Append)
                .help("Input file"),
        )
    }

    fn with_output(self) -> Self {
        self.arg(
            Arg::new("output")
                .long("output")
                .short('o')
                .help("Output file [default: STDOUT]"),
        )
    }

    fn with_format(self) -> Self {
        self.arg(
            Arg::new("format")
                .long("format")
                .short('f')
                .default_value("pretty")
                .value_parser(["binary", "text", "pretty", "lines"])
                .help("Output format"),
        )
    }

    fn with_syntax_highlighting(self) -> Self {
        // See https://jwodder.github.io/kbits/posts/clap-bool-negate/
        // Except that we want 3-value logic (color, no-color, neither specified)
        self.arg(
            Arg::new("color")
                .long("color")
                .action(ArgAction::SetTrue)
                .help("Enable colored syntax highlighting in output")
                .overrides_with("no-color"),
        )
        .arg(
            Arg::new("no-color")
                .long("no-color")
                .action(ArgAction::SetTrue)
                .help("Disable colored syntax highlighting in output")
                .overrides_with("color"),
        )
    }

    fn with_ion_version(self) -> Self {
        // TODO When this arg/feature becomes stable:
        //    Remove: show_unstable_flag()
        //    Remove: requires(USE_UNSTABLE_FLAG)
        //    Add:    env("ION_CLI_ION_VERSION")
        self.show_unstable_flag()
            .arg(
                Arg::new(ION_VERSION_ARG_ID)
                    .long("ion-version")
                    .short('i')
                    .help("UNSTABLE! Output Ion version")
                    .value_parser(["1.0", "1.1"])
                    .default_value("1.0")
                    .requires(UNSTABLE_FLAG),
            )
            .mut_arg(UNSTABLE_FLAG, |a| a.hide(false))
    }

    fn with_decompression_control(self) -> Self {
        let accepts_input = self.get_arguments().any(|flag| flag.get_id() == "input");
        self.arg(
            Arg::new("no-auto-decompress")
                .long("no-auto-decompress")
                .default_value("false")
                .action(ArgAction::SetTrue)
                .help("Turn off automatic decompression detection.")
                // Do not show this flag in `help` for commands that don't take an `--input` flag.
                .hide(!accepts_input),
        )
    }

    /// All commands automatically have the "unstable" opt-in flag. This makes it visible.
    fn show_unstable_flag(self) -> Self {
        self.mut_arg(UNSTABLE_FLAG, |arg| arg.hide(false))
    }
}

/// Offers convenience methods for working with input and output streams.
pub struct CommandIo<'a> {
    args: &'a ArgMatches,
    format: Format,
    encoding: IonEncoding,
    color: ColorChoice,
}

impl CommandIo<'_> {
    fn new(args: &ArgMatches) -> Result<CommandIo<'_>> {
        // --format pretty|text|lines|binary
        let format = args
            .try_get_one("format")
            .ok()
            .flatten()
            .map(String::as_str);

        // --ion_version 1.0|1.1
        let ion_version = args
            .try_get_one(ION_VERSION_ARG_ID)
            .ok()
            .flatten()
            .map(String::as_str);

        // `clap` validates the specified format/version and provides a default, unless CommandIO is
        // being used by a command which doesn't care about Ion output version/format
        let format = format.unwrap_or("pretty");
        let ion_version = ion_version.unwrap_or("1.0");

        let format = match format {
            "pretty" => Format::Text(TextFormat::Pretty),
            "text" => Format::Text(TextFormat::Compact),
            "lines" => Format::Text(TextFormat::Lines),
            "binary" => Format::Binary,
            unrecognized => bail!("unsupported format '{unrecognized}'"),
        };

        let encoding = match (ion_version, format) {
            ("1.0", Format::Text(_)) => IonEncoding::Text_1_0,
            ("1.0", Format::Binary) => IonEncoding::Binary_1_0,
            ("1.1", Format::Text(_)) => IonEncoding::Text_1_1,
            ("1.1", Format::Binary) => IonEncoding::Binary_1_1,
            (unrecognized, _) => bail!("unrecognized Ion version '{unrecognized}'"),
        };

        let color = if format == Format::Binary {
            ColorChoice::Never
        } else {
            let default_use_color = std::io::stdout().is_terminal();
            resolve_color_choice(default_use_color, args)
        };

        Ok(CommandIo {
            args,
            format,
            encoding,
            color,
        })
    }

    /// Returns `true` if the user has not explicitly disabled auto decompression.
    fn auto_decompression_enabled(&self) -> bool {
        if let Some(is_disabled) = self.args.get_one::<bool>("no-auto-decompress") {
            !*is_disabled
        } else {
            true
        }
    }

    /// Constructs a new [`CommandInput`] representing STDIN.
    fn command_input_for_stdin(&self) -> Result<CommandInput> {
        const STDIN_NAME: &str = "-";
        let stdin = std::io::stdin().lock();
        if self.auto_decompression_enabled() {
            CommandInput::decompress(STDIN_NAME, stdin)
        } else {
            CommandInput::without_decompression(STDIN_NAME, stdin)
        }
    }

    /// Constructs a new [`CommandInput`] representing the specified file.
    fn command_input_for_file_name(&self, name: &str) -> Result<CommandInput> {
        let stream = File::open(name)?;
        if self.auto_decompression_enabled() {
            CommandInput::decompress(name, stream)
        } else {
            CommandInput::without_decompression(name, stream)
        }
    }

    /// Calls the provided closure once for each input source specified by the user.
    /// For each invocation, provides a handle to the configured output stream.
    fn for_each_input(
        &mut self,
        mut f: impl FnMut(&mut CommandOutput, CommandInput) -> Result<()>,
    ) -> Result<()> {
        let spec = CommandOutputSpec {
            format: self.format,
            encoding: self.encoding,
        };

        let stdout: StandardStream;
        let stdout_lock: StandardStreamLock;

        let mut output = if let Some(output_file) = self.args.get_one::<String>("output") {
            // If the user has specified an output file, use it.
            let file = File::create(output_file).with_context(|| {
                format!(
                    "could not open file output file '{}' for writing",
                    output_file
                )
            })?;
            CommandOutput::File(FileWriter::new(file), spec)
        } else {
            // These types are provided by the `termcolor` crate. They wrap the normal `io::Stdout` and
            // `io::StdOutLock` types, making it possible to write colorful text to the output stream when
            // it's a TTY that understands formatting escape codes. These variables are declared here so
            // the lifetime will extend through the remainder of the function. Unlike `io::StdoutLock`,
            // the `StandardStreamLock` does not have a static lifetime.
            stdout = StandardStream::stdout(self.color);
            stdout_lock = stdout.lock();

            let stdout_tty = std::io::stdout().is_terminal();

            match self.color {
                ColorChoice::Never => CommandOutput::StdOut(stdout_lock, spec),
                ColorChoice::Auto if !stdout_tty => CommandOutput::StdOut(stdout_lock, spec),
                _ => CommandOutput::HighlightedOut(
                    Box::new(HighlightedStreamWriter::new(stdout_lock)),
                    spec,
                ),
            }
        };

        if let Some(input_file_names) = self.args.get_many::<String>("input") {
            // Input files were specified, run the converter on each of them in turn
            for input_file_name in input_file_names {
                let input = self.command_input_for_file_name(input_file_name)?;
                f(&mut output, input)?;
            }
        } else {
            let input = self.command_input_for_stdin()?;
            f(&mut output, input)?;
        }
        output.flush()?;
        Ok(())
    }

    fn write_output(&self, mut f: impl FnMut(&mut CommandOutput) -> Result<()>) -> Result<()> {
        let spec = CommandOutputSpec {
            format: self.format,
            encoding: self.encoding,
        };

        // These types are provided by the `termcolor` crate. They wrap the normal `io::Stdout` and
        // `io::StdOutLock` types, making it possible to write colorful text to the output stream when
        // it's a TTY that understands formatting escape codes. These variables are declared here so
        // the lifetime will extend through the remainder of the function. Unlike `io::StdoutLock`,
        // the `StandardStreamLock` does not have a static lifetime.
        let stdout: StandardStream;
        let stdout_lock: StandardStreamLock;
        let mut output = if let Some(output_file) = self.args.get_one::<String>("output") {
            // If the user has specified an output file, use it.
            let file = File::create(output_file).with_context(|| {
                format!(
                    "could not open file output file '{}' for writing",
                    output_file
                )
            })?;
            CommandOutput::File(FileWriter::new(file), spec)
        } else {
            // Otherwise, write to STDOUT.
            stdout = StandardStream::stdout(self.color);
            stdout_lock = stdout.lock();
            CommandOutput::StdOut(stdout_lock, spec)
        };
        f(&mut output)
    }
}

fn resolve_color_choice(context_default: bool, arg_matches: &ArgMatches) -> ColorChoice {
    // For SetTrue args, clap stores a default value, so try_get_one returns Ok(Some(&false)) when
    // the arg is registered but not passed by the user. It returns Ok(None) when the arg was never
    // registered (release builds) or Err (debug builds). In either unregistered case, the command
    // doesn't support color, so we return Never.
    let color = match arg_matches.try_get_one::<bool>("color") {
        Ok(Some(&val)) => val,
        _ => return ColorChoice::Never,
    };
    let no_color = match arg_matches.try_get_one::<bool>("no-color") {
        Ok(Some(&val)) => val,
        _ => return ColorChoice::Never,
    };
    if color {
        ColorChoice::Always
    } else if no_color {
        ColorChoice::Never
    } else if context_default {
        ColorChoice::Auto
    } else {
        ColorChoice::Never
    }
}

#[cfg(test)]
mod test {
    use crate::commands::{resolve_color_choice, WithIonCliArgument};
    use rstest::rstest;
    use termcolor::ColorChoice;

    // Like https://jwodder.github.io/kbits/posts/clap-bool-negate/
    // Except that we want 3-value logic (color, no-color, neither specified)
    #[rstest]
    #[case(true, "test ", ColorChoice::Auto)]
    #[case(false, "test ", ColorChoice::Never)]
    #[case(true, "test --color", ColorChoice::Always)]
    #[case(true, "test --no-color", ColorChoice::Never)]
    #[case(false, "test --color", ColorChoice::Always)]
    #[case(false, "test --no-color", ColorChoice::Never)]
    #[case(true, "test --no-color --color", ColorChoice::Always)]
    #[case(true, "test --color --no-color", ColorChoice::Never)]
    #[case(false, "test --no-color --color", ColorChoice::Always)]
    #[case(false, "test --color --no-color", ColorChoice::Never)]
    #[case(true, "test --color --no-color --color", ColorChoice::Always)]
    #[case(true, "test --no-color --color --no-color", ColorChoice::Never)]
    #[case(false, "test --color --no-color --color", ColorChoice::Always)]
    #[case(false, "test --no-color --color --no-color", ColorChoice::Never)]
    fn resolve_color_choice_args(
        #[case] context_default: bool,
        #[case] args: &str,
        #[case] expected: ColorChoice,
    ) {
        let args = clap::builder::Command::new("test")
            .with_syntax_highlighting()
            .get_matches_from(args.split_ascii_whitespace().collect::<Vec<_>>());
        assert_eq!(resolve_color_choice(context_default, &args), expected)
    }

    #[rstest]
    #[case(true)]
    #[case(false)]
    fn resolve_color_choice_without_color_args_registered(#[case] context_default: bool) {
        let args = clap::builder::Command::new("test").get_matches_from(vec!["test"]);
        assert_eq!(
            resolve_color_choice(context_default, &args),
            ColorChoice::Never
        )
    }
}
