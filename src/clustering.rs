//! Shared assignment files and diagnostics. Each method keeps its own runner.

use anyhow::{ensure, Context, Result};
use ndarray::Array2;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use crate::output::write_atomic;

/// Keep the existing token,community format for every method. IDs are run-local.
pub fn write_assignments(path: &Path, tokens: &[String], labels: &[usize]) -> Result<usize> {
    ensure!(
        tokens.len() == labels.len(),
        "Assignment count must match token count"
    );
    let mut rows: Vec<_> = tokens.iter().zip(labels).collect();
    rows.sort_unstable_by(|a, b| a.0.cmp(b.0));
    write_atomic(path, |output| {
        let mut writer = csv::Writer::from_writer(output);
        writer.write_record(["token", "community"])?;
        for row in rows {
            writer.serialize(row)?;
        }
        writer.flush()?;
        Ok(tokens.len())
    })
}

pub fn write_partition(path: &Path, communities: &[Vec<String>]) -> Result<usize> {
    let (tokens, labels): (Vec<_>, Vec<_>) = communities
        .iter()
        .enumerate()
        .flat_map(|(id, members)| members.iter().map(move |token| (token.clone(), id)))
        .unzip();
    write_assignments(path, &tokens, &labels)
}

/// Align by token, requiring exactly the input vocabulary (never its intersection).
pub fn read_assignments(path: &Path, tokens: &[String]) -> Result<Vec<usize>> {
    let mut reader = csv::Reader::from_path(path)?;
    let headers = reader.headers()?;
    let token_column = headers
        .iter()
        .position(|s| s == "token")
        .context("Missing token column")?;
    let label_column = headers
        .iter()
        .position(|s| s == "community")
        .context("Missing community column")?;
    let mut rows = HashMap::new();
    for record in reader.records() {
        let record = record?;
        let token = record[token_column].to_owned();
        let label: usize = record[label_column]
            .parse()
            .context("Community IDs must be non-negative integers")?;
        ensure!(
            rows.insert(token.clone(), label).is_none(),
            "Duplicate assignment for {token:?}"
        );
    }
    let labels = tokens
        .iter()
        .map(|token| {
            rows.remove(token)
                .with_context(|| format!("Missing assignment for {token:?} in {}", path.display()))
        })
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        rows.is_empty(),
        "{} contains {} unknown tokens",
        path.display(),
        rows.len()
    );
    Ok(labels)
}

#[derive(Debug, Serialize)]
pub struct PartitionMetrics {
    pub tokens: usize,
    pub clusters: usize,
    pub median_size: f64,
    pub largest: usize,
    pub singletons: usize,
    pub words_in_3_to_30: usize,
    pub words_in_over_100: usize,
    pub mean_pair_cosine_word_weighted_nonsingletons: Option<f64>,
    pub mean_cosine_silhouette: Option<f64>,
}

/// Exact cosine silhouette using unit-vector cluster means: O(n*k*d) work,
/// with similarities evaluated in batches rather than an n-by-n distance matrix.
pub fn partition_metrics(vectors: &[Vec<f32>], labels: &[usize]) -> Result<PartitionMetrics> {
    let n = vectors.len();
    ensure!(
        n > 0 && labels.len() == n,
        "Expected one assignment per nonempty input row"
    );
    let d = vectors[0].len();
    ensure!(
        d > 0 && vectors.iter().all(|v| v.len() == d),
        "Invalid embedding dimensions"
    );
    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (i, &label) in labels.iter().enumerate() {
        groups.entry(label).or_default().push(i);
    }
    let groups: Vec<_> = groups.into_values().collect();
    let k = groups.len();
    let mut membership = vec![0; n];
    let mut means = vec![vec![0.0; d]; k];
    for (g, members) in groups.iter().enumerate() {
        for &i in members {
            membership[i] = g;
            let norm: f64 = vectors[i].iter().map(|&x| f64::from(x).powi(2)).sum();
            ensure!(
                norm.is_finite() && (norm - 1.0).abs() < 1e-5,
                "Metrics require unit vectors"
            );
            for (sum, &x) in means[g].iter_mut().zip(&vectors[i]) {
                *sum += f64::from(x);
            }
        }
        for mean in &mut means[g] {
            *mean /= members.len() as f64;
        }
    }
    let mut cohesion = 0.0;
    let mut silhouette = 0.0;
    let mut nonsingletons = 0;
    let means = Array2::from_shape_vec((k, d), means.into_iter().flatten().collect())?;
    for (batch, rows) in vectors.chunks(256).enumerate() {
        let data = Array2::from_shape_vec(
            (rows.len(), d),
            rows.iter().flatten().map(|&x| f64::from(x)).collect(),
        )?;
        let similarities = data.dot(&means.t());
        for (row, vector) in rows.iter().enumerate() {
            let i = batch * 256 + row;
            let g = membership[i];
            let size = groups[g].len();
            if size == 1 {
                continue;
            }
            let self_dot: f64 = vector.iter().map(|&x| f64::from(x).powi(2)).sum();
            let own = ((similarities[[row, g]] * size as f64 - self_dot) / (size - 1) as f64)
                .clamp(-1.0, 1.0);
            cohesion += own;
            nonsingletons += 1;
            if k > 1 {
                let other = similarities
                    .row(row)
                    .iter()
                    .enumerate()
                    .filter(|(h, _)| *h != g)
                    .map(|(_, &similarity)| similarity)
                    .fold(-1.0_f64, f64::max)
                    .clamp(-1.0, 1.0);
                let (a, b) = (1.0 - own, 1.0 - other);
                if a.max(b) > 0.0 {
                    silhouette += (b - a) / a.max(b);
                }
            }
        }
    }
    let mut sizes: Vec<_> = groups.iter().map(Vec::len).collect();
    sizes.sort_unstable();
    Ok(PartitionMetrics {
        tokens: n,
        clusters: k,
        median_size: (sizes[(k - 1) / 2] + sizes[k / 2]) as f64 / 2.0,
        largest: sizes[k - 1],
        singletons: sizes.iter().filter(|&&s| s == 1).count(),
        words_in_3_to_30: sizes.iter().filter(|&&s| (3..=30).contains(&s)).sum(),
        words_in_over_100: sizes.iter().filter(|&&s| s > 100).sum(),
        mean_pair_cosine_word_weighted_nonsingletons: (nonsingletons > 0)
            .then(|| cohesion / nonsingletons as f64),
        mean_cosine_silhouette: (k > 1 && k < n).then(|| silhouette / n as f64),
    })
}

