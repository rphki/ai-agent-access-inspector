//! Command line interface of the AI Agent Access Inspector.
//!
//! Scanning is added in later phases; for now the CLI only reports its
//! version and usage.

use std::process::ExitCode;

use inspector_core::{APP_NAME, VERSION};

fn usage() -> String {
    format!(
        "{APP_NAME} {VERSION}\n\
         \n\
         Usage: {APP_NAME} [--help | --version]\n\
         \n\
         Scanning is not implemented yet."
    )
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None | Some("-h" | "--help") => {
            println!("{}", usage());
            ExitCode::SUCCESS
        }
        Some("-V" | "--version") => {
            println!("{APP_NAME} {VERSION}");
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("error: unknown argument '{other}'\n\n{}", usage());
            ExitCode::from(2)
        }
    }
}
