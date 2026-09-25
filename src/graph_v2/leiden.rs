//! Weighted CPM implementation of Traag et al., Algorithm A.2.
//! https://arxiv.org/html/1810.08473v3#A1
//! Separate from graphrs so legacy traversal and random behavior stay unchanged.
use super::graph::Graph;
use anyhow::{bail, ensure, Result};
use rand::{seq::SliceRandom, Rng, SeedableRng};
use rand_xoshiro::Xoshiro256PlusPlus;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub const BACKEND: &str = "hsk-leiden-cpm/1; xoshiro256++ rand_xoshiro-0.6.0";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LeidenConfig {
    pub seed: u64,
    pub resolution: f64,
    pub theta: f64,
    pub max_iterations: usize,
    pub max_levels: usize,
    pub max_local_moves: usize,
}

impl Default for LeidenConfig {
    fn default() -> Self {
        Self {
            seed: 42,
            resolution: 0.05,
            theta: 0.3,
            max_iterations: 100,
            max_levels: 1000,
            max_local_moves: 10_000_000,
        }
    }
}
impl LeidenConfig {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.resolution.is_finite() && self.resolution >= 0.0,
            "Resolution must be finite and nonnegative"
        );
        ensure!(
            self.theta.is_finite() && self.theta > 0.0,
            "Theta must be finite and positive"
        );
        ensure!(
            self.max_iterations > 0 && self.max_levels > 0 && self.max_local_moves > 0,
            "Solver limits must be positive"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LeidenRun {
    pub backend: String,
    pub config: LeidenConfig,
    pub gamma: f64,
    pub labels: Vec<usize>,
    pub objective: f64,
    pub iterations: usize,
    pub levels: usize,
    pub moves: usize,
    pub converged: bool,
}

pub fn canonicalize(labels: &[usize]) -> Vec<usize> {
    let mut mapping = BTreeMap::new();
    labels
        .iter()
        .map(|&label| {
            let next = mapping.len();
            *mapping.entry(label).or_insert(next)
        })
        .collect()
}

pub fn groups(labels: &[usize]) -> Vec<Vec<usize>> {
    let canonical = canonicalize(labels);
    let mut result = vec![vec![]; canonical.iter().max().map_or(0, |x| x + 1)];
    for (i, g) in canonical.into_iter().enumerate() {
        result[g].push(i);
    }
    result
}

pub fn objective(graph: &Graph, labels: &[usize], resolution: f64) -> Result<f64> {
    ensure!(labels.len() == graph.tokens.len(), "Incomplete assignments");
    let internal: f64 = graph
        .edges
        .iter()
        .filter(|e| labels[e.u] == labels[e.v])
        .map(|e| e.weight)
        .sum();
    let penalty: f64 = groups(labels)
        .iter()
        .map(|g| resolution * g.len() as f64 * (g.len() - 1) as f64 / 2.0)
        .sum();
    let result = internal - penalty;
    ensure!(result.is_finite(), "Nonfinite CPM objective");
    Ok(result)
}

/// Number of induced connected components in each canonical community.
pub fn community_components(graph: &Graph, labels: &[usize]) -> Result<Vec<usize>> {
    ensure!(labels.len() == graph.tokens.len(), "Incomplete assignments");
    let labels = canonicalize(labels);
    let mut counts = vec![0; labels.iter().max().map_or(0, |x| x + 1)];
    let adjacency = graph.adjacency();
    let mut seen = vec![false; labels.len()];
    for start in 0..labels.len() {
        if seen[start] {
            continue;
        }
        counts[labels[start]] += 1;
        seen[start] = true;
        let mut stack = vec![start];
        while let Some(i) = stack.pop() {
            for &(j, _) in &adjacency[i] {
                if !seen[j] && labels[i] == labels[j] {
                    seen[j] = true;
                    stack.push(j);
                }
            }
        }
    }
    Ok(counts)
}

#[derive(Clone)]
struct Level {
    adjacency: Vec<Vec<(usize, f64)>>,
    mass: Vec<usize>,
    originals: Vec<Vec<usize>>,
}

struct Partition {
    labels: Vec<usize>,
    mass: Vec<usize>,
    count: Vec<usize>,
    empty: BTreeSet<usize>,
}
impl Partition {
    fn new(level: &Level, labels: Vec<usize>) -> Self {
        let labels = canonicalize(&labels);
        let mut mass = vec![0; labels.len()];
        let mut count = vec![0; labels.len()];
        for (v, &c) in labels.iter().enumerate() {
            mass[c] += level.mass[v];
            count[c] += 1;
        }
        let empty = (0..labels.len()).filter(|&c| count[c] == 0).collect();
        Self {
            labels,
            mass,
            count,
            empty,
        }
    }
    fn move_node(&mut self, level: &Level, v: usize, target: usize) {
        let source = self.labels[v];
        if source == target {
            return;
        }
        self.mass[source] -= level.mass[v];
        self.count[source] -= 1;
        self.mass[target] += level.mass[v];
        self.count[target] += 1;
        if self.count[source] == 0 {
            self.empty.insert(source);
        }
        self.empty.remove(&target);
        self.labels[v] = target;
    }
}

fn local_move(
    level: &Level,
    labels: Vec<usize>,
    cfg: &LeidenConfig,
    rng: &mut Xoshiro256PlusPlus,
) -> Result<(Vec<usize>, usize)> {
    let n = labels.len();
    let mut p = Partition::new(level, labels);
    let mut order: Vec<_> = (0..n).collect();
    order.shuffle(rng);
    let mut queue: VecDeque<_> = order.into();
    let mut queued = vec![true; n];
    let mut moves = 0;
    while let Some(v) = queue.pop_front() {
        queued[v] = false;
        let source = p.labels[v];
        let mass = level.mass[v] as f64;
        let mut cuts = BTreeMap::<usize, f64>::new();
        for &(u, weight) in &level.adjacency[v] {
            *cuts.entry(p.labels[u]).or_default() += weight;
        }
        let stay = cuts.get(&source).copied().unwrap_or(0.0)
            - cfg.resolution * mass * (p.mass[source] - level.mass[v]) as f64;
        if let Some(&empty) = p.empty.first() {
            cuts.insert(empty, 0.0);
        }
        let mut target = source;
        let mut best = stay;
        for (c, cut) in cuts {
            if c == source {
                continue;
            }
            let score = cut - cfg.resolution * mass * p.mass[c] as f64;
            ensure!(
                score.is_finite() && stay.is_finite(),
                "Nonfinite local gain"
            );
            let tolerance = 32.0 * f64::EPSILON * score.abs().max(best.abs());
            if score > best + tolerance {
                best = score;
                target = c;
            }
        }
        if target != source {
            moves += 1;
            ensure!(
                moves <= cfg.max_local_moves,
                "Leiden local move limit exceeded"
            );
            p.move_node(level, v, target);
            for &(u, _) in &level.adjacency[v] {
                if p.labels[u] != target && !queued[u] {
                    queued[u] = true;
                    queue.push_back(u);
                }
            }
        }
    }
    Ok((canonicalize(&p.labels), moves))
}

fn refine(
    level: &Level,
    parent: &[usize],
    cfg: &LeidenConfig,
    rng: &mut Xoshiro256PlusPlus,
) -> Vec<usize> {
    let n = parent.len();
    let mut refined = Partition::new(level, (0..n).collect());
    let parents = groups(parent);
    let mut cut: Vec<f64> = (0..n)
        .map(|v| {
            level.adjacency[v]
                .iter()
                .filter(|&&(u, _)| parent[u] == parent[v])
                .map(|e| e.1)
                .sum()
        })
        .collect();
    for mut members in parents {
        let total: usize = members.iter().map(|&v| level.mass[v]).sum();
        let mut eligible: Vec<_> = members
            .drain(..)
            .filter(|&v| {
                cut[v] >= cfg.resolution * level.mass[v] as f64 * (total - level.mass[v]) as f64
            })
            .collect();
        eligible.shuffle(rng);
        for v in eligible {
            let source = refined.labels[v];
            if refined.count[source] != 1 {
                continue;
            }
            let mut cuts = BTreeMap::<usize, f64>::new();
            for &(u, w) in &level.adjacency[v] {
                if parent[u] == parent[v] {
                    *cuts.entry(refined.labels[u]).or_default() += w;
                }
            }
            let mut candidates = vec![(source, 0.0, 0.0)];
            for (c, weight) in cuts {
                if c == source {
                    continue;
                }
                let mass = refined.mass[c];
                if cut[c] < cfg.resolution * mass as f64 * (total - mass) as f64 {
                    continue;
                }
                let gain = weight - cfg.resolution * level.mass[v] as f64 * mass as f64;
                if gain >= 0.0 {
                    candidates.push((c, gain, weight));
                }
            }
            candidates.sort_by_key(|x| x.0);
            let maximum = candidates.iter().map(|c| c.1).fold(0.0, f64::max);
            let weights: Vec<_> = candidates
                .iter()
                .map(|c| ((c.1 - maximum) / cfg.theta).exp())
                .collect();
            let mut draw = rng.gen::<f64>() * weights.iter().sum::<f64>();
            let mut chosen = candidates.len() - 1;
            for (i, weight) in weights.iter().enumerate() {
                draw -= weight;
                if draw < 0.0 {
                    chosen = i;
                    break;
                }
            }
            let (target, _, edge_cut) = candidates[chosen];
            if target != source {
                cut[target] = (cut[target] + cut[source] - 2.0 * edge_cut).max(0.0);
                cut[source] = 0.0;
                refined.move_node(level, v, target);
            }
        }
    }
    canonicalize(&refined.labels)
}

fn aggregate(level: &Level, refined: &[usize], parent: &[usize]) -> (Level, Vec<usize>) {
    let groups = groups(refined);
    let n = groups.len();
    let mut next = Level {
        adjacency: vec![vec![]; n],
        mass: vec![0; n],
        originals: vec![vec![]; n],
    };
    let mut next_labels = vec![0; n];
    for (c, members) in groups.iter().enumerate() {
        next_labels[c] = parent[members[0]];
        for &v in members {
            next.mass[c] += level.mass[v];
            next.originals[c].extend(&level.originals[v]);
        }
        next.originals[c].sort_unstable();
    }
    let mut edges = BTreeMap::<(usize, usize), f64>::new();
    for (v, row) in level.adjacency.iter().enumerate() {
        for &(u, weight) in row {
            if v >= u || refined[v] == refined[u] {
                continue;
            }
            let (a, b) = (refined[v].min(refined[u]), refined[v].max(refined[u]));
            *edges.entry((a, b)).or_default() += weight;
        }
    }
    for ((a, b), w) in edges {
        next.adjacency[a].push((b, w));
        next.adjacency[b].push((a, w));
    }
    for row in &mut next.adjacency {
        row.sort_by_key(|x| x.0);
    }
    (next, canonicalize(&next_labels))
}

pub fn run(graph: &Graph, config: &LeidenConfig) -> Result<LeidenRun> {
    config.validate()?;
    graph.validate()?;
    let n = graph.tokens.len();
    let initial = Level {
        adjacency: graph.adjacency(),
        mass: vec![1; n],
        originals: (0..n).map(|i| vec![i]).collect(),
    };
    let mut rng = Xoshiro256PlusPlus::seed_from_u64(config.seed);
    let mut labels: Vec<_> = (0..n).collect();
    let mut levels = 0;
    let mut moves = 0;
    if graph.edges.is_empty() {
        return Ok(LeidenRun {
            backend: BACKEND.into(),
            config: config.clone(),
            gamma: config.resolution,
            labels,
            objective: 0.0,
            iterations: 0,
            levels: 0,
            moves: 0,
            converged: true,
        });
    }
    let mut previous_q = objective(graph, &labels, config.resolution)?;
    for iteration in 1..=config.max_iterations {
        let previous = labels.clone();
        let mut level = initial.clone();
        let mut partition = labels;
        let mut flattened = None;
        for _ in 0..config.max_levels {
            levels += 1;
            let (updated, moved) = local_move(&level, partition, config, &mut rng)?;
            partition = updated;
            moves += moved;
            if groups(&partition).len() == level.mass.len() {
                let mut flat = vec![0; n];
                for (v, members) in level.originals.iter().enumerate() {
                    for &i in members {
                        flat[i] = partition[v];
                    }
                }
                flattened = Some(canonicalize(&flat));
                break;
            }
            let refined = refine(&level, &partition, config, &mut rng);
            (level, partition) = aggregate(&level, &refined, &partition);
        }
        labels =
            flattened.ok_or_else(|| anyhow::anyhow!("Leiden aggregation level limit exceeded"))?;
        let q = objective(graph, &labels, config.resolution)?;
        ensure!(
            q + 1e-10 * previous_q.abs().max(1.0) >= previous_q,
            "CPM objective decreased"
        );
        ensure!(
            community_components(graph, &labels)?
                .iter()
                .all(|&c| c == 1),
            "Leiden returned a disconnected community"
        );
        if labels == previous {
            return Ok(LeidenRun {
                backend: BACKEND.into(),
                config: config.clone(),
                gamma: config.resolution,
                labels,
                objective: q,
                iterations: iteration,
                levels,
                moves,
                converged: true,
            });
        }
        previous_q = q;
    }
    bail!(
        "Leiden did not converge in {} iterations",
        config.max_iterations
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph_v2::graph::Edge;
    #[test]
    fn seeded_repeatability_isolates_and_direct_quality() -> Result<()> {
        let graph = Graph {
            tokens: (0..7).map(|i| format!("n{i}")).collect(),
            edges: vec![
                Edge {
                    u: 0,
                    v: 1,
                    weight: 1.0,
                },
                Edge {
                    u: 0,
                    v: 2,
                    weight: 1.0,
                },
                Edge {
                    u: 1,
                    v: 2,
                    weight: 1.0,
                },
                Edge {
                    u: 2,
                    v: 3,
                    weight: 0.01,
                },
                Edge {
                    u: 3,
                    v: 4,
                    weight: 1.0,
                },
                Edge {
                    u: 3,
                    v: 5,
                    weight: 1.0,
                },
                Edge {
                    u: 4,
                    v: 5,
                    weight: 1.0,
                },
            ],
        };
        for seed in 0..10 {
            let cfg = LeidenConfig {
                seed,
                resolution: 0.2,
                ..Default::default()
            };
            let result = run(&graph, &cfg)?;
            assert_eq!(result.labels, [0, 0, 0, 1, 1, 1, 2]);
            assert_eq!(result.labels, run(&graph, &cfg)?.labels);
            assert!((result.objective - 4.8).abs() < 1e-12);
        }
        Ok(())
    }
    #[test]
    fn tiny_weights_and_zero_resolution() -> Result<()> {
        let graph = Graph {
            tokens: vec!["a".into(), "b".into(), "c".into()],
            edges: vec![Edge {
                u: 0,
                v: 1,
                weight: 1e-100,
            }],
        };
        let run = run(
            &graph,
            &LeidenConfig {
                resolution: 0.0,
                ..Default::default()
            },
        )?;
        assert_eq!(run.labels, [0, 0, 1]);
        Ok(())
    }

    #[test]
    fn every_four_node_topology_has_connected_node_optimal_output() -> Result<()> {
        let pairs = [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)];
        for mask in 0..64 {
            let graph = Graph {
                tokens: (0..4).map(|i| format!("n{i}")).collect(),
                edges: pairs
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| mask & (1 << i) != 0)
                    .map(|(i, &(u, v))| Edge {
                        u,
                        v,
                        weight: 0.3 * (1 + i % 3) as f64,
                    })
                    .collect(),
            };
            for resolution in [0.0, 0.2, 0.7] {
                for seed in [1, 29] {
                    let run = run(
                        &graph,
                        &LeidenConfig {
                            seed,
                            resolution,
                            ..Default::default()
                        },
                    )?;
                    let direct = |labels: &[usize]| {
                        let mut q = 0.0;
                        for (u, v) in pairs {
                            if labels[u] == labels[v] {
                                q += graph
                                    .edges
                                    .iter()
                                    .find(|e| e.u == u && e.v == v)
                                    .map_or(0.0, |e| e.weight)
                                    - resolution;
                            }
                        }
                        q
                    };
                    assert!((direct(&run.labels) - run.objective).abs() < 1e-12);
                    assert!(community_components(&graph, &run.labels)?
                        .iter()
                        .all(|&c| c == 1));
                    for v in 0..4 {
                        for target in 0..=4 {
                            let mut modified = run.labels.clone();
                            modified[v] = target;
                            assert!(
                                direct(&modified) <= run.objective + 1e-12,
                                "mask={mask}, r={resolution}, seed={seed}"
                            );
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
