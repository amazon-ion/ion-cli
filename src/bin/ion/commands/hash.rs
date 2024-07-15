use crate::commands::{CommandIo, IonCliCommand, WithIonCliArgument};
use anyhow::Result;
use clap::builder::PossibleValue;
use clap::{value_parser, Arg, ArgAction, ArgMatches, Command, ValueEnum};
use ion_rs::ion_hash::IonHasher;
use ion_rs::*;
use sha2::{Sha256, Sha512};
use sha3::{Sha3_256, Sha3_512};
use std::io::Write;

// Macro to eliminate repetitive code for each hash algorithm.
macro_rules! supported_hash_functions {
    ($($name:literal => $hash:ident),+$(,)?) => {
        #[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
        enum DigestType {
            #[default]
            $($hash),+
        }
        impl DigestType {
            const VARIANTS: &'static [DigestType] = &[
                $(DigestType::$hash),+
            ];

            fn hash_it(&self, element: &Element) -> IonResult<Vec<u8>> {
                match &self {
                    $(DigestType::$hash => Ok($hash::hash_element(&element)?.to_vec()),)+
                }
            }
        }
        impl ValueEnum for DigestType {
            fn value_variants<'a>() -> &'a [Self] {
                DigestType::VARIANTS
            }

            fn to_possible_value(&self) -> Option<PossibleValue> {
                match self {
                    $(DigestType::$hash => Some($name.into()),)+
                }
            }
        }
    };
}

supported_hash_functions! {
    "sha-256" => Sha256,
    "sha-512" => Sha512,
    "sha3-256" => Sha3_256,
    "sha3-512" => Sha3_512,
}

pub struct HashCommand;

impl IonCliCommand for HashCommand {
    fn name(&self) -> &'static str {
        "hash"
    }

    fn about(&self) -> &'static str {
        "Calculates a hash of Ion values using the Ion Hash algorithm."
    }

    fn is_stable(&self) -> bool {
        false
    }

    fn is_porcelain(&self) -> bool {
        false
    }

    fn configure_args(&self, command: Command) -> Command {
        command
            .arg(
                Arg::new("hash")
                    .required(true)
                    .value_parser(value_parser!(DigestType)),
            )
            .with_output()
            .with_input()
            // TODO: If we want to support other output formats, add flags for them
            //       and an ArgGroup to ensure only one is selected.
            //       Default right now is to emit base16 strings of the digest.
            .arg(
                Arg::new("raw")
                    .long("raw")
                    .help("Emit the digest(s) as raw bytes.")
                    .action(ArgAction::SetTrue),
            )
    }

    fn run(&self, _command_path: &mut Vec<String>, args: &ArgMatches) -> Result<()> {
        CommandIo::new(args).for_each_input(|output, input| {
            let mut reader = Reader::new(AnyEncoding, input.into_source())?;

            for elem in reader.elements() {
                let elem = elem?;
                if let Some(hash) = args.get_one::<DigestType>("hash") {
                    let digest = hash.hash_it(&elem)?;
                    if args.get_flag("raw") {
                        output.write_all(&digest)?;
                    } else {
                        let digest_string: String =
                            digest.iter().map(|b| format!("{:02x?}", b)).collect();
                        output.write_all(digest_string.as_bytes())?;
                    };
                    output.write_all("\n".as_bytes())?;
                } else {
                    unreachable!("clap ensures that there is a valid argument")
                }
            }
            Ok(())
        })
    }
}
