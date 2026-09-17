mod filter;

pub use filter::{filter_graphml, FilterStats};

use anyhow::{ensure, Context, Result};
use graphrs::{readwrite, Graph, GraphSpecs};
use quick_xml::escape::escape;
use rayon::prelude::*;
use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::{embeddings::cosine_normalized, output::write_atomic};

pub const DEFAULT_FULL_GRAPH: &str = "graphs/1-9-leiden/hsk1-9-full.graphml";
pub const DEFAULT_FILTERED_GRAPH: &str = "graphs/1-9-leiden/hsk1-9-thresh0.70.graphml";

/// Stream every pair to disk without constructing an in-memory graph.
pub fn build_graph_and_write_graphml(
    tokens: &[String],
    vectors: &[Vec<f32>],
    output_path: &Path,
) -> Result<u64> {
    build_threshold_graph(tokens, vectors, output_path, 0.0)
}

/// Write all nodes and edges whose `(cosine + 1) / 2` weight meets the threshold.
///
/// Pairwise similarities take O(n²d) work. Only 32 rows of edges are buffered at
/// once, so working memory is O(nd + n), including the normalized embeddings.
/// Parallel computation preserves input order and floating-point summation order.
pub fn build_threshold_graph(
    tokens: &[String],
    vectors: &[Vec<f32>],
    output_path: &Path,
    threshold: f64,
) -> Result<u64> {
    validate_threshold(threshold)?;
    ensure!(
        tokens.len() == vectors.len(),
        "Tokens and vectors must have the same length"
    );
    let mut seen = HashSet::new();
    let dimension = vectors.first().map_or(0, Vec::len);
    for (token, vector) in tokens.iter().zip(vectors) {
        ensure!(
            !token.is_empty() && token.chars().all(valid_xml_char),
            "Invalid GraphML token {token:?}"
        );
        ensure!(seen.insert(token), "Duplicate graph token {token:?}");
        ensure!(
            dimension > 0 && vector.len() == dimension,
            "Invalid vector dimension for {token:?}"
        );
        let norm_sq: f64 = vector.iter().map(|&v| f64::from(v).powi(2)).sum();
        ensure!(
            norm_sq.is_finite() && (norm_sq - 1.0).abs() < 1e-5,
            "Expected a finite L2-normalized vector for {token:?}"
        );
    }
    let names: Vec<_> = tokens.iter().map(|token| escape(token.as_str())).collect();
    write_atomic(output_path, |writer| {
        writeln!(writer, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>")?;
        writeln!(writer, "<graphml xmlns=\"http://graphml.graphdrawing.org/xmlns\">")?;
        writeln!(writer, "<key id=\"weight\" for=\"edge\" attr.name=\"weight\" attr.type=\"double\"/>")?;
        writeln!(writer, "<graph id=\"G\" edgedefault=\"undirected\">")?;
        // Explicit nodes preserve isolates, single-node graphs and threshold=1.
        for name in &names { writeln!(writer, "<node id=\"{name}\"/>")?; }
        let mut edge_count = 0;
        const ROW_BATCH: usize = 32;
        for start in (0..tokens.len()).step_by(ROW_BATCH) {
            let end = (start + ROW_BATCH).min(tokens.len());
            let rows: Vec<Vec<(usize, f64)>> = (start..end).into_par_iter().map(|i| {
                vectors.iter().enumerate().skip(i + 1).filter_map(|(j, vector)| {
                    let weight = (f64::from(cosine_normalized(&vectors[i], vector)) + 1.0) / 2.0;
                    (weight >= threshold).then_some((j, weight))
                }).collect()
            }).collect();
            for (i, edges) in (start..end).zip(rows) {
                for (j, weight) in edges {
                    writeln!(writer, "<edge source=\"{}\" target=\"{}\"><data key=\"weight\">{weight}</data></edge>", names[i], names[j])?;
                    edge_count += 1;
                }
            }
            if start % 256 == 0 || end == tokens.len() {
                eprintln!("Similarity rows {end}/{}; {edge_count} edges written", tokens.len());
            }
        }
        writeln!(writer, "</graph>\n</graphml>")?;
        Ok(edge_count)
    }).with_context(|| format!("Failed to build {}", output_path.display()))
}

fn valid_xml_char(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}')
}

