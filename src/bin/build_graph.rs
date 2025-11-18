use anyhow::{Context, Result};
use clap::Parser;
use hskclustering::{
    embeddings::{prepare_embeddings, PcaWhiteningConfig},
    graph_io::{build_graph_and_write_graphml, default_output_for_input},
};

/// Build an undirected, weighted graph from an SGNS embedding file and save as GraphML.
/// used this with the following input file, filtered for hsk 1+2+3 levels:
/// https://github.com/Embedding/Chinese-Word-Vectors
/// sgns, 300d, word + character + ngram
#[derive(Parser, Debug)]
#[command(name = "build_graph")]
#[command(
    about = "Construct a full weighted graph from normalized SGNS embeddings and save as GraphML"
)]
struct Args {
    /// path to the SGNS-like embeddings file (e.g., hsk123-embeddings.txt)
    #[arg(long, short = 'i', default_value = "hsk123-embeddings.txt")]
    input: String,

    /// output GraphML path (defaults to input basename with .graphml)
    #[arg(long, short = 'o')]
    output: Option<String>,

    /// apply PCA + whitening to embeddings before normalization
    #[arg(long)]
    preprocess: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    eprintln!(
        "Reading embeddings from {}{}",
        args.input,
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
        .with_context(|| format!("While loading embeddings from {}", args.input))?;

    let out_path_buf = match args.output {
        Some(o) => std::path::PathBuf::from(o),
        None => default_output_for_input(&args.input),
    };

    eprintln!(
        "Building full graph with {} nodes (complete graph has {} edges) and writing to {}",
        tokens.len(),
        tokens.len() * (tokens.len() - 1) / 2,
        out_path_buf.display()
    );

    build_graph_and_write_graphml(&tokens, &vectors, &out_path_buf)?;

    eprintln!("Done.");
    Ok(())
}
