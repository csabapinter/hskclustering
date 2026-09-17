use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use clap::Parser;
use graphrs::{
    algorithms::{
        centrality::{betweenness, closeness, eigenvector},
        cluster,
    },
    Graph,
};
use hskclustering::graph_io::{read_graph, DEFAULT_FILTERED_GRAPH};

#[derive(Parser, Debug)]
#[command(name = "observe_graph")]
#[command(about = "Inspect a GraphML file and print quick structural stats.")]
struct Args {
    #[arg(long, short = 'i', default_value = DEFAULT_FILTERED_GRAPH)]
    input: PathBuf,

    /// how many entries to show for ranked lists
    #[arg(long, short = 'k', default_value_t = 5)]
    top: usize,

    /// Compute expensive centralities; shortest paths interpret weights as distances
    #[arg(long, conflicts_with = "skip_centrality")]
    centrality: bool,

    /// Accepted for compatibility; centrality is now skipped by default
    #[arg(long, hide = true)]
    skip_centrality: bool,

    /// compute clustering coefficient and transitivity. it might take hours, off by default
    #[arg(long)]
    clustering_metrics: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let graph = read_graph(&args.input)?;

    let all_edges = graph.get_all_edges();
    let missing_weight_count = all_edges.iter().filter(|e| !e.weight.is_finite()).count();
    let mut weights: Vec<f64> = all_edges
        .iter()
        .filter_map(|edge| {
            if !edge.weight.is_finite() {
                None
            } else {
                Some(edge.weight)
            }
        })
        .collect();

    println!("== Graph Summary ==");
    describe_graph_structure(&graph, all_edges.len(), missing_weight_count)?;

    if args.clustering_metrics {
        println!("\n== Clustering Metrics ==");
        describe_clustering_metrics(&graph)?;
    } else {
        println!("\n== Clustering Metrics ==");
        println!("  (skipped; enable with --clustering-metrics)");
    }

    println!("\n== Degree Statistics ==");
    describe_degrees(&graph, args.top);

    println!("\n== Edge Weight Distribution ==");
    describe_weights(&mut weights, missing_weight_count);
    drop(weights);
    drop(all_edges);

    if !args.centrality {
        println!("\n== Centrality Metrics ==");
        println!("  (skipped; enable with --centrality)");
    } else {
        println!("\n== Centrality Metrics (top {}) ==", args.top);
        describe_centralities(&graph, args.top)?;
    }

    Ok(())
}

fn describe_graph_structure(
    graph: &Graph<String, ()>,
    edge_count: usize,
    missing_weight_count: usize,
) -> Result<()> {
    println!("File interpreted as {} graph", direction_label(graph));
    println!("Nodes: {}", graph.number_of_nodes());
    println!("Edges: {}", edge_count);
    println!("Density: {:.6}", graph.get_density());

    if missing_weight_count > 0 {
        println!(
            "Edges with missing/non-finite weights: {} (excluded from weight stats)",
            missing_weight_count
        );
    }

    Ok(())
}

fn describe_clustering_metrics(graph: &Graph<String, ()>) -> Result<()> {
    match cluster::average_clustering(graph, true, None, false) {
        Ok(value) => println!("  Weighted average clustering coefficient: {:.6}", value),
        Err(err) => println!("  Weighted average clustering coefficient: N/A ({})", err),
    }

    match cluster::transitivity(graph) {
        Ok(value) => println!("  Transitivity (unweighted only): {:.6}", value),
        Err(err) => println!("  Transitivity (unweighted only): N/A ({})", err),
    }

    Ok(())
}

fn describe_degrees(graph: &Graph<String, ()>, top: usize) {
    let degree_map = graph.get_degree_for_all_nodes();

    if degree_map.is_empty() {
        println!("  (no nodes)");
        return;
    }

    println!("  Unweighted (edge counts):");
    print_unweighted_degree_stats(&degree_map, top);

    if graph.edges_have_weight() {
        println!("\n  Weighted (sum of incident weights):");
        let weighted_map = graph.get_weighted_degree_for_all_nodes();
        print_weighted_degree_stats(&weighted_map, top);
    } else {
        println!("\n  Weighted degree stats: skipped (graph contains edges without weights)");
    }
}

fn describe_weights(weights: &mut [f64], missing_weight_count: usize) {
    if weights.is_empty() {
        if missing_weight_count == 0 {
            println!("  (graph has no edge weights)");
        } else {
            println!("  (all edges missing weights)");
        }
        return;
    }

    let summary = summarize(weights);
    if let Some(stats) = &summary {
        println!(
            "Count {} | min {:.4} | median {:.4} | mean {:.4} | max {:.4}",
            stats.count, stats.min, stats.median, stats.mean, stats.max
        );
        println!(
            "  Spread: p10 {:.4} | p90 {:.4} | std {:.4}",
            stats.p10, stats.p90, stats.std_dev
        );
        println!("Total weight: {:.6}", weights.iter().sum::<f64>());
    }

    print_ascii_histogram(weights, 20, "  ", "Edge weight histogram");
}

