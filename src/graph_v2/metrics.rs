use super::{
    embeddings::{dot, Embeddings},
    graph::{BuiltGraph, Graph},
    leiden::{canonicalize, community_components, groups},
};
use anyhow::{ensure, Result};
use ndarray::Array2;
use rand::{seq::SliceRandom, SeedableRng};
use rand_xoshiro::Xoshiro256PlusPlus;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Metric {
    pub value: Option<f64>,
    pub reason: Option<String>,
}
impl Metric {
    pub fn value(value: f64) -> Self {
        Self {
            value: Some(value),
            reason: None,
        }
    }
    pub fn missing(reason: &str) -> Self {
        Self {
            value: None,
            reason: Some(reason.into()),
        }
    }
    pub fn mean(values: impl IntoIterator<Item = f64>, reason: &str) -> Self {
        let values: Vec<_> = values.into_iter().collect();
        if values.is_empty() {
            Self::missing(reason)
        } else {
            Self::value(values.iter().sum::<f64>() / values.len() as f64)
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Distribution {
    pub count: usize,
    pub mean: Option<f64>,
    pub stddev: Option<f64>,
    pub min: Option<f64>,
    pub q25: Option<f64>,
    pub median: Option<f64>,
    pub q75: Option<f64>,
    pub q95: Option<f64>,
    pub max: Option<f64>,
    pub reason: Option<String>,
}
pub fn distribution(values: impl IntoIterator<Item = f64>) -> Distribution {
    let mut x: Vec<_> = values.into_iter().collect();
    x.sort_by(f64::total_cmp);
    let n = x.len();
    let q = |p: f64| {
        if n == 0 {
            None
        } else {
            let i = p * (n - 1) as f64;
            let lo = i.floor() as usize;
            let hi = i.ceil() as usize;
            Some(x[lo] * (1.0 - i.fract()) + x[hi] * i.fract())
        }
    };
    let mean = (n > 0).then(|| x.iter().sum::<f64>() / n as f64);
    Distribution {
        count: n,
        mean,
        stddev: mean.map(|m| (x.iter().map(|v| (v - m).powi(2)).sum::<f64>() / n as f64).sqrt()),
        min: q(0.0),
        q25: q(0.25),
        median: q(0.5),
        q75: q(0.75),
        q95: q(0.95),
        max: q(1.0),
        reason: (n == 0).then(|| "No observations".into()),
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphMetrics {
    pub nodes: usize,
    pub edges: usize,
    pub degree: Distribution,
    pub strength: Distribution,
    pub weight: Distribution,
    pub isolates: usize,
    pub components: usize,
    pub largest_component_fraction: f64,
    pub neighbor_reciprocity: Metric,
    pub incoming_neighbor_count: Option<Distribution>,
    pub incoming_neighbor_skewness: Metric,
    pub incoming_neighbor_max: Option<usize>,
    pub zero_overlap_edges_removed: Option<usize>,
    pub zero_weight_edges_removed: Option<usize>,
}
pub fn graph_metrics(graph: &Graph, built: Option<&BuiltGraph>) -> Result<GraphMetrics> {
    graph.validate()?;
    let n = graph.tokens.len();
    let adjacency = graph.adjacency();
    let degrees = graph.degrees();
    let mut sizes = vec![];
    let mut seen = vec![false; n];
    for root in 0..n {
        if seen[root] {
            continue;
        }
        seen[root] = true;
        let mut stack = vec![root];
        let mut size = 0;
        while let Some(v) = stack.pop() {
            size += 1;
            for &(u, _) in &adjacency[v] {
                if !seen[u] {
                    seen[u] = true;
                    stack.push(u);
                }
            }
        }
        sizes.push(size);
    }
    let missing = "Directed neighbor lists unavailable for archived graph";
    let mut result = GraphMetrics {
        nodes: n,
        edges: graph.edges.len(),
        degree: distribution(degrees.iter().map(|&d| d as f64)),
        strength: distribution(adjacency.iter().map(|r| r.iter().map(|e| e.1).sum())),
        weight: distribution(graph.edges.iter().map(|e| e.weight)),
        isolates: degrees.iter().filter(|&&d| d == 0).count(),
        components: sizes.len(),
        largest_component_fraction: *sizes.iter().max().unwrap() as f64 / n as f64,
        neighbor_reciprocity: Metric::missing(missing),
        incoming_neighbor_count: None,
        incoming_neighbor_skewness: Metric::missing(missing),
        incoming_neighbor_max: None,
        zero_overlap_edges_removed: built.map(|b| b.zero_overlap_edges_removed),
        zero_weight_edges_removed: built.map(|b| b.zero_weight_edges_removed),
    };
    if let Some(built) = built {
        let mut incoming = vec![0usize; n];
        let mut total = 0;
        let mut reciprocated = 0;
        let sets: Vec<BTreeSet<_>> = built
            .neighbors
            .iter()
            .map(|r| r.iter().map(|x| x.node).collect())
            .collect();
        for (i, row) in built.neighbors.iter().enumerate() {
            for neighbor in row {
                incoming[neighbor.node] += 1;
                total += 1;
                if sets[neighbor.node].contains(&i) {
                    reciprocated += 1;
                }
            }
        }
        result.neighbor_reciprocity = if total > 0 {
            Metric::value(reciprocated as f64 / total as f64)
        } else {
            Metric::missing("No directed neighbors")
        };
        let mean = total as f64 / n as f64;
        let variance = incoming
            .iter()
            .map(|&v| (v as f64 - mean).powi(2))
            .sum::<f64>()
            / n as f64;
        result.incoming_neighbor_skewness = if variance > 0.0 {
            Metric::value(
                incoming
                    .iter()
                    .map(|&v| (v as f64 - mean).powi(3))
                    .sum::<f64>()
                    / n as f64
                    / variance.powf(1.5),
            )
        } else {
            Metric::missing("Incoming counts have zero variance")
        };
        result.incoming_neighbor_max = incoming.iter().max().copied();
        result.incoming_neighbor_count = Some(distribution(incoming.iter().map(|&x| x as f64)));
    }
    Ok(result)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WordMetrics {
    pub community: usize,
    pub size: usize,
    pub cohesion: Option<f64>,
    pub silhouette: Option<f64>,
    pub margin: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PartitionMetrics {
    pub tokens: usize,
    pub communities: usize,
    pub sizes: Distribution,
    pub singleton_token_fraction: f64,
    pub pair_token_fraction: f64,
    pub singleton_community_fraction: f64,
    pub pair_community_fraction: f64,
    pub largest_community_share: f64,
    pub fraction_in_sizes_3_to_30: f64,
    pub fraction_in_sizes_over_100: f64,
    pub nonsingleton_coverage: f64,
    pub induced_components: Vec<usize>,
    pub all_communities_connected: bool,
    pub cohesion: Metric,
    pub silhouette: Metric,
    pub margin: Metric,
    pub margin_distribution: Distribution,
    pub external_reference_agreement: Metric,
    pub independent_space_coherence: Metric,
}

/// Exact cosine distances via community means. O(nd + kd + batch*k) storage.
pub fn partition_metrics(
    input: &Embeddings,
    graph: &Graph,
    labels: &[usize],
) -> Result<(PartitionMetrics, Vec<WordMetrics>)> {
    ensure!(
        input.tokens == graph.tokens && labels.len() == input.tokens.len(),
        "Metrics require complete, aligned assignments"
    );
    let labels = canonicalize(labels);
    let communities = groups(&labels);
    let n = labels.len();
    let k = communities.len();
    let d = input.vectors[0].len();
    let mut means = Array2::<f64>::zeros((k, d));
    for (c, members) in communities.iter().enumerate() {
        for &v in members {
            for j in 0..d {
                means[[c, j]] += input.vectors[v][j] / members.len() as f64;
            }
        }
    }
    let defined = k > 1 && k < n;
    let mut words = Vec::with_capacity(n);
    for (batch, rows) in input.vectors.chunks(128).enumerate() {
        let records =
            Array2::from_shape_vec((rows.len(), d), rows.iter().flatten().copied().collect())?;
        let similarities = records.dot(&means.t());
        for (i, vector) in rows.iter().enumerate() {
            let c = labels[batch * 128 + i];
            let size = communities[c].len();
            let mut word = WordMetrics {
                community: c,
                size,
                cohesion: None,
                silhouette: (defined || size == 1).then_some(0.0),
                margin: None,
            };
            if size > 1 {
                let own = ((similarities[[i, c]] * size as f64 - dot(vector, vector))
                    / (size - 1) as f64)
                    .clamp(-1.0, 1.0);
                word.cohesion = Some(own);
                if k > 1 {
                    let other = (0..k)
                        .filter(|&h| h != c)
                        .map(|h| similarities[[i, h]])
                        .fold(-1.0, f64::max)
                        .clamp(-1.0, 1.0);
                    let a = 1.0 - own;
                    let b = 1.0 - other;
                    word.silhouette = Some(if a.max(b) > 0.0 {
                        (b - a) / a.max(b)
                    } else {
                        0.0
                    });
                    word.margin = Some(own - other);
                }
            }
            words.push(word);
        }
    }
    let singleton_count = communities.iter().filter(|g| g.len() == 1).count();
    let pair_count = communities.iter().filter(|g| g.len() == 2).count();
    let induced_components = community_components(graph, &labels)?;
    let metrics = PartitionMetrics {
        tokens: n,
        communities: k,
        sizes: distribution(communities.iter().map(|g| g.len() as f64)),
        singleton_token_fraction: singleton_count as f64 / n as f64,
        pair_token_fraction: (2 * pair_count) as f64 / n as f64,
        singleton_community_fraction: singleton_count as f64 / k as f64,
        pair_community_fraction: pair_count as f64 / k as f64,
        largest_community_share: communities.iter().map(Vec::len).max().unwrap() as f64 / n as f64,
        fraction_in_sizes_3_to_30: communities
            .iter()
            .filter(|g| (3..=30).contains(&g.len()))
            .map(Vec::len)
            .sum::<usize>() as f64
            / n as f64,
        fraction_in_sizes_over_100: communities
            .iter()
            .filter(|g| g.len() > 100)
            .map(Vec::len)
            .sum::<usize>() as f64
            / n as f64,
        nonsingleton_coverage: 1.0 - singleton_count as f64 / n as f64,
        all_communities_connected: induced_components.iter().all(|&c| c == 1),
        induced_components,
        cohesion: Metric::mean(
            words.iter().filter_map(|w| w.cohesion),
            "No nonsingleton tokens",
        ),
        silhouette: if defined {
            Metric::mean(words.iter().filter_map(|w| w.silhouette), "No tokens")
        } else {
            Metric::missing("Silhouette undefined for one community or all singletons")
        },
        margin: Metric::mean(
            words.iter().filter_map(|w| w.margin),
            "Margins require nonsingleton tokens and at least two communities",
        ),
        margin_distribution: distribution(words.iter().filter_map(|w| w.margin)),
        external_reference_agreement: Metric::missing("No external evaluation data available"),
        independent_space_coherence: Metric::missing("No independent embeddings available"),
    };
    Ok((metrics, words))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Agreement {
    pub ari: f64,
    pub variation_of_information: f64,
    pub left_best_jaccard: Vec<f64>,
    pub right_best_jaccard: Vec<f64>,
}
pub fn agreement(left: &[usize], right: &[usize]) -> Result<Agreement> {
    ensure!(
        left.len() == right.len() && !left.is_empty(),
        "Agreement requires identical nonempty token coverage"
    );
    let left = canonicalize(left);
    let right = canonicalize(right);
    let n = left.len() as f64;
    let mut a = BTreeMap::<usize, usize>::new();
    let mut b = BTreeMap::<usize, usize>::new();
    let mut joint = BTreeMap::<(usize, usize), usize>::new();
    for (&x, &y) in left.iter().zip(&right) {
        *a.entry(x).or_default() += 1;
        *b.entry(y).or_default() += 1;
        *joint.entry((x, y)).or_default() += 1;
    }
    let pairs = |x: usize| x as f64 * x.saturating_sub(1) as f64 / 2.0;
    let x: f64 = a.values().map(|&s| pairs(s)).sum();
    let y: f64 = b.values().map(|&s| pairs(s)).sum();
    let z: f64 = joint.values().map(|&s| pairs(s)).sum();
    let expected = if left.len() > 1 {
        x * y / pairs(left.len())
    } else {
        0.0
    };
    let denominator = (x + y) / 2.0 - expected;
    let entropy = |counts: &BTreeMap<usize, usize>| {
        -counts
            .values()
            .map(|&s| {
                let p = s as f64 / n;
                p * p.ln()
            })
            .sum::<f64>()
    };
    let mutual: f64 = joint
        .iter()
        .map(|(&(x, y), &s)| {
            let p = s as f64 / n;
            p * (s as f64 * n / (a[&x] * b[&y]) as f64).ln()
        })
        .sum();
    let mut best_a = vec![0.0_f64; a.len()];
    let mut best_b = vec![0.0_f64; b.len()];
    for (&(x, y), &s) in &joint {
        let j = s as f64 / (a[&x] + b[&y] - s) as f64;
        best_a[x] = best_a[x].max(j);
        best_b[y] = best_b[y].max(j);
    }
    Ok(Agreement {
        ari: if denominator == 0.0 {
            1.0
        } else {
            (z - expected) / denominator
        },
        variation_of_information: (entropy(&a) + entropy(&b) - 2.0 * mutual).max(0.0),
        left_best_jaccard: best_a,
        right_best_jaccard: best_b,
    })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RepeatComparison {
    pub left_seed: u64,
    pub right_seed: u64,
    pub agreement: Agreement,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RepeatMetrics {
    pub pairs: Vec<RepeatComparison>,
    pub ari: Distribution,
    pub variation_of_information: Distribution,
    pub best_match_jaccard: Distribution,
    pub representative_seed: Option<u64>,
    pub representative_index: Option<usize>,
    pub note: String,
}
pub fn repeats(runs: &[(u64, Vec<usize>)]) -> Result<RepeatMetrics> {
    let mut pairs = vec![];
    let mut sums = vec![0.0; runs.len()];
    for i in 0..runs.len() {
        for j in i + 1..runs.len() {
            let agreement = agreement(&runs[i].1, &runs[j].1)?;
            sums[i] += agreement.ari;
            sums[j] += agreement.ari;
            pairs.push(RepeatComparison {
                left_seed: runs[i].0,
                right_seed: runs[j].0,
                agreement,
            });
        }
    }
    let representative_index = (0..runs.len())
        .max_by(|&a, &b| sums[a].total_cmp(&sums[b]).then(runs[b].0.cmp(&runs[a].0)));
    Ok(RepeatMetrics { ari:distribution(pairs.iter().map(|p| p.agreement.ari)),
        variation_of_information:distribution(pairs.iter().map(|p| p.agreement.variation_of_information)),
        best_match_jaccard:distribution(pairs.iter().flat_map(|p| p.agreement.left_best_jaccard.iter().chain(&p.agreement.right_best_jaccard)).copied()),
        representative_seed:representative_index.map(|i| runs[i].0), representative_index,pairs,
        note:"VI uses natural logarithms. Pairwise comparisons are dependent diagnostics, not independent replicates. Jaccards are directional per-community matches.".into() })
}

pub fn edge_overlap(left: &Graph, right: &Graph) -> Metric {
    let a: BTreeSet<_> = left
        .edges
        .iter()
        .map(|e| (&left.tokens[e.u], &left.tokens[e.v]))
        .collect();
    let b: BTreeSet<_> = right
        .edges
        .iter()
        .map(|e| (&right.tokens[e.u], &right.tokens[e.v]))
        .collect();
    let union = a.union(&b).count();
    if union == 0 {
        Metric::missing("Both edge sets are empty")
    } else {
        Metric::value(a.intersection(&b).count() as f64 / union as f64)
    }
}

/// Word-weighted leave-one-out cohesion, O(nd), also used by label permutations.
pub fn cohesion(input: &Embeddings, labels: &[usize]) -> Metric {
    let groups = groups(labels);
    let d = input.vectors[0].len();
    let mut total = 0.0;
    let mut words = 0;
    for group in groups.iter().filter(|g| g.len() > 1) {
        let mut sum = vec![0.0; d];
        let mut self_dots = 0.0;
        for &i in group {
            for (s, x) in sum.iter_mut().zip(&input.vectors[i]) {
                *s += x;
            }
            self_dots += dot(&input.vectors[i], &input.vectors[i]);
        }
        total += (dot(&sum, &sum) - self_dots) / (group.len() - 1) as f64;
        words += group.len();
    }
    if words == 0 {
        Metric::missing("No nonsingleton tokens")
    } else {
        Metric::value(total / words as f64)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NullControl {
    pub seeds: Vec<u64>,
    pub observed: Metric,
    pub null: Distribution,
    pub observed_minus_null_mean: Metric,
}
pub fn null_control(input: &Embeddings, labels: &[usize], seeds: &[u64]) -> NullControl {
    let observed = cohesion(input, labels);
    let values: Vec<_> = seeds
        .iter()
        .filter_map(|&seed| {
            let mut labels = labels.to_vec();
            labels.shuffle(&mut Xoshiro256PlusPlus::seed_from_u64(seed));
            cohesion(input, &labels).value
        })
        .collect();
    let null = distribution(values);
    let difference = match (observed.value, null.mean) {
        (Some(a), Some(b)) => Metric::value(a - b),
        _ => Metric::missing("Observed or null cohesion undefined"),
    };
    NullControl {
        seeds: seeds.to_vec(),
        observed,
        null,
        observed_minus_null_mean: difference,
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TokenMetadata {
    pub bands: BTreeSet<String>,
    pub parts_of_speech: BTreeSet<String>,
    pub mapped_rows: usize,
}
pub fn load_metadata(path: &Path) -> Result<BTreeMap<String, TokenMetadata>> {
    let mut reader = csv::Reader::from_path(path)?;
    let header = reader.headers()?.clone();
    let token = header
        .iter()
        .position(|h| h == "Simplified" || h == "token")
        .ok_or_else(|| anyhow::anyhow!("Metadata requires Simplified or token"))?;
    let level = header.iter().position(|h| h == "Level" || h == "hsk_level");
    let pos = header
        .iter()
        .position(|h| h == "pos" || h == "POS" || h == "part_of_speech");
    let mut map = BTreeMap::<String, TokenMetadata>::new();
    for row in reader.records() {
        let row = row?;
        for spelling in row[token]
            .split('|')
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            let item = map.entry(spelling.into()).or_default();
            item.mapped_rows += 1;
            if let Some(level) = level {
                if !row[level].is_empty() {
                    item.bands.insert(row[level].into());
                }
            }
            if let Some(pos) = pos {
                for part in row[pos].split('|').map(str::trim).filter(|s| !s.is_empty()) {
                    item.parts_of_speech.insert(part.into());
                }
            }
        }
    }
    Ok(map)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Stratum {
    pub dimension: String,
    pub group: String,
    pub tokens: usize,
    pub vocabulary_fraction: f64,
    pub nonsingleton_coverage: f64,
    pub cohesion: Metric,
    pub silhouette: Metric,
    pub margin: Metric,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Stratification {
    pub strata: Vec<Stratum>,
    pub missing_hsk: usize,
    pub missing_pos: usize,
    pub ambiguous_mappings: usize,
    pub degree_quantile_boundaries: Vec<f64>,
    pub note: String,
}
pub fn stratify(
    tokens: &[String],
    words: &[WordMetrics],
    metadata: &BTreeMap<String, TokenMetadata>,
    baseline_degrees: Option<&[usize]>,
) -> Stratification {
    let mut strata = BTreeMap::<(String, String), Vec<usize>>::new();
    let mut missing_hsk = 0;
    let mut missing_pos = 0;
    let mut ambiguous = 0;
    let boundaries = baseline_degrees
        .map(|d| {
            let mut x = d.to_vec();
            x.sort_unstable();
            [1, 2, 3, 4]
                .iter()
                .map(|i| x[(x.len() - 1) * i / 5] as f64)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for (i, token) in tokens.iter().enumerate() {
        let item = metadata.get(token);
        let bands = item.map(|m| &m.bands);
        let pos = item.map(|m| &m.parts_of_speech);
        if bands.is_none_or(BTreeSet::is_empty) {
            missing_hsk += 1;
            strata
                .entry(("hsk".into(), "missing".into()))
                .or_default()
                .push(i);
        } else {
            for band in bands.unwrap() {
                strata
                    .entry(("hsk".into(), band.clone()))
                    .or_default()
                    .push(i);
            }
        }
        if pos.is_none_or(BTreeSet::is_empty) {
            missing_pos += 1;
            strata
                .entry(("pos".into(), "missing".into()))
                .or_default()
                .push(i);
        } else {
            for p in pos.unwrap() {
                strata.entry(("pos".into(), p.clone())).or_default().push(i);
            }
        }
        if item
            .is_some_and(|m| m.mapped_rows > 1 || m.bands.len() > 1 || m.parts_of_speech.len() > 1)
        {
            ambiguous += 1;
        }
        let band = baseline_degrees
            .map(|d| {
                format!(
                    "q{}",
                    1 + boundaries.iter().filter(|&&b| d[i] as f64 > b).count()
                )
            })
            .unwrap_or_else(|| "unavailable".into());
        strata
            .entry(("baseline_degree".into(), band))
            .or_default()
            .push(i);
    }
    let communities = words
        .iter()
        .map(|w| w.community)
        .collect::<BTreeSet<_>>()
        .len();
    let silhouette_defined = communities > 1 && communities < words.len();
    Stratification {strata:strata.into_iter().map(|((dimension,group),rows)|Stratum {dimension,group,tokens:rows.len(),vocabulary_fraction:rows.len() as f64/tokens.len() as f64,
        nonsingleton_coverage:rows.iter().filter(|&&i|words[i].size>1).count() as f64/rows.len() as f64,
        cohesion:Metric::mean(rows.iter().filter_map(|&i|words[i].cohesion),"No nonsingletons"),
        silhouette:if silhouette_defined{Metric::mean(rows.iter().filter_map(|&i|words[i].silhouette),"Undefined silhouette")}else{Metric::missing("Silhouette undefined for one community or all singletons")},margin:Metric::mean(rows.iter().filter_map(|&i|words[i].margin),"Undefined margin")}).collect(),
        missing_hsk,missing_pos,ambiguous_mappings:ambiguous,degree_quantile_boundaries:boundaries,
        note:"Summaries use global community membership. Multiple HSK/POS mappings appear in each applicable stratum. Tied degrees stay in the lower quintile; bins may be empty. HSK is metadata, never a label.".into()}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metrics_match_direct_distances_and_null_preserves_sizes() -> Result<()> {
        let input =
            Embeddings::read("5 2\na 1 0\nb 0.8 0.6\nc -1 0\nd -0.8 -0.6\ne 0 1\n".as_bytes())?;
        let graph = Graph {
            tokens: input.tokens.clone(),
            edges: vec![],
        };
        let labels = [0, 0, 1, 1, 2];
        let (m, words) = partition_metrics(&input, &graph, &labels)?;
        let mut expected = 0.0;
        for i in 0..4 {
            let a = 1.0 - dot(&input.vectors[i], &input.vectors[i ^ 1]);
            let mut b = f64::INFINITY;
            for g in 0..3 {
                if g == labels[i] {
                    continue;
                }
                let members: Vec<_> = (0..5).filter(|&j| labels[j] == g).collect();
                b = b.min(
                    members
                        .iter()
                        .map(|&j| 1.0 - dot(&input.vectors[i], &input.vectors[j]))
                        .sum::<f64>()
                        / members.len() as f64,
                );
            }
            expected += (b - a) / a.max(b);
        }
        assert!((m.silhouette.value.unwrap() - expected / 5.0).abs() < 1e-12);
        assert_eq!(words[4].silhouette, Some(0.0));
        assert!((m.cohesion.value.unwrap() - 0.8).abs() < 1e-12);
        assert!((cohesion(&input, &labels).value.unwrap() - 0.8).abs() < 1e-12);
        assert!(partition_metrics(&input, &graph, &[0; 5])?
            .0
            .silhouette
            .value
            .is_none());
        assert_eq!(
            null_control(&input, &labels, &(0..100).collect::<Vec<_>>())
                .null
                .count,
            100
        );
        Ok(())
    }
    #[test]
    fn agreement_and_representative_ties() -> Result<()> {
        let result = agreement(&[0, 0, 1, 1], &[0, 1, 0, 1])?;
        assert!((result.ari + 0.5).abs() < 1e-12);
        assert!((result.variation_of_information - 2.0_f64.ln() * 2.0).abs() < 1e-12);
        assert_eq!(result.left_best_jaccard, vec![1.0 / 3.0; 2]);
        let result = repeats(&[(8, vec![0, 0, 1]), (3, vec![1, 1, 0])])?;
        assert_eq!(result.representative_seed, Some(3));
        assert_eq!(result.ari.mean, Some(1.0));
        Ok(())
    }
}