/// Agreement independent of numeric label identities; not a semantic quality score.
pub fn adjusted_rand_index(left: &[usize], right: &[usize]) -> Result<f64> {
    ensure!(
        left.len() == right.len(),
        "Partitions must cover the same tokens"
    );
    if left.len() < 2 {
        return Ok(1.0);
    }
    let mut a = HashMap::<usize, usize>::new();
    let mut b = HashMap::<usize, usize>::new();
    let mut joint = HashMap::<(usize, usize), usize>::new();
    for (&x, &y) in left.iter().zip(right) {
        *a.entry(x).or_default() += 1;
        *b.entry(y).or_default() += 1;
        *joint.entry((x, y)).or_default() += 1;
    }
    let pairs = |n: usize| n as f64 * n.saturating_sub(1) as f64 / 2.0;
    let x: f64 = a.values().map(|&n| pairs(n)).sum();
    let y: f64 = b.values().map(|&n| pairs(n)).sum();
    let z: f64 = joint.values().map(|&n| pairs(n)).sum();
    let expected = x * y / pairs(left.len());
    let denominator = (x + y) / 2.0 - expected;
    Ok(if denominator == 0.0 {
        1.0
    } else {
        (z - expected) / denominator
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_round_trip_aligns_tokens_and_rejects_partial_partitions() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("assignments.csv");
        let tokens = vec!["中,文".into(), "带\"引号".into(), "a".into()];
        write_assignments(&path, &tokens, &[2, 0, 1])?;
        assert_eq!(read_assignments(&path, &tokens)?, [2, 0, 1]);
        assert!(read_assignments(&path, &tokens[..2]).is_err());
        assert!(read_assignments(&path, &["missing".into()]).is_err());
        std::fs::write(&path, "token,community\na,0\na,1\n")?;
        assert!(read_assignments(&path, &["a".into()]).is_err());
        Ok(())
    }

    #[test]
    fn agreement_ignores_labels_and_handles_degenerate_partitions() -> Result<()> {
        assert_eq!(adjusted_rand_index(&[0, 0, 1, 1], &[8, 8, 3, 3])?, 1.0);
        assert!((adjusted_rand_index(&[0, 0, 1, 1], &[0, 1, 0, 1])? + 0.5).abs() < 1e-12);
        assert_eq!(adjusted_rand_index(&[0, 0], &[3, 3])?, 1.0);
        assert_eq!(adjusted_rand_index(&[0, 1], &[4, 5])?, 1.0);
        Ok(())
    }

    #[test]
    fn silhouette_matches_direct_pairwise_definition() -> Result<()> {
        let vectors = vec![
            vec![1.0, 0.0],
            vec![0.8, 0.6],
            vec![-1.0, 0.0],
            vec![-0.8, -0.6],
            vec![0.0, 1.0],
        ];
        let labels = [0, 0, 1, 1, 2];
        let mut total = 0.0;
        for (i, v) in vectors.iter().enumerate().take(4) {
            let distance = |j: usize| {
                1.0 - v
                    .iter()
                    .zip(&vectors[j])
                    .map(|(&x, &y)| f64::from(x) * f64::from(y))
                    .sum::<f64>()
            };
            let a = distance(i ^ 1);
            let b = (0..3)
                .filter(|&g| g != labels[i])
                .map(|g| {
                    let members: Vec<_> = (0..5).filter(|&j| labels[j] == g).collect();
                    members.iter().map(|&j| distance(j)).sum::<f64>() / members.len() as f64
                })
                .fold(f64::INFINITY, f64::min);
            total += (b - a) / a.max(b);
        }
        let metrics = partition_metrics(&vectors, &labels)?;
        assert!((metrics.mean_cosine_silhouette.unwrap() - total / 5.0).abs() < 1e-7);
        assert_eq!(metrics.singletons, 1);
        assert!(partition_metrics(&vectors, &[0; 5])?
            .mean_cosine_silhouette
            .is_none());
        assert!(partition_metrics(&vectors, &[0, 1, 2, 3, 4])?
            .mean_cosine_silhouette
            .is_none());
        Ok(())
    }
}