pub fn validate_threshold(threshold: f64) -> Result<()> {
    ensure!(
        threshold.is_finite() && (0.0..=1.0).contains(&threshold),
        "Weight threshold must be finite and in [0, 1]"
    );
    Ok(())
}

/// Human-readable thresholds without rounding different settings to one filename.
pub fn threshold_label(threshold: f64) -> String {
    let rounded = format!("{threshold:.2}");
    if rounded.parse::<f64>().ok() == Some(threshold) {
        rounded
    } else {
        threshold.to_string()
    }
}

pub fn default_output_for_input(input_path: impl AsRef<Path>) -> PathBuf {
    input_path.as_ref().with_extension("graphml")
}

pub fn filtered_output_for_input(input: &Path, threshold: f64) -> PathBuf {
    let stem = input.file_stem().unwrap_or_default();
    let mut name = match stem.to_str().and_then(|s| s.strip_suffix("-full")) {
        Some(stem) => std::ffi::OsString::from(stem),
        None => stem.to_os_string(),
    };
    name.push(format!("-thresh{}.graphml", threshold_label(threshold)));
    input.with_file_name(name)
}

/// Load a graph for algorithms that require random access. Use a thresholded
/// graph here: graphrs' reader and adjacency structures still require O(n + m) RAM.
pub fn read_graph(path: &Path) -> Result<Graph<String, ()>> {
    let path_string = path
        .to_str()
        .context("Graph input path is not valid UTF-8")?;
    readwrite::graphml::read_graphml_file(path_string, GraphSpecs::undirected())
        .map_err(|error| anyhow::anyhow!("{error}"))
        .with_context(|| format!("Failed to read GraphML from {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_graph_round_trips_weights_and_escaped_tokens() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("nested/graph.graphml");
        let tokens = vec!["学习&中文".into(), "a\"<b".into(), "独立".into()];
        let vectors = vec![vec![1.0, 0.0], vec![0.0, 1.0], vec![-1.0, 0.0]];
        assert_eq!(build_graph_and_write_graphml(&tokens, &vectors, &path)?, 3);
        let graph = read_graph(&path)?;
        assert_eq!(graph.number_of_nodes(), 3);
        assert!(graph
            .get_all_nodes()
            .iter()
            .any(|node| node.name == tokens[0]));
        let mut weights: Vec<_> = graph
            .get_all_edges()
            .iter()
            .map(|edge| edge.weight)
            .collect();
        weights.sort_by(f64::total_cmp);
        assert_eq!(weights, [0.0, 0.5, 0.5]);
        Ok(())
    }

    #[test]
    fn threshold_keeps_all_nodes_including_singletons() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("graph.graphml");
        for count in 0..=2 {
            let tokens = (0..count).map(|i| format!("word{i}")).collect::<Vec<_>>();
            let vectors = [vec![1.0, 0.0], vec![0.0, 1.0]];
            assert_eq!(
                build_threshold_graph(&tokens, &vectors[..count], &path, 0.7)?,
                0
            );
            assert_eq!(read_graph(&path)?.number_of_nodes(), count);
        }
        Ok(())
    }

    #[test]
    fn filename_keeps_precise_threshold_and_dotted_stem() {
        assert_eq!(
            filtered_output_for_input(Path::new("a/my.graph.graphml"), 0.701),
            Path::new("a/my.graph-thresh0.701.graphml")
        );
        assert_eq!(threshold_label(0.7), "0.70");
        assert_ne!(threshold_label(0.701), threshold_label(0.702));
    }
}
