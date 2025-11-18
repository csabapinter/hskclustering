use anyhow::{Context, Result};
use graphrs::{readwrite, Graph, GraphSpecs};
use std::path::{Path, PathBuf};

/// Nodes: words (`&str` borrowed from the supplied `tokens`).
/// Edges: complete graph: one undirected weighted edge for each pair (i<j).
/// Weight: `w = (cos + 1) / 2`, where `cos` is the cosine similarity of L2-normalized vectors.
pub fn build_graph_and_write_graphml(
    tokens: &[String],
    vectors: &[Vec<f32>],
    output_path: &Path,
) -> Result<()> {
    let n = tokens.len();
    assert_eq!(n, vectors.len(), "tokens and vectors must have same length");

    // create_missing() automatically adds nodes together with edges, without having to explicitly
    // add nodes first
    let mut graph: Graph<&str, ()> = Graph::new(GraphSpecs::undirected_create_missing());

    // Borrowed slices of names keep memory small; they must outlive the graph usage below.
    let names: Vec<&str> = tokens.iter().map(|s| s.as_str()).collect();

    const CHUNK: usize = 200_000;
    let mut buf: Vec<(&str, &str, f64)> = Vec::with_capacity(CHUNK);

    // Full upper triangle, no self loops, i<j to avoid duplicate undirected edges.
    for i in 0..n {
        let vi = &vectors[i];
        for j in (i + 1)..n {
            let cos = crate::embeddings::cosine_normalized(vi, &vectors[j]) as f64;
            let w = (cos + 1.0) / 2.0;

            buf.push((names[i], names[j], w));
            if buf.len() >= CHUNK {
                graph
                    .add_edge_tuples_weighted(std::mem::take(&mut buf))
                    .map_err(|e| {
                        eprintln!("Failed while adding a chunk of edges to the graph: {}", e);
                        anyhow::anyhow!("Failed while adding a chunk of edges to the graph: {}", e)
                    })?;
            }
        }

        if i % 250 == 0 {
            eprintln!("Progress: computed similarities for row {} / {}", i, n);
        }
    }
    if !buf.is_empty() {
        graph.add_edge_tuples_weighted(buf).map_err(|e| {
            eprintln!("Failed while adding the final chunk of edges: {}", e);
            anyhow::anyhow!("Failed while adding the final chunk of edges: {}", e)
        })?;
    }

    // Persist the graph to GraphML.
    let out_str = output_path
        .to_str()
        .context("Output path is not valid UTF-8")?;
    readwrite::graphml::write_graphml_file(&graph, out_str)
        .with_context(|| format!("Failed to write GraphML to {}", out_str))?;

    Ok(())
}

/// Create a default output path by replacing the input's extension with `.graphml`.
pub fn default_output_for_input(input_path: &str) -> PathBuf {
    let p = Path::new(input_path);
    match (p.file_stem(), p.parent()) {
        (Some(stem), Some(parent)) => {
            let mut out = parent.to_path_buf();
            let mut name = stem.to_os_string();
            name.push(".graphml");
            out.push(name);
            out
        }
        _ => PathBuf::from("hsk123.graphml"),
    }
}
