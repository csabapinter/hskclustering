use anyhow::Result;
use clap::{Parser, ValueEnum};
use graphrs::algorithms::community::leiden::QualityFunction;
use hskclustering::{
    community::{detect_communities, write_assignments, LeidenConfig},
    graph_io::{read_graph, DEFAULT_FILTERED_GRAPH},
    output::ensure_distinct_paths,
};
use std::path::PathBuf;
use std::time::Instant;

#[derive(Parser, Debug)]
#[command(about = "Run Leiden on a weighted GraphML and write a token-community CSV")]
struct Args {
    #[arg(long, short = 'i', default_value = DEFAULT_FILTERED_GRAPH)]
    input: PathBuf,

    /// Output CSV (defaults to input basename + .communities.csv)
    #[arg(long, short = 'o')]
    output: Option<PathBuf>,

    #[arg(long, value_enum, default_value = "cpm")]
    quality: Quality,

    /// Resolution parameter (higher => smaller communities)
    #[arg(long, default_value_t = 0.05)]
    resolution: f64,

    /// Positive theta controlling refinement randomness
    #[arg(long, default_value_t = 0.3)]
    theta: f64,

    /// Refinement connectivity parameter, used by graphrs for BOTH quality functions
    #[arg(long, default_value_t = 0.05)]
    gamma: f64,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum Quality {
    Cpm,
    Modularity,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let config = LeidenConfig {
        quality: match args.quality {
            Quality::Cpm => QualityFunction::CPM,
            Quality::Modularity => QualityFunction::Modularity,
        },
        resolution: args.resolution,
        theta: args.theta,
        gamma: args.gamma,
    };
    config.validate()?;
    let output = args
        .output
        .unwrap_or_else(|| args.input.with_extension("communities.csv"));
    ensure_distinct_paths(&args.input, &output)?;
    eprintln!("Loading {}", args.input.display());
    let graph = read_graph(&args.input)?;
    eprintln!(
        "Running Leiden on {} nodes / {} edges ({:?}, resolution {}, theta {}, gamma {})",
        graph.number_of_nodes(),
        graph.number_of_edges(),
        args.quality,
        args.resolution,
        args.theta,
        args.gamma,
    );
    eprintln!("graphrs does not expose a random seed; partitions may differ between runs.");
    let started = Instant::now();
    let communities = detect_communities(&graph, config)?;
    let mut sizes: Vec<_> = communities.iter().map(Vec::len).collect();
    sizes.sort_unstable();
    let median = if sizes.is_empty() {
        0.0
    } else {
        (sizes[(sizes.len() - 1) / 2] as f64 + sizes[sizes.len() / 2] as f64) / 2.0
    };
    eprintln!(
        "{} communities; median size {median}, largest {}, singletons {}; Leiden took {:.2?}",
        sizes.len(),
        sizes.last().copied().unwrap_or(0),
        sizes.iter().filter(|&&size| size == 1).count(),
        started.elapsed()
    );
    let count = write_assignments(&output, &communities)?;
    eprintln!("Wrote {count} assignments to {}", output.display());
    Ok(())
}
