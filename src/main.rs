use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;

mod pty_session;
mod script_control;
mod logging;
mod utils;

use script_control::ScriptControl;

/// Make a typescript of a terminal session
#[derive(Parser, Debug)]
#[command(name = "script")]
#[command(about = "Make a typescript of a terminal session")]
#[command(version = "1.0.0")]
struct Args {
    /// Log stdin to file
    #[arg(short = 'I', long = "log-in")]
    log_in: Option<PathBuf>,

    /// Log stdout to file (default)
    #[arg(short = 'O', long = "log-out")]
    log_out: Option<PathBuf>,

    /// Log stdin and stdout to file
    #[arg(short = 'B', long = "log-io")]
    log_io: Option<PathBuf>,

    /// Log timing information to file
    #[arg(short = 'T', long = "log-timing")]
    log_timing: Option<PathBuf>,

    /// Deprecated alias to -T (default file is stderr)
    #[arg(short = 't', long = "timing")]
    timing: Option<Option<PathBuf>>,

    /// Force to 'classic' or 'advanced' format
    #[arg(short = 'm', long = "logging-format")]
    logging_format: Option<String>,

    /// Append to the log file
    #[arg(short = 'a', long = "append")]
    append: bool,

    /// Run command rather than interactive shell
    #[arg(short = 'c', long = "command")]
    command: Option<String>,

    /// Return exit code of the child process
    #[arg(short = 'e', long = "return")]
    return_exit_code: bool,

    /// Run flush after each write
    #[arg(short = 'f', long = "flush")]
    flush: bool,

    /// Use output file even when it is a link
    #[arg(long = "force")]
    force: bool,

    /// Echo input in session (auto, always or never)
    #[arg(short = 'E', long = "echo")]
    echo: Option<String>,

    /// Terminate if output files exceed size
    #[arg(short = 'o', long = "output-limit")]
    output_limit: Option<String>,

    /// Be quiet
    #[arg(short = 'q', long = "quiet")]
    quiet: bool,

    /// Output file (default: typescript)
    file: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize the script control structure
    let mut control = ScriptControl::new(args)?;

    // Run the script session
    control.run().await
        .context("Failed to run script session")?;

    Ok(())
}