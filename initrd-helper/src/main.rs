use anyhow::{anyhow, Context};
use base64::prelude::*;
use clap::{Args, Parser, Subcommand};
use sev::firmware::guest::{AttestationReport, Firmware};
use std::{fs::File, io::BufWriter, path::PathBuf, process::exit};

/// A helper that provides utilities for the initrd script.
#[derive(Parser)]
struct Cli {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Generate an attestation report and store it as a JSON file in the given path.
    Report(ReportArgs),
}

#[derive(Args)]
struct ReportArgs {
    // A base64-encoded 64 byte string.
    #[clap(short, long)]
    data: String,

    /// The path where the output report in JSON format will be written to.
    output_path: PathBuf,
}

fn generate_report(args: ReportArgs) -> anyhow::Result<()> {
    let ReportArgs { data, output_path } = args;

    // Parse the input data as a 64 byte blob.
    let data = BASE64_STANDARD.decode(&data).context("parsing data as base64")?;
    let data: [u8; 64] = data.try_into().map_err(|_| anyhow!("data must be 64 bytes"))?;

    // Get a report.
    let mut firmware = Firmware::open().context("opening /dev/sev-guest")?;
    let report = firmware.get_report(None, Some(data), None).context("creating attestation report")?;
    let report = AttestationReport::from_bytes(&report).context("parsing attestation report")?;

    // Write the report into the given path, as JSON.
    let output_file = File::create(output_path).context("opening output path")?;
    let output_file = BufWriter::new(output_file);
    serde_json::to_writer(output_file, &report).context("writing report to the output file")?;
    Ok(())
}

fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Command::Report(args) => generate_report(args),
    }
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("Failed to run: {e:#}");
        exit(1);
    }
}
