use anyhow::{Context, Result};
use clap::Parser;
use hskclustering::{
    embeddings::{prepare_embeddings, PcaWhiteningConfig},
    graph_io::{
        build_threshold_graph, filtered_output_for_input, validate_threshold, DEFAULT_FULL_GRAPH,
    },
    output::ensure_distinct_paths,
};
use std::path::{Path, PathBuf};

/// Build an undirected, weighted graph from an SGNS embedding file and save as GraphML.
#[derive(Parser, Debug)]
#[command(name = "build_graph")]
#[command(about = "Stream a weighted graph from normalized SGNS embeddings to GraphML")]
struct Args {
    /// path to the SGNS-like embeddings file
    #[arg(long, short = 'i', default_value = "data/input-embeddings.txt")]
    input: PathBuf,

    /// Output GraphML path (defaults to results/leiden/1-9/hsk1-9-full.graphml)
    #[arg(long, short = 'o')]
    output: Option<PathBuf>,

    /// Keep weights >= this value in [0, 1]; omit to export the complete graph
    #[arg(long, short = 't')]
    threshold: Option<f64>,

    /// apply PCA + whitening to embeddings before normalization
    #[arg(long)]
    preprocess: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let threshold = args.threshold.unwrap_or(0.0);
    validate_threshold(threshold)?;
    let out_path_buf = args.output.unwrap_or_else(|| match args.threshold {
        Some(threshold) => filtered_output_for_input(Path::new(DEFAULT_FULL_GRAPH), threshold),
        None => PathBuf::from(DEFAULT_FULL_GRAPH),
    });
    ensure_distinct_paths(&args.input, &out_path_buf)?;

    eprintln!(
        "Reading embeddings from {}{}",
        args.input.display(),
        if args.preprocess {
            " with PCA whitening"
        } else {
            ""
        }
    );
    let whitening = if args.preprocess {
        Some(PcaWhiteningConfig::whitening(None))
    } else {
        None
    };
    let (tokens, vectors) = prepare_embeddings(&args.input, whitening)
        .with_context(|| format!("While loading embeddings from {}", args.input.display()))?;

    eprintln!(
        "Building graph with {} nodes ({} possible edges), threshold {}, writing to {}",
        tokens.len(),
        tokens.len() as u128 * tokens.len().saturating_sub(1) as u128 / 2,
        threshold,
        out_path_buf.display()
    );

    let edges = build_threshold_graph(&tokens, &vectors, &out_path_buf, threshold)?;

    eprintln!(
        "Wrote {} nodes and {edges} edges to {}",
        tokens.len(),
        out_path_buf.display()
    );
    Ok(())
}
