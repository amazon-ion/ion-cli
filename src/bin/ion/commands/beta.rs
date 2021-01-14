use clap::{App, ArgMatches};

//Modify beta-support command here
pub fn generate_subcmd() -> Vec<App<'static, 'static>> {
    vec![]
}

//Modify beta-support runner here
pub fn run(command_name: &str, matches: &ArgMatches<'static>) {
    let (command_name, command_args) = matches.subcommand();
    match command_name {
        _ => (),
    }
}

pub fn app() -> App<'static, 'static> {
    App::new("beta")
        .about(
            "The 'beta' command is a namespace for commands whose interfaces are not yet stable.",
        )
        .subcommands(generate_subcmd())
}
