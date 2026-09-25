use super::embeddings::{cosine, dot, transform, Embeddings, FittedTransform, Representation};
use crate::output::{write_atomic, write_json};
use anyhow::{ensure, Result};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet, BinaryHeap},
    io::Write,
    path::Path,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Symmetrization {
    #[default]
    Union,
    Mutual,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum WeightMode {
    #[default]
    Cosine,
    Snn,
    Local,
    CosineSnn,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GraphConfig {
    pub representation: Representation,
    pub k: usize,
    pub symmetrization: Symmetrization,
    pub tau: f64,
    pub weight: WeightMode,
    pub lambda: f64,
    pub k_scale: Option<usize>,
    pub block_rows: usize,
}

impl Default for GraphConfig {
    fn default() -> Self {
        Self {
            representation: Representation::Raw,
            k: 20,
            symmetrization: Symmetrization::Union,
            tau: 0.0,
            weight: WeightMode::Cosine,
            lambda: 0.5,
            k_scale: None,
            block_rows: 32,
        }
    }
}

impl GraphConfig {
    pub fn validate(&self) -> Result<()> {
        ensure!(self.k >= 1, "k must be at least 1");
        ensure!(self.block_rows >= 1, "block_rows must be at least 1");
        ensure!(
            self.tau.is_finite() && (0.0..=1.0).contains(&self.tau),
            "tau must be in [0,1]"
        );
        ensure!(
            self.lambda.is_finite() && (0.0..=1.0).contains(&self.lambda),
            "lambda must be in [0,1]"
        );
        ensure!(
            self.k_scale.is_none_or(|k| k > 0),
            "k_scale must be positive"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Neighbor {
    pub node: usize,
    pub cosine: f64,
    pub distance: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Edge {
    pub u: usize,
    pub v: usize,
    pub weight: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Graph {
    pub tokens: Vec<String>,
    pub edges: Vec<Edge>,
}

impl Graph {
    pub fn validate(&self) -> Result<()> {
        ensure!(!self.tokens.is_empty(), "Empty graph");
        ensure!(
            self.tokens.windows(2).all(|w| w[0] < w[1]),
            "Tokens must be unique and sorted"
        );
        ensure!(self.tokens.iter().all(|s| !s.is_empty() && s.chars().all(|c|
            matches!(c, '\t' | '\n' | '\r' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}'))), "Invalid XML token");
        let mut previous = None;
        for edge in &self.edges {
            ensure!(
                edge.u < edge.v && edge.v < self.tokens.len(),
                "Invalid undirected edge"
            );
            ensure!(
                edge.weight.is_finite() && edge.weight > 0.0,
                "Weights must be finite and strictly positive"
            );
            ensure!(
                previous.is_none_or(|p| p < (edge.u, edge.v)),
                "Duplicate or unordered edges"
            );
            previous = Some((edge.u, edge.v));
        }
        Ok(())
    }
    pub fn adjacency(&self) -> Vec<Vec<(usize, f64)>> {
        let mut adjacency = vec![vec![]; self.tokens.len()];
        for e in &self.edges {
            adjacency[e.u].push((e.v, e.weight));
            adjacency[e.v].push((e.u, e.weight));
        }
        for row in &mut adjacency {
            row.sort_by_key(|e| e.0);
        }
        adjacency
    }
    pub fn degrees(&self) -> Vec<usize> {
        let mut degrees = vec![0; self.tokens.len()];
        for e in &self.edges {
            degrees[e.u] += 1;
            degrees[e.v] += 1;
        }
        degrees
    }
    pub fn median_weight(&self) -> Option<f64> {
        let mut weights: Vec<_> = self.edges.iter().map(|e| e.weight).collect();
        weights.sort_by(f64::total_cmp);
        (!weights.is_empty())
            .then(|| weights[(weights.len() - 1) / 2] / 2.0 + weights[weights.len() / 2] / 2.0)
    }
    pub fn write_graphml(&self, path: &Path) -> Result<()> {
        self.validate()?;
        write_atomic(path, |w| {
            writeln!(w, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<graphml xmlns=\"http://graphml.graphdrawing.org/xmlns\">\n<key id=\"weight\" for=\"edge\" attr.name=\"weight\" attr.type=\"double\"/>\n<graph id=\"G\" edgedefault=\"undirected\">")?;
            let names: Vec<_> = self.tokens.iter().map(quick_xml::escape::escape).collect();
            for name in &names {
                writeln!(w, "<node id=\"{name}\"/>")?;
            }
            for e in &self.edges {
                writeln!(
                    w,
                    "<edge source=\"{}\" target=\"{}\"><data key=\"weight\">{}</data></edge>",
                    names[e.u], names[e.v], e.weight
                )?;
            }
            writeln!(w, "</graph>\n</graphml>")?;
            Ok(())
        })
    }
    /// Load an archived graph, preserving its stored weights unless explicitly unshifted.
    pub fn from_graphml(path: &Path, threshold: f64, unshift: bool) -> Result<Self> {
        ensure!(
            threshold.is_finite() && (0.0..=1.0).contains(&threshold),
            "Invalid archived threshold"
        );
        let source = crate::graph_io::read_graph(path)?;
        let mut tokens: Vec<_> = source
            .get_all_nodes()
            .iter()
            .map(|n| n.name.clone())
            .collect();
        tokens.sort();
        let index: BTreeMap<_, _> = tokens.iter().enumerate().map(|(i, t)| (t, i)).collect();
        let mut edges = vec![];
        for e in source.get_all_edges() {
            ensure!(
                e.weight.is_finite() && (0.0..=1.0).contains(&e.weight),
                "Invalid archived weight"
            );
            if e.weight < threshold {
                continue;
            }
            let weight = if unshift {
                2.0 * e.weight - 1.0
            } else {
                e.weight
            };
            ensure!(
                weight > 0.0,
                "Unshifted controls require positive weights on the same edges"
            );
            let (u, v) = (index[&e.u], index[&e.v]);
            edges.push(Edge {
                u: u.min(v),
                v: u.max(v),
                weight,
            });
        }
        edges.sort_by_key(|e| (e.u, e.v));
        let graph = Self { tokens, edges };
        graph.validate()?;
        Ok(graph)
    }
    pub fn induced(&self, indices: &[usize]) -> Self {
        let mapping: BTreeMap<_, _> = indices
            .iter()
            .enumerate()
            .map(|(i, &old)| (old, i))
            .collect();
        Self {
            tokens: indices.iter().map(|&i| self.tokens[i].clone()).collect(),
            edges: self
                .edges
                .iter()
                .filter_map(|e| {
                    Some(Edge {
                        u: *mapping.get(&e.u)?,
                        v: *mapping.get(&e.v)?,
                        weight: e.weight,
                    })
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PairData {
    pub u: usize,
    pub v: usize,
    pub cosine: f64,
    pub rank_u_to_v: Option<usize>,
    pub rank_v_to_u: Option<usize>,
    pub shared_neighbors: usize,
    pub jaccard: f64,
    pub weight: f64,
    pub retained: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BuiltGraph {
    pub config: GraphConfig,
    pub transform: FittedTransform,
    pub graph: Graph,
    pub neighbors: Vec<Vec<Neighbor>>,
    pub pairs: Vec<PairData>,
    pub k_eff: usize,
    pub k_scale_eff: usize,
    pub zero_overlap_edges_removed: usize,
    pub zero_weight_edges_removed: usize,
    pub below_tau: usize,
}

// Max heap's root is the worst retained neighbor, including lexical tie order.
#[derive(Clone, Copy, Debug)]
struct Ranked {
    node: usize,
    cosine: f64,
}
impl PartialEq for Ranked {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for Ranked {}
impl PartialOrd for Ranked {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Ranked {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .cosine
            .total_cmp(&self.cosine)
            .then(self.node.cmp(&other.node))
    }
}

/// O(nd + nk) storage, no n-by-n similarity matrix, fixed-order f64 dot products.
pub fn exact_neighbors(
    vectors: &[Vec<f64>],
    k: usize,
    block_rows: usize,
) -> Result<Vec<Vec<Neighbor>>> {
    ensure!(k > 0 && block_rows > 0, "k and block_rows must be positive");
    let n = vectors.len();
    let dimension = vectors.first().map_or(0, Vec::len);
    ensure!(n > 0 && dimension > 0, "Neighbors require nonempty vectors");
    ensure!(
        vectors.iter().all(|v| v.len() == dimension
            && v.iter().all(|x| x.is_finite())
            && (dot(v, v) - 1.0).abs() < 1e-10),
        "Neighbors require finite unit vectors with matching dimensions"
    );
    let k = k.min(n.saturating_sub(1));
    let mut result = Vec::with_capacity(n);
    for start in (0..n).step_by(block_rows) {
        let rows: Vec<_> = (start..(start + block_rows).min(n))
            .into_par_iter()
            .map(|i| {
                let mut heap = BinaryHeap::with_capacity(k + 1);
                if k > 0 {
                    for (j, row) in vectors.iter().enumerate() {
                        if i == j {
                            continue;
                        }
                        let candidate = Ranked {
                            node: j,
                            cosine: cosine(&vectors[i], row),
                        };
                        if heap.len() < k {
                            heap.push(candidate);
                        } else if candidate < *heap.peek().unwrap() {
                            *heap.peek_mut().unwrap() = candidate;
                        }
                    }
                }
                let mut row = heap.into_vec();
                row.sort_by(|a, b| b.cosine.total_cmp(&a.cosine).then(a.node.cmp(&b.node)));
                row.into_iter()
                    .map(|r| Neighbor {
                        node: r.node,
                        cosine: r.cosine,
                        distance: (2.0 - 2.0 * r.cosine).max(0.0).sqrt(),
                    })
                    .collect()
            })
            .collect();
        result.extend(rows);
    }
    Ok(result)
}

pub fn build(input: &Embeddings, config: &GraphConfig) -> Result<BuiltGraph> {
    config.validate()?;
    ensure!(
        input.tokens.windows(2).all(|w| w[0] < w[1]),
        "Input tokens must be unique and lexically sorted"
    );
    let (vectors, transform) = transform(input, config.representation)?;
    let neighbors = exact_neighbors(&vectors, config.k, config.block_rows)?;
    let k_eff = config.k.min(input.tokens.len() - 1);
    let k_scale_eff = if k_eff == 0 {
        0
    } else {
        config.k_scale.unwrap_or(7.min(k_eff))
    };
    ensure!(
        k_scale_eff <= k_eff,
        "k_scale exceeds effective neighbor count {k_eff}"
    );
    let sets: Vec<BTreeSet<_>> = neighbors
        .iter()
        .map(|r| r.iter().map(|n| n.node).collect())
        .collect();
    let ranks: Vec<BTreeMap<_, _>> = neighbors
        .iter()
        .map(|r| {
            r.iter()
                .enumerate()
                .map(|(rank, n)| (n.node, rank + 1))
                .collect()
        })
        .collect();
    let mut candidates = BTreeSet::new();
    for (i, row) in neighbors.iter().enumerate() {
        for n in row {
            if config.symmetrization == Symmetrization::Union || sets[n.node].contains(&i) {
                candidates.insert((i.min(n.node), i.max(n.node)));
            }
        }
    }
    let mut result = BuiltGraph {
        config: config.clone(),
        transform,
        graph: Graph {
            tokens: input.tokens.clone(),
            edges: vec![],
        },
        neighbors,
        pairs: vec![],
        k_eff,
        k_scale_eff,
        zero_overlap_edges_removed: 0,
        zero_weight_edges_removed: 0,
        below_tau: 0,
    };
    for (u, v) in candidates {
        let cosine = cosine(&vectors[u], &vectors[v]);
        let shared = sets[u].intersection(&sets[v]).count();
        let jaccard = shared as f64 / (2 * k_eff - shared) as f64;
        let a = cosine.max(0.0);
        let weight = match config.weight {
            WeightMode::Cosine => a,
            WeightMode::Snn => jaccard,
            WeightMode::CosineSnn => a * ((1.0 - config.lambda) + config.lambda * jaccard),
            WeightMode::Local => {
                let sigma_u = result.neighbors[u][k_scale_eff - 1].distance.max(1e-12);
                let sigma_v = result.neighbors[v][k_scale_eff - 1].distance.max(1e-12);
                (-(2.0 - 2.0 * cosine).max(0.0) / (sigma_u * sigma_v)).exp()
            }
        };
        let retained = cosine >= config.tau && weight > 0.0;
        if cosine < config.tau {
            result.below_tau += 1;
        } else if weight == 0.0 {
            result.zero_weight_edges_removed += 1;
            if shared == 0
                && (config.weight == WeightMode::Snn
                    || (config.weight == WeightMode::CosineSnn && config.lambda == 1.0))
            {
                result.zero_overlap_edges_removed += 1;
            }
        }
        if retained {
            result.graph.edges.push(Edge { u, v, weight });
        }
        result.pairs.push(PairData {
            u,
            v,
            cosine,
            rank_u_to_v: ranks[u].get(&v).copied(),
            rank_v_to_u: ranks[v].get(&u).copied(),
            shared_neighbors: shared,
            jaccard,
            weight,
            retained,
        });
    }
    result.graph.validate()?;
    Ok(result)
}

impl BuiltGraph {
    pub fn save(&self, directory: &Path) -> Result<()> {
        std::fs::create_dir_all(directory)?;
        write_json(&directory.join("config.json"), &self.config)?;
        write_json(&directory.join("transform.json"), &self.transform)?;
        self.graph.write_graphml(&directory.join("graph.graphml"))?;
        write_atomic(&directory.join("neighbors.csv"), |w| {
            let mut csv = csv::Writer::from_writer(w);
            csv.write_record(["token", "neighbor", "rank", "cosine", "distance"])?;
            for (i, row) in self.neighbors.iter().enumerate() {
                for (rank, n) in row.iter().enumerate() {
                    csv.serialize((
                        &self.graph.tokens[i],
                        &self.graph.tokens[n.node],
                        rank + 1,
                        n.cosine,
                        n.distance,
                    ))?;
                }
            }
            csv.flush()?;
            Ok(())
        })?;
        write_atomic(&directory.join("pairs.csv"), |w| {
            let mut csv = csv::Writer::from_writer(w);
            csv.write_record([
                "source",
                "target",
                "cosine",
                "rank_source_to_target",
                "rank_target_to_source",
                "shared_neighbors",
                "jaccard",
                "weight",
                "retained",
            ])?;
            for p in &self.pairs {
                csv.serialize((
                    &self.graph.tokens[p.u],
                    &self.graph.tokens[p.v],
                    p.cosine,
                    p.rank_u_to_v,
                    p.rank_v_to_u,
                    p.shared_neighbors,
                    p.jaccard,
                    p.weight,
                    p.retained,
                ))?;
            }
            csv.flush()?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn neighbors_match_brute_force_and_deterministic_ties() -> Result<()> {
        let data = Embeddings::read("5 2\ne -1 0\nd 0 1\nc 1 0\nb 1 0\na 1 0\n".as_bytes())?;
        for k in [1, 3, 9] {
            let actual = exact_neighbors(&data.vectors, k, 2)?;
            assert_eq!(actual, exact_neighbors(&data.vectors, k, 1)?);
            for (i, row) in actual.iter().enumerate() {
                let mut expected: Vec<_> = (0..5)
                    .filter(|&j| j != i)
                    .map(|j| (j, dot(&data.vectors[i], &data.vectors[j])))
                    .collect();
                expected.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
                assert_eq!(
                    row.iter().map(|n| (n.node, n.cosine)).collect::<Vec<_>>(),
                    expected[..k.min(4)]
                );
            }
        }
        let zeros = Embeddings::read("3 2\na 1 -0\nb -0 1\nc 0 -1\n".as_bytes())?;
        assert_eq!(exact_neighbors(&zeros.vectors, 1, 1)?[0][0].node, 1);
        Ok(())
    }
    #[test]
    fn union_mutual_snn_and_duplicate_local_scales() -> Result<()> {
        let data = Embeddings::read("3 2\na 1 0\nb 1 0\nc 1 0\n".as_bytes())?;
        let config = GraphConfig {
            k: 1,
            ..Default::default()
        };
        assert_eq!(build(&data, &config)?.graph.edges.len(), 2);
        assert_eq!(
            build(
                &data,
                &GraphConfig {
                    symmetrization: Symmetrization::Mutual,
                    ..config.clone()
                }
            )?
            .graph
            .edges
            .len(),
            1
        );
        let snn = build(
            &data,
            &GraphConfig {
                weight: WeightMode::Snn,
                ..config.clone()
            },
        )?;
        assert!(snn.graph.edges.is_empty());
        assert_eq!(snn.zero_overlap_edges_removed, 2);
        let local = build(
            &data,
            &GraphConfig {
                weight: WeightMode::Local,
                ..config.clone()
            },
        )?;
        assert!(local.graph.edges.iter().all(|e| e.weight == 1.0));
        let diagonal = Embeddings::read("3 2\na 1 1\nb 1 1\nc 1 1\n".as_bytes())?;
        let diagonal = build(
            &diagonal,
            &GraphConfig {
                weight: WeightMode::Local,
                ..config.clone()
            },
        )?;
        assert!(diagonal.graph.edges.iter().all(|e| e.weight == 1.0));
        assert!(diagonal
            .neighbors
            .iter()
            .flatten()
            .all(|n| n.distance == 0.0));
        let blend = build(
            &data,
            &GraphConfig {
                weight: WeightMode::CosineSnn,
                ..config
            },
        )?;
        assert!(blend.graph.edges.iter().all(|e| e.weight == 0.5));
        let snn = build(
            &data,
            &GraphConfig {
                k: 2,
                weight: WeightMode::Snn,
                ..Default::default()
            },
        )?;
        assert!(snn
            .graph
            .edges
            .iter()
            .all(|e| (e.weight - 1.0 / 3.0).abs() < 1e-12));
        Ok(())
    }
    #[test]
    fn small_graphs_and_graphml_isolates() -> Result<()> {
        let data = Embeddings::read("2 2\n中 1 0\n文 -1 0\n".as_bytes())?;
        let graph = build(&data, &GraphConfig::default())?;
        assert!(graph.graph.edges.is_empty());
        let dir = tempfile::tempdir()?;
        graph.save(dir.path())?;
        let read = Graph::from_graphml(&dir.path().join("graph.graphml"), 0.0, false)?;
        assert_eq!(read.tokens, data.tokens);
        let one = build(
            &data.subset(&[0]),
            &GraphConfig {
                representation: Representation::Center,
                ..Default::default()
            },
        )?;
        assert_eq!(one.k_eff, 0);
        assert_eq!(one.graph.tokens.len(), 1);
        Ok(())
    }

    #[test]
    fn local_formula_and_overlap_use_unfiltered_directed_neighbors() -> Result<()> {
        let data = Embeddings::read("3 2\na 1 0\nb 0.8 0.6\nc 0 1\n".as_bytes())?;
        let local = build(
            &data,
            &GraphConfig {
                k: 2,
                k_scale: Some(1),
                tau: 0.2,
                weight: WeightMode::Local,
                ..Default::default()
            },
        )?;
        assert_eq!(local.graph.edges.len(), 2);
        assert!((local.graph.edges[0].weight - (-1.0_f64).exp()).abs() < 1e-12);
        assert_eq!(local.below_tau, 1);
        assert!(local
            .pairs
            .iter()
            .all(|p| (p.jaccard - 1.0 / 3.0).abs() < 1e-12));
        let snn = build(
            &data,
            &GraphConfig {
                k: 2,
                tau: 0.7,
                weight: WeightMode::Snn,
                ..Default::default()
            },
        )?;
        assert_eq!(snn.graph.edges.len(), 1);
        assert!((snn.graph.edges[0].weight - 1.0 / 3.0).abs() < 1e-12);
        assert!(build(
            &data,
            &GraphConfig {
                k: 1,
                k_scale: Some(2),
                ..Default::default()
            }
        )
        .is_err());
        Ok(())
    }
}
