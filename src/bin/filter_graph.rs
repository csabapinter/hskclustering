use anyhow::Result;
use clap::Parser;
use hskclustering::graph_io::{filter_graphml, filtered_output_for_input, DEFAULT_FULL_GRAPH};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(about = "Stream GraphML, dropping edges below a weight threshold and keeping every node")]
struct Args {
    #[arg(long, short = 'i', default_value = DEFAULT_FULL_GRAPH)]
    input: PathBuf,

    /// Output GraphML path (defaults to input basename plus threshold)
    #[arg(long, short = 'o')]
    output: Option<PathBuf>,

    /// Keep weights >= this value in [0, 1]
    #[arg(long, short = 't', default_value_t = 0.70)]
    threshold: f64,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let output = args
        .output
        .unwrap_or_else(|| filtered_output_for_input(&args.input, args.threshold));
    eprintln!(
        "Filtering {} at weight >= {}",
        args.input.display(),
        args.threshold
    );
    let stats = filter_graphml(&args.input, &output, args.threshold)?;
    eprintln!(
        "Wrote {} nodes and {} of {} edges to {}",
        stats.nodes,
        stats.edges_kept,
        stats.edges_read,
        output.display()
    );
    Ok(())
}
