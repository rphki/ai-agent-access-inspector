//! Command line interface of the AI Agent Access Inspector.

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use inspector_core::context::ScanContext;
use inspector_core::export;
use inspector_core::export::anonymize::{self, PathPrivacy};
use inspector_core::export::markdown::{self, MarkdownOptions};
use inspector_core::scanner::{self, AgentScanner};
use inspector_core::{APP_NAME, VERSION};

#[derive(Parser)]
#[command(name = APP_NAME, version = VERSION, about = "Shows which folders local AI agents can access according to their configuration.")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Scan agent configurations and print the findings.
    Scan(ScanArgs),
    /// List the available scanners.
    ListScanners,
}

#[derive(clap::Args)]
struct ScanArgs {
    /// Scanner to run (repeatable). Default: all scanners for real agents.
    #[arg(long = "agent", value_name = "ID")]
    agents: Vec<String>,

    /// Output format.
    #[arg(long, value_enum, default_value_t = Format::Json)]
    format: Format,

    /// Use this directory as the home directory instead of the real one.
    #[arg(long, value_name = "DIR", conflicts_with = "fixture")]
    home: Option<PathBuf>,

    /// Directory to treat as the current project.
    #[arg(long, value_name = "DIR")]
    cwd: Option<PathBuf>,

    /// Scan a fixture directory (with fixture.json) instead of the system.
    #[arg(long, value_name = "DIR")]
    fixture: Option<PathBuf>,

    /// Markdown: also show informational diagnostics. JSON always contains all.
    #[arg(long, short)]
    verbose: bool,

    /// How paths are shown: as they are, with the home directory as `~`, or
    /// additionally with anonymised user names.
    #[arg(long, value_enum, default_value_t = Paths::Full)]
    paths: Paths,
}

#[derive(Clone, Copy, ValueEnum)]
enum Paths {
    Full,
    Home,
    Anonymized,
}

impl From<Paths> for PathPrivacy {
    fn from(paths: Paths) -> Self {
        match paths {
            Paths::Full => PathPrivacy::Full,
            Paths::Home => PathPrivacy::Home,
            Paths::Anonymized => PathPrivacy::Anonymized,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Json,
    Markdown,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Scan(args) => scan(args),
        Command::ListScanners => list_scanners(),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

fn scan(args: ScanArgs) -> Result<(), String> {
    let mut context = match &args.fixture {
        Some(root) => ScanContext::from_fixture(root),
        None => ScanContext::from_system(),
    }
    .map_err(|e| e.to_string())?;
    if let Some(home) = args.home {
        context.home_dir = home;
    }
    if let Some(cwd) = args.cwd {
        context.current_dir = Some(cwd);
    }

    let scanners = select_scanners(&args.agents)?;
    let report = anonymize::anonymize(
        &scanner::run_scan(&context, &scanners),
        &context.home_dir.to_string_lossy(),
        args.paths.into(),
    );

    let output = match args.format {
        Format::Json => export::json(&report),
        Format::Markdown => markdown::render(
            &report,
            MarkdownOptions {
                verbose: args.verbose,
                translations: None,
            },
        ),
    };
    write_stdout(output.trim_end())
}

fn select_scanners(ids: &[String]) -> Result<Vec<&'static dyn AgentScanner>, String> {
    if ids.is_empty() {
        return Ok(scanner::default_scanners());
    }
    ids.iter()
        .map(|id| {
            scanner::find_scanner(id)
                .ok_or_else(|| format!("unknown scanner '{id}' (see `{APP_NAME} list-scanners`)"))
        })
        .collect()
}

fn list_scanners() -> Result<(), String> {
    let defaults: Vec<String> = scanner::default_scanners()
        .iter()
        .map(|s| s.metadata().id)
        .collect();
    let mut lines = String::new();
    for scanner in scanner::all_scanners() {
        let meta = scanner.metadata();
        let marker = if defaults.contains(&meta.id) {
            ""
        } else {
            " (only with --agent)"
        };
        lines.push_str(&format!(
            "{:<14} {}{marker}\n               {}\n",
            meta.id, meta.name, meta.description
        ));
    }
    write_stdout(lines.trim_end())
}

/// Writes to stdout; a closed pipe (e.g. `| head`) is not an error.
fn write_stdout(text: &str) -> Result<(), String> {
    let mut stdout = io::stdout().lock();
    match writeln!(stdout, "{text}") {
        Err(e) if e.kind() != io::ErrorKind::BrokenPipe => Err(e.to_string()),
        _ => Ok(()),
    }
}
