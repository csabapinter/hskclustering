use anyhow::Result;
use clap::{Parser, Subcommand};
use hskclustering::graph_v2::dataset;
use std::path::PathBuf;

/// Prepare exact word vectors and preserve source IDs for Leiden assignments.
#[derive(Parser)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Filter a plain/bzip2 word2vec source to ID,Tilelabels CSV spellings.
    /// Repeated spellings share one vector; unmatched IDs remain explicit.
    Prepare {
        #[arg(long)]
        dataset: PathBuf,
        #[arg(long)]
        source: PathBuf,
        /// A new directory for embeddings, ID mapping, missing rows and hashes.
        #[arg(long)]
        output: PathBuf,
    },
    /// Restore every source ID after clustering; missing vectors have no community.
    Export {
        /// Directory produced by prepare; its entries/vectors are hash-verified.
        #[arg(long)]
        prepared: PathBuf,
        /// Complete token,community CSV for the matched vocabulary.
        #[arg(long)]
        assignments: PathBuf,
        /// A new ID,token,community,status CSV file.
        #[arg(long)]
        output: PathBuf,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Prepare {
            dataset,
            source,
            output,
        } => {
            let report = dataset::prepare(&dataset, &source, &output)?;
            println!("Prepared {} rows / {} spellings: {} matched rows / {} matched spellings; {} missing rows / {} missing spellings; {} dimensions", report.rows, report.unique_tokens, report.matched_rows, report.matched_tokens, report.missing_rows, report.missing_tokens, report.dimension);
        }
        Command::Export {
            prepared,
            assignments,
            output,
        } => {
            let count = dataset::export(&prepared, &assignments, &output)?;
            println!("Exported {count} source IDs to {}", output.display());
        }
    }
    Ok(())
}
