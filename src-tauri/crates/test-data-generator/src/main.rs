//! Test Data Generator CLI

use test_data_generator::cli::Cli;
use clap::Parser;

fn main() {
    let cli = Cli::parse();
    if let Err(e) = cli.run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}