//! Export GMM-only diagnostics and inspection tables from completed native runs.
use anyhow::{ensure, Context, Result};
use clap::Parser;
use hskclustering::{
    clustering::{partition_metrics, read_assignments},
    embeddings::prepare_embeddings,
    output::{sha256, write_atomic, write_json},
};
use ndarray::Array2;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{collections::HashMap, fs, path::PathBuf};

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "data/input-embeddings.txt")]
    input: PathBuf,
    #[arg(required = true, num_args = 1..)]
    runs: Vec<PathBuf>,
}

#[derive(Serialize)]
struct Word<'a> {
    token: &'a str,
    community: usize,
    size: usize,
    mean_cosine_to_own_others: Option<f64>,
    nearest_other_community: Option<usize>,
    cosine_margin: Option<f64>,
    cosine_silhouette: Option<f64>,
    max_membership: f64,
    second_community: Option<usize>,
    second_membership: f64,
    membership_margin: f64,
    normalized_membership_entropy: f64,
}

#[derive(Serialize)]
struct Cluster {
    community: usize,
    size: usize,
    mean_pair_cosine: Option<f64>,
    mean_cosine_silhouette: Option<f64>,
    negative_silhouette_fraction: Option<f64>,
    representatives: String,
    boundary_words: String,
    deterministic_sample: String,
    all_members: String,
    soft_mass: f64,
    mean_max_membership: Option<f64>,
    mean_normalized_membership_entropy: Option<f64>,
    highest_membership_words: String,
    strongest_members_outside_hard_cluster: String,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let (tokens, vectors) = prepare_embeddings(&args.input, None)?;
    let n = tokens.len();
    let d = vectors[0].len();
    let input_hash = sha256(&args.input)?;
    let index: HashMap<_, _> = tokens
        .iter()
        .enumerate()
        .map(|(i, t)| (t.as_str(), i))
        .collect();
    let hashes: Vec<_> = tokens
        .iter()
        .map(|t| Sha256::digest(t.as_bytes()))
        .collect();
    let data = Array2::from_shape_vec(
        (n, d),
        vectors.iter().flatten().map(|&v| f64::from(v)).collect(),
    )?;
    for directory in args.runs {
        let run: serde_json::Value =
            serde_json::from_reader(fs::File::open(directory.join("run.json"))?)?;
        ensure!(
            run["method"] == "gmm" && run["input_sha256"] == input_hash,
            "Expected a GMM run on this exact input"
        );
        let k = run["selected"]["config"]["clusters"]
            .as_u64()
            .context("No successful fit")? as usize;
        let labels = read_assignments(&directory.join("communities.csv"), &tokens)?;
        ensure!(labels.iter().all(|&v| v < k), "Invalid component ID");
        let mut groups = vec![Vec::new(); k];
        for (i, &g) in labels.iter().enumerate() {
            groups[g].push(i);
        }
        let occupied = groups.iter().filter(|g| !g.is_empty()).count();
        let mut probabilities = Array2::<f64>::zeros((n, k));
        let mut seen = vec![false; n];
        let mut max_probability_sum_error = 0.0_f64;
        let mut reader = csv::Reader::from_path(directory.join("memberships.csv"))?;
        let expected: Vec<_> = std::iter::once("token".to_owned())
            .chain((0..k).map(|j| format!("probability_{j}")))
            .collect();
        ensure!(
            reader
                .headers()?
                .iter()
                .eq(expected.iter().map(String::as_str)),
            "Invalid probability header"
        );
        for row in reader.records() {
            let row = row?;
            let i = *index.get(&row[0]).context("Unknown probability token")?;
            ensure!(!seen[i], "Duplicate probability token");
            seen[i] = true;
            let mut sum = 0.0;
            let mut best = 0;
            for (g, value) in row.iter().skip(1).enumerate() {
                let p: f64 = value.parse()?;
                ensure!(
                    p.is_finite() && (0.0..=1.0).contains(&p),
                    "Invalid probability"
                );
                probabilities[[i, g]] = p;
                sum += p;
                if p > probabilities[[i, best]] {
                    best = g;
                }
            }
            max_probability_sum_error = max_probability_sum_error.max((sum - 1.0).abs());
            ensure!(
                (sum - 1.0).abs() < 1e-8 && best == labels[i],
                "Probability sum/argmax mismatch"
            );
        }
        ensure!(seen.iter().all(|&v| v), "Missing probability rows");
        let soft_mass = probabilities.sum_axis(ndarray::Axis(0));
        let mut means = Array2::<f64>::zeros((k, d));
        for (g, members) in groups.iter().enumerate() {
            for &i in members {
                means.row_mut(g).scaled_add(1.0, &data.row(i));
            }
            if !members.is_empty() {
                means.row_mut(g).mapv_inplace(|v| v / members.len() as f64);
            }
        }
        let similarities = data.dot(&means.t());
        let mut words = Vec::with_capacity(n);
        for i in 0..n {
            let g = labels[i];
            let size = groups[g].len();
            let own = (size > 1).then(|| {
                ((similarities[[i, g]] * size as f64 - data.row(i).dot(&data.row(i)))
                    / (size - 1) as f64)
                    .clamp(-1.0, 1.0)
            });
            let other = (0..k)
                .filter(|&h| h != g && !groups[h].is_empty())
                .max_by(|&a, &b| {
                    similarities[[i, a]]
                        .total_cmp(&similarities[[i, b]])
                        .then_with(|| b.cmp(&a))
                });
            let margin = own
                .zip(other)
                .map(|(v, h)| v - similarities[[i, h]].clamp(-1.0, 1.0));
            let silhouette = if occupied <= 1 || occupied == n {
                None
            } else if size == 1 {
                Some(0.0)
            } else {
                own.zip(other).map(|(v, h)| {
                    let a = 1.0 - v;
                    let b = 1.0 - similarities[[i, h]].clamp(-1.0, 1.0);
                    if a.max(b) == 0.0 {
                        0.0
                    } else {
                        (b - a) / a.max(b)
                    }
                })
            };
            let second = (0..k).filter(|&h| h != g).max_by(|&a, &b| {
                probabilities[[i, a]]
                    .total_cmp(&probabilities[[i, b]])
                    .then_with(|| b.cmp(&a))
            });
            let p1 = probabilities[[i, g]];
            let p2 = second.map_or(0.0, |h| probabilities[[i, h]]);
            let entropy = -probabilities
                .row(i)
                .iter()
                .filter(|&&v| v > 0.0)
                .map(|&v| v * v.ln())
                .sum::<f64>();
            words.push(Word {
                token: &tokens[i],
                community: g,
                size,
                mean_cosine_to_own_others: own,
                nearest_other_community: other,
                cosine_margin: margin,
                cosine_silhouette: silhouette,
                max_membership: p1,
                second_community: second,
                second_membership: p2,
                membership_margin: p1 - p2,
                normalized_membership_entropy: if k > 1 {
                    entropy / (k as f64).ln()
                } else {
                    0.0
                },
            });
        }
        let metrics = partition_metrics(&vectors, &labels)?;
        if let Some(expected) = metrics.mean_cosine_silhouette {
            let calculated = words
                .iter()
                .map(|w| w.cosine_silhouette.unwrap_or(0.0))
                .sum::<f64>()
                / n as f64;
            ensure!(
                (expected - calculated).abs() < 1e-10,
                "Silhouette disagrees with shared implementation"
            );
        }
        let join = |indices: &[usize], limit: usize| {
            indices
                .iter()
                .take(limit)
                .map(|&i| tokens[i].as_str())
                .collect::<Vec<_>>()
                .join(" | ")
        };
        let probability_join = |indices: &[usize], g: usize, limit: usize| {
            indices
                .iter()
                .take(limit)
                .map(|&i| format!("{}:{:.6}", tokens[i], probabilities[[i, g]]))
                .collect::<Vec<_>>()
                .join(" | ")
        };
        let mut clusters = Vec::with_capacity(k);
        for (g, members) in groups.iter().enumerate() {
            let size = members.len();
            let mut central = members.clone();
            central.sort_by(|&a, &b| {
                words[b]
                    .mean_cosine_to_own_others
                    .unwrap_or(-1.0)
                    .total_cmp(&words[a].mean_cosine_to_own_others.unwrap_or(-1.0))
                    .then_with(|| tokens[a].cmp(&tokens[b]))
            });
            let mut boundary = members.clone();
            boundary.sort_by(|&a, &b| {
                words[a]
                    .cosine_margin
                    .unwrap_or(0.0)
                    .total_cmp(&words[b].cosine_margin.unwrap_or(0.0))
                    .then_with(|| tokens[a].cmp(&tokens[b]))
            });
            let mut sample = members.clone();
            sample.sort_by_key(|&i| hashes[i]);
            let mut by_probability: Vec<_> = (0..n).collect();
            by_probability.sort_by(|&a, &b| {
                probabilities[[b, g]]
                    .total_cmp(&probabilities[[a, g]])
                    .then_with(|| tokens[a].cmp(&tokens[b]))
            });
            let outsiders: Vec<_> = by_probability
                .iter()
                .copied()
                .filter(|&i| labels[i] != g && probabilities[[i, g]] >= 0.01)
                .collect();
            let average = |f: fn(&Word) -> f64| {
                (size > 0).then(|| members.iter().map(|&i| f(&words[i])).sum::<f64>() / size as f64)
            };
            clusters.push(Cluster {
                community: g,
                size,
                mean_pair_cosine: (size > 1).then(|| {
                    members
                        .iter()
                        .map(|&i| words[i].mean_cosine_to_own_others.unwrap())
                        .sum::<f64>()
                        / size as f64
                }),
                mean_cosine_silhouette: (occupied > 1 && occupied < n && size > 0).then(|| {
                    members
                        .iter()
                        .map(|&i| words[i].cosine_silhouette.unwrap())
                        .sum::<f64>()
                        / size as f64
                }),
                negative_silhouette_fraction: average(|w| {
                    if w.cosine_silhouette.is_some_and(|v| v < 0.0) {
                        1.0
                    } else {
                        0.0
                    }
                }),
                representatives: join(&central, 12),
                boundary_words: join(&boundary, 8),
                deterministic_sample: join(&sample, 12),
                all_members: join(&central, size),
                soft_mass: soft_mass[g],
                mean_max_membership: average(|w| w.max_membership),
                mean_normalized_membership_entropy: average(|w| w.normalized_membership_entropy),
                highest_membership_words: probability_join(&by_probability, g, 12),
                strongest_members_outside_hard_cluster: probability_join(&outsiders, g, 8),
            });
        }
        write_atomic(&directory.join("clusters.csv"), |out| {
            let mut writer = csv::Writer::from_writer(out);
            for cluster in &clusters {
                writer.serialize(cluster)?;
            }
            writer.flush()?;
            Ok(())
        })?;
        let mut order: Vec<_> = (0..n).collect();
        order.sort_by(|&a, &b| tokens[a].cmp(&tokens[b]));
        write_atomic(&directory.join("words.csv"), |out| {
            let mut writer = csv::Writer::from_writer(out);
            for &i in &order {
                writer.serialize(&words[i])?;
            }
            writer.flush()?;
            Ok(())
        })?;
        let mut sizes: Vec<_> = groups.iter().map(Vec::len).collect();
        sizes.sort_unstable_by(|a, b| b.cmp(a));
        let hard_effective =
            (n as f64).powi(2) / sizes.iter().map(|&s| (s as f64).powi(2)).sum::<f64>();
        let soft_effective = soft_mass.sum().powi(2) / soft_mass.iter().map(|v| v * v).sum::<f64>();
        let pca: serde_json::Value =
            serde_json::from_reader(fs::File::open(directory.join("pca.json"))?)?;
        write_json(
            &directory.join("metrics.json"),
            &serde_json::json!({
                "tokens": n, "components": k, "occupied_components": occupied, "empty_hard_components": k-occupied,
                "median_hard_size": metrics.median_size, "smallest_hard_size": sizes.last(), "largest_hard_size": sizes[0],
                "largest_share": sizes[0] as f64 / n as f64, "top5_share": sizes.iter().take(5).sum::<usize>() as f64 / n as f64,
                "hard_components_size_1_to_5": sizes.iter().filter(|&&s| (1..=5).contains(&s)).count(),
                "words_in_components_size_1_to_5": sizes.iter().filter(|&&s| (1..=5).contains(&s)).sum::<usize>(),
                "soft_components_mass_below_5": soft_mass.iter().filter(|&&v| v < 5.0).count(),
                "effective_hard_components": hard_effective, "effective_soft_components": soft_effective,
                "mean_pair_cosine": metrics.mean_pair_cosine_word_weighted_nonsingletons,
                "mean_cosine_silhouette": metrics.mean_cosine_silhouette,
                "fraction_negative_silhouette": words.iter().filter(|w| w.cosine_silhouette.is_some_and(|v| v < 0.0)).count() as f64 / n as f64,
                "mean_max_membership": words.iter().map(|w| w.max_membership).sum::<f64>() / n as f64,
                "fraction_max_membership_below_half": words.iter().filter(|w| w.max_membership < 0.5).count() as f64 / n as f64,
                "fraction_membership_margin_below_point2": words.iter().filter(|w| w.membership_margin < 0.2).count() as f64 / n as f64,
                "mean_normalized_membership_entropy": words.iter().map(|w| w.normalized_membership_entropy).sum::<f64>() / n as f64,
                "retained_variance_ratio": pca["retained_variance_ratio"],
                "interpretation": "Concentration and geometry are descriptive diagnostics. No target granularity; component and semantic group need not correspond. Responsibilities do not measure semantic correctness."
            }),
        )?;
        write_json(
            &directory.join("review-validation.json"),
            &serde_json::json!({
                "input_sha256": input_hash, "assignments_sha256": sha256(&directory.join("communities.csv"))?,
                "memberships_sha256": sha256(&directory.join("memberships.csv"))?, "tokens_verified": n,
                "all_probabilities_finite_in_unit_interval": true, "hard_assignments_equal_argmax": true,
                "max_probability_sum_error": max_probability_sum_error, "cluster_sizes_sum_to_tokens": sizes.iter().sum::<usize>() == n,
                "soft_mass_sum": soft_mass.sum(), "silhouette_matches_shared_implementation": true
            }),
        )?;
        eprintln!(
            "{}: k={k}, largest={:.1}%, tiny={}, mean membership={:.3}",
            directory.display(),
            sizes[0] as f64 / n as f64 * 100.0,
            sizes.iter().filter(|&&s| (1..=5).contains(&s)).count(),
            words.iter().map(|w| w.max_membership).sum::<f64>() / n as f64
        );
    }
    Ok(())
}