fn describe_centralities(graph: &Graph<String, ()>, top: usize) -> Result<()> {
    anyhow::ensure!(
        graph
            .get_all_edges()
            .iter()
            .all(|edge| edge.weight.is_finite() && edge.weight > 0.0),
        "Weighted path centralities require finite, strictly positive edge lengths"
    );
    println!("  Path metrics treat weights as distances. Similarity weights need a distance model before these values are meaningful.");
    let bet = betweenness::betweenness_centrality(graph, true, true)
        .map_err(|err| anyhow!("betweenness centrality failed: {}", err))?;
    let closeness = closeness::closeness_centrality(graph, true, true)
        .map_err(|err| anyhow!("closeness centrality failed: {}", err))?;
    let eigen = eigenvector::eigenvector_centrality(graph, true, None, None)
        .map_err(|err| anyhow!("eigenvector centrality failed: {}", err))?;

    print_top_float_map("Betweenness", &bet, top);
    print_top_float_map("Closeness", &closeness, top);
    print_top_float_map("Eigenvector", &eigen, top);

    Ok(())
}

#[derive(Debug)]
struct SummaryStats {
    count: usize,
    min: f64,
    max: f64,
    mean: f64,
    median: f64,
    p10: f64,
    p90: f64,
    std_dev: f64,
}

fn summarize(values: &mut [f64]) -> Option<SummaryStats> {
    if values.is_empty() {
        return None;
    }

    values.sort_unstable_by(f64::total_cmp);
    let count = values.len();
    let sum: f64 = values.iter().sum();
    let mean = sum / count as f64;
    let median = percentile(values, 0.5);
    let p10 = percentile(values, 0.1);
    let p90 = percentile(values, 0.9);
    let min = values.first().copied().unwrap();
    let max = values.last().copied().unwrap();
    let variance = values
        .iter()
        .map(|v| {
            let diff = *v - mean;
            diff * diff
        })
        .sum::<f64>()
        / count as f64;

    Some(SummaryStats {
        count,
        min,
        max,
        mean,
        median,
        p10,
        p90,
        std_dev: variance.sqrt(),
    })
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    if sorted.len() == 1 {
        return sorted[0];
    }
    let clamped = p.clamp(0.0, 1.0);
    let position = clamped * (sorted.len() - 1) as f64;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    if lower == upper {
        return sorted[lower];
    }
    let weight = position - lower as f64;
    sorted[lower] + (sorted[upper] - sorted[lower]) * weight
}

#[derive(Debug)]
struct HistBin {
    start: f64,
    end: f64,
    count: usize,
}

fn histogram(values: &[f64], bins: usize) -> Vec<HistBin> {
    if values.is_empty() || bins == 0 {
        return vec![];
    }
    let min = values.iter().fold(f64::INFINITY, |acc, v| acc.min(*v));
    let max = values.iter().fold(f64::NEG_INFINITY, |acc, v| acc.max(*v));

    if (max - min).abs() < f64::EPSILON {
        return vec![HistBin {
            start: min,
            end: max,
            count: values.len(),
        }];
    }

    let width = (max - min) / bins as f64;
    let mut counts = vec![0usize; bins];
    for &value in values {
        let mut idx = ((value - min) / width).floor() as usize;
        if idx >= bins {
            idx = bins - 1;
        }
        counts[idx] += 1;
    }

    counts
        .into_iter()
        .enumerate()
        .map(|(i, count)| HistBin {
            start: min + i as f64 * width,
            end: if i == bins - 1 {
                max
            } else {
                min + (i + 1) as f64 * width
            },
            count,
        })
        .collect()
}

fn print_unweighted_degree_stats(degree_map: &HashMap<String, usize>, top: usize) {
    let mut degree_values: Vec<f64> = degree_map.values().map(|&d| d as f64).collect();
    if let Some(stats) = summarize(&mut degree_values) {
        println!(
            "    Nodes: {} | min {:.0} | median {:.2} | mean {:.2} | max {:.0}",
            stats.count, stats.min, stats.median, stats.mean, stats.max
        );
        println!(
            "      Spread: p10 {:.2} | p90 {:.2} | std {:.2}",
            stats.p10, stats.p90, stats.std_dev
        );
    }

    let hist = degree_histogram(degree_map);
    print_degree_histogram("    Lowest degrees", hist.iter(), 5);
    print_degree_histogram("    Highest degrees", hist.iter().rev(), 5);

    println!("    Top nodes by degree:");
    for (name, value) in top_entries_usize(degree_map, top) {
        println!("      {} -> {}", name, value);
    }
}

