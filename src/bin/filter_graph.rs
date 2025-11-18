use anyhow::{Context, Result};
use clap::Parser;
use graphrs::{readwrite, Graph, GraphSpecs};
use hskclustering::graph_io::default_output_for_input;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "filter_graph")]
#[command(about = "Load a GraphML, drop edges below a weight threshold, write a new GraphML")]
struct Args {
    /// Input GraphML file to read
    #[arg(long, short = 'i', default_value = "hsk123-embeddings.graphml")]
    input: String,

    /// Output GraphML path (defaults to input basename with suffix)
    #[arg(long, short = 'o')]
    output: Option<String>,

    /// Edges with weight strictly below this threshold are removed
    #[arg(long, short = 't', default_value_t = 0.6)]
    threshold: f64,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let input = &args.input;
    let output_path: PathBuf = match &args.output {
        Some(path) => PathBuf::from(path),
        None => {
            let mut p = default_output_for_input(input);
            p.set_extension("");
            let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("filtered");
            let new_name = format!("{}-thresh{:.2}.graphml", stem, args.threshold);
            p.set_file_name(&new_name);
            p
        }
    };

    eprintln!(
        "Loading graph from {} and removing edges with weight < {:.4}",
        input, args.threshold
    );
    let graph = readwrite::graphml::read_graphml_file(input, GraphSpecs::undirected())
        .map_err(|e| anyhow::anyhow!("Failed to read GraphML from {}: {}", input, e))?;

    let total_edges = graph.get_all_edges().len();
    let filtered_edges: Vec<_> = graph
        .get_all_edges()
        .into_iter()
        .filter(|edge| edge.weight >= args.threshold)
        .collect();
    let removed = total_edges - filtered_edges.len();
    eprintln!(
        "Keeping {} edges, removed {} ({}%)",
        filtered_edges.len(),
        removed,
        if total_edges > 0 {
            (removed as f64 / total_edges as f64) * 100.0
        } else {
            0.0
        }
    );

    let mut new_graph: Graph<String, ()> = Graph::new(graph.specs.clone());
    for node in graph.get_all_nodes() {
        new_graph.add_node((*node).clone());
    }
    for edge in filtered_edges {
        let e = (**edge).clone();
        new_graph
            .add_edge(e.into())
            .map_err(|err| anyhow::anyhow!("Failed to add edge after filtering: {}", err))?;
    }

    let out_str = output_path
        .to_str()
        .context("Output path is not valid UTF-8")?;
    eprintln!("Writing filtered graph to {}", out_str);
    readwrite::graphml::write_graphml_file(&new_graph, out_str)
        .with_context(|| format!("Failed to write filtered GraphML to {}", out_str))?;

    eprintln!("Done.");
    Ok(())
}
