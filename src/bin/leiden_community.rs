use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use graphrs::{
    algorithms::community::leiden::{self, QualityFunction},
    readwrite, GraphSpecs,
};
use hskclustering::graph_io::default_output_for_input;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "leiden_community")]
#[command(about = "Run Leiden community detection on a GraphML and write token-community CSV")]
struct Args {
    /// Input GraphML file
    #[arg(long, short = 'i', default_value = "hsk123-embeddings.graphml")]
    input: String,

    /// Output CSV (defaults to input basename + .communities.csv)
    #[arg(long, short = 'o')]
    output: Option<String>,

    /// Quality function (CPM or Modularity)
    #[arg(long, value_enum, default_value = "cpm")]
    quality: Quality,

    /// Resolution parameter (higher => smaller communities)
    #[arg(long, default_value_t = 0.05)]
    resolution: f64,

    /// Theta parameter controlling randomness
    #[arg(long, default_value_t = 0.3)]
    theta: f64,

    /// CPM gamma controlling community granularity (ignored for modularity)
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
    let input = &args.input;
    let csv_path = match &args.output {
        Some(path) => PathBuf::from(path),
        None => default_output_for_input(input).with_extension("communities.csv"),
    };

    eprintln!(
        "Loading graph from {} and running Leiden ({:?}, resolution {:.3}, theta {:.3}, gamma {:.3})",
        input, args.quality, args.resolution, args.theta, args.gamma
    );
    let graph = readwrite::graphml::read_graphml_file(input, GraphSpecs::undirected())
        .map_err(|e| anyhow::anyhow!("Failed to read GraphML from {}: {}", input, e))?;

    let quality_fn = match args.quality {
        Quality::Cpm => QualityFunction::CPM,
        Quality::Modularity => QualityFunction::Modularity,
    };
    let gamma = match args.quality {
        Quality::Cpm => Some(args.gamma),
        Quality::Modularity => None,
    };
    let communities = leiden::leiden(
        &graph,
        true,
        quality_fn,
        Some(args.resolution),
        Some(args.theta),
        gamma,
    )
    .map_err(|e| anyhow::anyhow!("Leiden community detection failed: {}", e))?;

    let mut community_sizes: Vec<usize> = communities.iter().map(|c| c.len()).collect();
    community_sizes.sort_by(|a, b| b.cmp(a));
    eprintln!(
        "Communities: {} (median size {}), largest {}",
        community_sizes.len(),
        median(&community_sizes),
        community_sizes.first().copied().unwrap_or(0)
    );

    let mut assignments: HashMap<String, usize> = HashMap::new();
    for (cid, members) in communities.iter().enumerate() {
        for token in members {
            assignments.insert(token.clone(), cid);
        }
    }

    let mut writer = BufWriter::new(
        File::create(&csv_path).with_context(|| format!("Failed to create {:?}", csv_path))?,
    );
    writeln!(writer, "token,community")?;
    for node in graph.get_all_nodes() {
        let token = node.name.clone();
        if let Some(cid) = assignments.get(&token) {
            writeln!(writer, "{},{}", token, cid)?;
        }
    }
    writer.flush()?;

    eprintln!("Wrote {} assignments to {:?}", assignments.len(), csv_path);
    Ok(())
}

fn median(values: &[usize]) -> usize {
    if values.is_empty() {
        return 0;
    }
    let mut sorted = values.to_vec();
    sorted.sort();
    sorted[sorted.len() / 2]
}