fn print_weighted_degree_stats(weighted_map: &HashMap<String, f64>, top: usize) {
    if weighted_map.is_empty() {
        println!("    (no weights)");
        return;
    }

    let mut values: Vec<f64> = weighted_map.values().copied().collect();
    if let Some(stats) = summarize(&mut values) {
        println!(
            "    Nodes: {} | min {:.6} | median {:.6} | mean {:.6} | max {:.6}",
            stats.count, stats.min, stats.median, stats.mean, stats.max
        );
        println!(
            "      Spread: p10 {:.6} | p90 {:.6} | std {:.6}",
            stats.p10, stats.p90, stats.std_dev
        );
    }

    print_weighted_degree_histogram(&values, 20);
    print_weighted_degree_cdf(&values);

    println!("    Top nodes by weighted degree:");
    for (name, value) in top_entries_f64(weighted_map, top) {
        println!("      {} -> {:.6}", name, value);
    }
}

fn print_weighted_degree_histogram(values: &[f64], bins: usize) {
    print_ascii_histogram(values, bins, "    ", "Histogram");
}

fn print_ascii_histogram(values: &[f64], bins: usize, indent: &str, title: &str) {
    println!("{}{} ({} bins):", indent, title, bins);
    let bins = histogram(values, bins);
    if bins.is_empty() {
        println!("{}  (insufficient data)", indent);
        return;
    }
    let max_count = bins.iter().map(|b| b.count).max().unwrap_or(0).max(1);
    for bin in bins {
        let bar_len = ((bin.count as f64 / max_count as f64) * 40.0).round() as usize;
        let bar = match bin.count {
            0 => String::new(),
            _ => "#".repeat(bar_len.max(1)),
        };
        println!(
            "{}  [{:.6}, {:.6}] {:>8} {}",
            indent, bin.start, bin.end, bin.count, bar
        );
    }
}

fn print_weighted_degree_cdf(sorted_values: &[f64]) {
    if sorted_values.is_empty() {
        println!("    CDF: (no data)");
        return;
    }
    println!("    Weighted degree CDF:");
    let marks = [0.0_f64, 0.1, 0.25, 0.5, 0.75, 0.9, 0.95, 0.99, 1.0];
    for mark in marks {
        let pct = (mark * 100.0).round();
        let value = percentile(sorted_values, mark);
        println!("      p{:>5.0}: {:.6}", pct, value);
    }
}

fn degree_histogram(degrees: &HashMap<String, usize>) -> BTreeMap<usize, usize> {
    let mut hist = BTreeMap::new();
    for &value in degrees.values() {
        *hist.entry(value).or_insert(0) += 1;
    }
    hist
}

fn print_degree_histogram<'a, I>(label: &str, iter: I, limit: usize)
where
    I: Iterator<Item = (&'a usize, &'a usize)>,
{
    println!("{}:", label);
    for (idx, (degree, count)) in iter.enumerate() {
        if idx >= limit {
            if limit > 0 {
                println!("    …");
            }
            break;
        }
        println!("    degree {:>3} -> {} nodes", degree, count);
    }
}

fn top_entries_usize(map: &HashMap<String, usize>, k: usize) -> Vec<(String, usize)> {
    if k == 0 {
        return Vec::new();
    }
    let mut entries: Vec<(String, usize)> = map.iter().map(|(k, &v)| (k.clone(), v)).collect();
    entries.sort_by(|a, b| match b.1.cmp(&a.1) {
        Ordering::Equal => a.0.cmp(&b.0),
        other => other,
    });
    entries.truncate(k);
    entries
}

fn print_top_float_map(title: &str, map: &HashMap<String, f64>, k: usize) {
    println!("  {}:", title);
    if map.is_empty() {
        println!("    (no data)");
        return;
    }
    if k == 0 {
        println!("    (top = 0)");
        return;
    }
    for (name, value) in top_entries_f64(map, k) {
        println!("    {} -> {:.6}", name, value);
    }
}

fn top_entries_f64(map: &HashMap<String, f64>, k: usize) -> Vec<(String, f64)> {
    let mut entries: Vec<(String, f64)> = map.iter().map(|(k, &v)| (k.clone(), v)).collect();
    entries.sort_by(|a, b| match b.1.total_cmp(&a.1) {
        Ordering::Equal => a.0.cmp(&b.0),
        other => other,
    });
    entries.truncate(k);
    entries
}

fn direction_label(graph: &Graph<String, ()>) -> &'static str {
    if graph.specs.directed {
        "directed"
    } else {
        "undirected"
    }
}
