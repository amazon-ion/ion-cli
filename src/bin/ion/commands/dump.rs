use clap::{App, Arg, ArgMatches};

use libc::c_char;
use libc::c_int;
use std::ffi::CString;
use std::ptr;

// ion_c_cli_main is a C function that lives in the ion-c CLI, to which ion-cli is
// statically linked.
extern "C" {
    fn ion_c_cli_main(argc: c_int, argv: *const *const c_char);
}

fn run_ion_c_cli(args: &[&str]) {
    // Convert the length-prefixed Rust str arguments to null-terminated C strings
    let argv_as_c_str = args
        .iter()
        .map(|arg| CString::new(*arg).unwrap())
        .collect::<Vec<CString>>();

    // Convert the C strings to char * pointers. Note: it's important that we collect()
    // the values below into a separate vector from the values above; it guarantees that
    // the memory being pointed to will still be valid by the time the ion_c_cli accesses it.
    let mut argv_as_char_star = argv_as_c_str
        .iter()
        .map(|arg| arg.as_ptr())
        .collect::<Vec<*const c_char>>();

    // The number of arguments as a C int
    let argc = argv_as_char_star.len() as c_int;

    // Programs sometimes rely on argv being null-terminated, so we'll push a null onto the array.
    argv_as_char_star.push(ptr::null());

    let argv = argv_as_char_star.as_ptr();

    unsafe {
        ion_c_cli_main(argc, argv);
    }
}

pub fn app() -> App<'static, 'static> {
    App::new("dump")
        .about("Prints Ion in the requested format")
        .arg(
            Arg::with_name("format")
                .long("format")
                .short("f")
                .takes_value(true)
                .default_value("pretty")
                .possible_values(&["binary", "text", "pretty"])
                .help("Output format"),
        )
        .arg(
            Arg::with_name("output")
                .long("output")
                .short("o")
                .takes_value(true)
                .help("Output file [default: STDOUT]"),
        )
        .arg(
            // All argv entries after the program name (argv[0])
            // and any `clap`-managed options are considered input files.
            Arg::with_name("input")
                .index(1)
                .multiple(true)
                .help("Input file [default: STDIN]"),
        )
}

pub fn run(command_name: &str, matches: &ArgMatches<'static>) {
    let mut args: Vec<&str> = vec![command_name, "process"];

    // -f pretty|text|binary
    if let Some(format) = matches.value_of("format") {
        args.push("-f");
        args.push(format);
    }

    // -o filename
    if let Some(output_file) = matches.value_of("output") {
        args.push("-o");
        args.push(output_file);
    }

    // ...files
    if let Some(input_file_iter) = matches.values_of("input") {
        for input_file in input_file_iter {
            args.push(input_file);
        }
    } else {
        args.push("-"); // Signifies STDIN
    }

    run_ion_c_cli(&args);
}
