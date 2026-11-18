//! A focused, size-profile study for a changed vocabulary; no legacy baseline gate.
use anyhow::{ensure, Context, Result};
use clap::{Parser, Subcommand};
use hskclustering::{
    clustering::{read_assignments, write_assignments},
    graph_v2::{
        embeddings::{Embeddings, Representation},
        experiment::{code_hashes, fresh_directory, load_graph},
        graph::{self, Graph, GraphConfig, Symmetrization, WeightMode},
        leiden::{self, LeidenConfig},
        metrics::{self, PartitionMetrics},
    },
    output::{sha256, write_json},
};
use rand::{seq::SliceRandom, SeedableRng};
use rand_xoshiro::Xoshiro256PlusPlus;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
    time::Instant,
};

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Freeze and run the explicit graph/resolution grid, with three common seeds.
    Screen {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        reference: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Run only the frozen shortlist with ten fresh seeds and paired stress tests.
    Confirm {
        #[arg(long)]
        screening: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "config", rename_all = "snake_case")]
enum Source {
    Threshold(f64),
    Neighbors(GraphConfig),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Spec {
    name: String,
    source: Source,
    q: Vec<f64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Profile {
    tokens: usize,
    communities: usize,
    mean: f64,
    median: f64,
    q25: f64,
    q75: f64,
    q95: f64,
    max: usize,
    group_fractions: Vec<f64>,
    token_fractions: Vec<f64>,
}
fn profile(labels: &[usize]) -> Profile {
    let groups = leiden::groups(labels);
    let sizes: Vec<_> = groups.iter().map(Vec::len).collect();
    let d = metrics::distribution(sizes.iter().map(|&n| n as f64));
    let mut gc = vec![0.; 9];
    let mut tc = vec![0.; 9];
    for n in sizes {
        let b = match n {
            1 => 0,
            2 => 1,
            3..=5 => 2,
            6..=10 => 3,
            11..=20 => 4,
            21..=30 => 5,
            31..=50 => 6,
            51..=100 => 7,
            _ => 8,
        };
        gc[b] += 1.;
        tc[b] += n as f64;
    }
    for v in &mut gc {
        *v /= groups.len() as f64;
    }
    for v in &mut tc {
        *v /= labels.len() as f64;
    }
    Profile {
        tokens: labels.len(),
        communities: groups.len(),
        mean: d.mean.unwrap(),
        median: d.median.unwrap(),
        q25: d.q25.unwrap(),
        q75: d.q75.unwrap(),
        q95: d.q95.unwrap(),
        max: d.max.unwrap() as usize,
        group_fractions: gc,
        token_fractions: tc,
    }
}
/// A descriptive preference, not a significance test or a semantic quality score.
fn distance(a: &Profile, b: &Profile) -> f64 {
    let tv = |x: &[f64], y: &[f64]| x.iter().zip(y).map(|(x, y)| (x - y).abs()).sum::<f64>() / 2.;
    0.5 * tv(&a.token_fractions, &b.token_fractions)
        + 0.3 * tv(&a.group_fractions, &b.group_fractions)
        + 0.2 * (a.mean / b.mean).ln().abs()
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Protocol {
    input: PathBuf,
    input_sha256: String,
    reference: PathBuf,
    reference_sha256: String,
    reference_profile: Profile,
    specs: Vec<Spec>,
    screening_seeds: Vec<u64>,
    confirmation_seeds: Vec<u64>,
    stress_seeds: Vec<u64>,
    code_hashes: BTreeMap<String, String>,
    selection_rule: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Run {
    seed: u64,
    metrics: PartitionMetrics,
    solver_seconds: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Stress {
    kind: String,
    seed: u64,
    ari: f64,
    retained_tokens: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Candidate {
    name: String,
    spec: Spec,
    q: f64,
    resolution: f64,
    theta: f64,
    graph_key: String,
    runs: Vec<Run>,
    representative_seed: u64,
    representative_metrics: PartitionMetrics,
    repeat_ari: f64,
    profile_distance: f64,
    directory: PathBuf,
    stress: Vec<Stress>,
}
#[derive(Serialize, Deserialize)]
struct Shortlist {
    protocol_sha256: String,
    summary_sha256: String,
    candidates: Vec<String>,
    rule: String,
}
fn read_json<T: serde::de::DeserializeOwned>(p: &Path) -> Result<T> {
    Ok(serde_json::from_reader(BufReader::new(File::open(p)?))?)
}
fn source_hashes() -> BTreeMap<String, String> {
    let mut h = code_hashes();
    h.insert(
        "bin/leiden_study.rs".into(),
        format!("{:x}", Sha256::digest(include_bytes!("leiden_study.rs"))),
    );
    h
}
fn specs() -> Vec<Spec> {
    let mut out: Vec<_> = [0.35, 0.4, 0.45]
        .into_iter()
        .map(|tau| Spec {
            name: format!("threshold-{tau:.2}"),
            source: Source::Threshold(tau),
            q: vec![0.15, 0.225, 0.3, 0.4, 0.55],
        })
        .collect();
    let variants = [
        (
            "raw-k10",
            GraphConfig {
                k: 10,
                ..Default::default()
            },
        ),
        (
            "raw-k20",
            GraphConfig {
                k: 20,
                ..Default::default()
            },
        ),
        (
            "raw-k40",
            GraphConfig {
                k: 40,
                ..Default::default()
            },
        ),
        (
            "center-k10",
            GraphConfig {
                k: 10,
                representation: Representation::Center,
                ..Default::default()
            },
        ),
        (
            "abtt1-k10",
            GraphConfig {
                k: 10,
                representation: Representation::Abtt(1),
                ..Default::default()
            },
        ),
        (
            "abtt3-k10",
            GraphConfig {
                k: 10,
                representation: Representation::Abtt(3),
                ..Default::default()
            },
        ),
        (
            "mutual-k10",
            GraphConfig {
                k: 10,
                symmetrization: Symmetrization::Mutual,
                ..Default::default()
            },
        ),
        (
            "snn-k10",
            GraphConfig {
                k: 10,
                weight: WeightMode::Snn,
                ..Default::default()
            },
        ),
        (
            "local-k10",
            GraphConfig {
                k: 10,
                weight: WeightMode::Local,
                ..Default::default()
            },
        ),
        (
            "blend-k10",
            GraphConfig {
                k: 10,
                weight: WeightMode::CosineSnn,
                ..Default::default()
            },
        ),
    ];
    out.extend(variants.into_iter().map(|(name, c)| Spec {
        name: name.into(),
        source: Source::Neighbors(c),
        q: vec![0.03, 0.06, 0.1, 0.2, 0.3],
    }));
    out
}
fn build(input: &Embeddings, source: &Source) -> Result<Graph> {
    match source {
        Source::Threshold(tau) => graph::build_threshold(input, *tau),
        Source::Neighbors(config) => Ok(graph::build(input, config)?.graph),
    }
}
fn graph_key(source: &Source) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(source)?)))
}
fn fit(
    input: &Embeddings,
    graph: &Graph,
    config: &LeidenConfig,
    directory: &Path,
) -> Result<(Run, Vec<usize>)> {
    std::fs::create_dir_all(directory)?;
    let t = Instant::now();
    let result = leiden::run(graph, config)?;
    let solver_seconds = t.elapsed().as_secs_f64();
    let (m, _) = metrics::partition_metrics(input, graph, &result.labels)?;
    ensure!(
        result.converged && m.all_communities_connected,
        "Invalid partition"
    );
    write_assignments(
        &directory.join("communities.csv"),
        &input.tokens,
        &result.labels,
    )?;
    write_json(&directory.join("solver.json"), &result)?;
    write_json(&directory.join("metrics.json"), &m)?;
    Ok((
        Run {
            seed: config.seed,
            metrics: m,
            solver_seconds,
        },
        result.labels,
    ))
}
fn evaluate(
    input: &Embeddings,
    graph: &Graph,
    spec: &Spec,
    q: f64,
    seeds: &[u64],
    reference: &Profile,
    out: &Path,
) -> Result<Candidate> {
    let name = format!("{}-q{q}", spec.name);
    let directory = out.join("candidates").join(&name);
    eprintln!(
        "{}: {} nodes, {} edges; {} seeds",
        name,
        graph.tokens.len(),
        graph.edges.len(),
        seeds.len()
    );
    let w50 = graph.median_weight().context("Study graph has no edges")?;
    let resolution = q * w50;
    let theta = 0.3 * w50;
    let mut runs = vec![];
    let mut partitions = vec![];
    for &seed in seeds {
        let (r, l) = fit(
            input,
            graph,
            &LeidenConfig {
                seed,
                resolution,
                theta,
                ..Default::default()
            },
            &directory.join(format!("seed-{seed}")),
        )?;
        runs.push(r);
        partitions.push((seed, l));
    }
    let repeats = metrics::repeats(&partitions)?;
    let i = repeats.representative_index.unwrap();
    write_json(&directory.join("repeats.json"), &repeats)?;
    write_json(
        &directory.join("null-control.json"),
        &metrics::null_control(input, &partitions[i].1, &(30000..30100).collect::<Vec<_>>()),
    )?;
    write_assignments(
        &directory.join("communities.csv"),
        &input.tokens,
        &partitions[i].1,
    )?;
    let c = Candidate {
        name,
        spec: spec.clone(),
        q,
        resolution,
        theta,
        graph_key: graph_key(&spec.source)?,
        representative_seed: seeds[i],
        representative_metrics: runs[i].metrics.clone(),
        repeat_ari: repeats.ari.median.context("Missing repeat ARI")?,
        profile_distance: distance(&profile(&partitions[i].1), reference),
        runs,
        directory: directory.clone(),
        stress: vec![],
    };
    write_json(&directory.join("candidate.json"), &c)?;
    Ok(c)
}
fn eligible(c: &Candidate) -> bool {
    c.runs.iter().all(|r| {
        r.metrics.nonsingleton_coverage >= 0.95
            && r.metrics.largest_community_share <= 0.04
            && r.metrics.communities > 1
            && r.metrics.all_communities_connected
    })
}
fn shortlist(candidates: &[Candidate]) -> Result<Vec<String>> {
    let mut sorted: Vec<_> = candidates.iter().filter(|c| eligible(c)).collect();
    sorted.sort_by(|a, b| {
        a.profile_distance
            .total_cmp(&b.profile_distance)
            .then(a.name.cmp(&b.name))
    });
    ensure!(
        !sorted.is_empty(),
        "No candidate met coverage/concentration guardrails"
    );
    let mut names = vec![];
    // Exact transferred setting plus the closest profile in three graph families.
    let anchor = candidates
        .iter()
        .find(|c| c.name == "threshold-0.40-q0.3")
        .context("Missing reference control")?;
    names.push(anchor.name.clone());
    for family in 0..3 {
        if let Some(c) = sorted.iter().find(|c| match (&c.spec.source, family) {
            (Source::Threshold(_), 0) => true,
            (Source::Neighbors(g), 1) => g.representation == Representation::Raw,
            (Source::Neighbors(g), 2) => g.representation != Representation::Raw,
            _ => false,
        }) {
            if !names.contains(&c.name) {
                names.push(c.name.clone());
            }
        }
    }
    Ok(names)
}
fn screen(input_path: &Path, reference_path: &Path, out: &Path) -> Result<()> {
    fresh_directory(out)?;
    let input = Embeddings::load(input_path)?;
    let mut reader = csv::Reader::from_path(reference_path)?;
    let mut labels = vec![];
    let headers = reader.headers()?;
    let col = headers
        .iter()
        .position(|x| x == "community")
        .context("Missing reference community column")?;
    for row in reader.records() {
        labels.push(row?[col].parse::<usize>()?);
    }
    ensure!(!labels.is_empty(), "Empty reference");
    let protocol=Protocol{input:input_path.into(),input_sha256:sha256(input_path)?,reference:reference_path.into(),reference_sha256:sha256(reference_path)?,reference_profile:profile(&labels),specs:specs(),screening_seeds:vec![42,43,44],confirmation_seeds:(1000..1010).collect(),stress_seeds:(20000..20010).collect(),code_hashes:source_hashes(),selection_rule:"Profile distance = 0.5 token-size-band TV + 0.3 group-size-band TV + 0.2 abs(ln(mean-size/reference-mean)); bands 1,2,3-5,6-10,11-20,21-30,31-50,51-100,>100. Guardrails every seed >=95% nonsingleton coverage, <=4% largest group, connected nondegenerate partition. Confirm transferred threshold .40 q .3 plus best eligible threshold, raw-neighbor, and transformed-neighbor candidates; choose by development profile distance only if fresh-seed repeat ARI >=.65 and both paired stress median ARIs >=.65; otherwise report no qualified recommendation. Score is descriptive, no significance/semantic claim. No tuning after confirmation; no endpoint expansion.".into()};
    write_json(&out.join("protocol.json"), &protocol)?;
    let mut results = vec![];
    let mut attempts = vec![];
    for spec in &protocol.specs {
        let g = match build(&input, &spec.source) {
            Ok(g) => g,
            Err(error) => {
                attempts.push(serde_json::json!({"graph":spec.name,"status":"failed","error":format!("{error:#}")}));
                write_json(&out.join("attempts.json"), &attempts)?;
                continue;
            }
        };
        let gd = out.join("graphs").join(graph_key(&spec.source)?);
        std::fs::create_dir_all(&gd)?;
        write_json(&gd.join("graph.json"), &g)?;
        write_json(&gd.join("source.json"), &spec.source)?;
        write_json(&gd.join("metrics.json"), &metrics::graph_metrics(&g, None)?)?;
        for &q in &spec.q {
            let evaluated = evaluate(
                &input,
                &g,
                spec,
                q,
                &protocol.screening_seeds,
                &protocol.reference_profile,
                out,
            );
            match evaluated {
                Ok(c) => {
                    attempts.push(serde_json::json!({"name":c.name,"status":"complete"}));
                    results.push(c);
                }
                Err(error) => {
                    eprintln!("Failed {} q={q}: {error:#}", spec.name);
                    attempts.push(serde_json::json!({"graph":spec.name,"q":q,"status":"failed","error":format!("{error:#}")}));
                }
            }
            write_json(&out.join("attempts.json"), &attempts)?;
            write_json(&out.join("summary.json"), &results)?;
        }
    }
    let selected = shortlist(&results)?;
    write_json(
        &out.join("shortlist.json"),
        &Shortlist {
            protocol_sha256: sha256(&out.join("protocol.json"))?,
            summary_sha256: sha256(&out.join("summary.json"))?,
            candidates: selected,
            rule: protocol.selection_rule.clone(),
        },
    )?;
    Ok(())
}
fn stress(
    input: &Embeddings,
    graph: &Graph,
    c: &mut Candidate,
    seeds: &[u64],
    out: &Path,
) -> Result<()> {
    for &seed in seeds {
        eprintln!("stress {} seed {seed}", c.name);
        let cfg = LeidenConfig {
            seed,
            resolution: c.resolution,
            theta: c.theta,
            ..Default::default()
        };
        let dir = out
            .join("stress")
            .join(&c.name)
            .join(format!("seed-{seed}"));
        let (_, base) = fit(input, graph, &cfg, &dir.join("comparator"))?;
        let mut edge_rng = Xoshiro256PlusPlus::seed_from_u64(seed);
        let mut shuffled: Vec<_> = (0..graph.edges.len()).collect();
        shuffled.shuffle(&mut edge_rng);
        let removed = ((graph.edges.len() as f64) * 0.05).round() as usize;
        let mut keep = vec![true; graph.edges.len()];
        for &i in &shuffled[..removed] {
            keep[i] = false;
        }
        let perturbed = Graph {
            tokens: graph.tokens.clone(),
            edges: graph
                .edges
                .iter()
                .zip(keep)
                .filter_map(|(e, k)| k.then_some(e.clone()))
                .collect(),
        };
        let (_, part) = fit(input, &perturbed, &cfg, &dir.join("edge_removal"))?;
        c.stress.push(Stress {
            kind: "edge_removal".into(),
            seed,
            ari: metrics::agreement(&base, &part)?.ari,
            retained_tokens: input.tokens.len(),
        });
        let mut subsample_rng = Xoshiro256PlusPlus::seed_from_u64(seed);
        let mut indices: Vec<_> = (0..input.tokens.len()).collect();
        indices.shuffle(&mut subsample_rng);
        indices.truncate(((input.tokens.len() as f64) * 0.9).round().max(1.) as usize);
        indices.sort();
        let subset = input.subset(&indices);
        // Fixed raw threshold edges induce exactly. kNN requires refitting transforms
        // and rebuilding neighbors on the retained vocabulary.
        let sampled = match &c.spec.source {
            Source::Threshold(_) => graph.induced(&indices),
            Source::Neighbors(_) => build(&subset, &c.spec.source)?,
        };
        let (_, part) = fit(&subset, &sampled, &cfg, &dir.join("vocabulary_subsample"))?;
        let retained: Vec<_> = indices.iter().map(|&i| base[i]).collect();
        c.stress.push(Stress {
            kind: "vocabulary_subsample".into(),
            seed,
            ari: metrics::agreement(&retained, &part)?.ari,
            retained_tokens: subset.tokens.len(),
        });
        write_json(
            &dir.join("sampling.json"),
            &serde_json::json!({"seed":seed,"removed_edges":removed,"retained_indices":indices,"source":c.spec.source}),
        )?;
        write_json(&c.directory.join("candidate.json"), c)?;
    }
    Ok(())
}
fn confirm(screening: &Path, out: &Path) -> Result<()> {
    let protocol: Protocol = read_json(&screening.join("protocol.json"))?;
    let frozen: Shortlist = read_json(&screening.join("shortlist.json"))?;
    ensure!(
        frozen.protocol_sha256 == sha256(&screening.join("protocol.json"))?
            && frozen.summary_sha256 == sha256(&screening.join("summary.json"))?,
        "Frozen screening evidence changed"
    );
    ensure!(
        protocol.code_hashes == source_hashes(),
        "Study build changed since screening"
    );
    ensure!(
        protocol.input_sha256 == sha256(&protocol.input)?
            && protocol.reference_sha256 == sha256(&protocol.reference)?,
        "Input/reference changed"
    );
    let screened: Vec<Candidate> = read_json(&screening.join("summary.json"))?;
    ensure!(
        shortlist(&screened)? == frozen.candidates,
        "Shortlist violates frozen rule"
    );
    let input = Embeddings::load(&protocol.input)?;
    fresh_directory(out)?;
    write_json(&out.join("protocol.json"), &protocol)?;
    write_json(&out.join("shortlist.json"), &frozen)?;
    let mut results = vec![];
    for name in &frozen.candidates {
        let c = screened
            .iter()
            .find(|c| &c.name == name)
            .context("Unknown finalist")?;
        let gp = screening
            .join("graphs")
            .join(&c.graph_key)
            .join("graph.json");
        let graph = load_graph(&gp)?;
        ensure!(graph.tokens == input.tokens, "Graph vocabulary changed");
        // Rebuild to validate cached graph integrity, outside all solver timing.
        let rebuilt = build(&input, &c.spec.source)?;
        ensure!(
            rebuilt.tokens == graph.tokens && rebuilt.edges == graph.edges,
            "Cached graph changed"
        );
        let mut confirmed = evaluate(
            &input,
            &graph,
            &c.spec,
            c.q,
            &protocol.confirmation_seeds,
            &protocol.reference_profile,
            out,
        )?;
        ensure!(
            confirmed.resolution == c.resolution && confirmed.theta == c.theta,
            "Confirmation retuned settings"
        );
        stress(&input, &graph, &mut confirmed, &protocol.stress_seeds, out)?;
        results.push(confirmed);
        write_json(&out.join("summary.json"), &results)?;
    }
    let mut qualified: Vec<_> = results
        .iter()
        .filter(|c| {
            eligible(c)
                && c.repeat_ari >= 0.65
                && ["edge_removal", "vocabulary_subsample"].iter().all(|kind| {
                    metrics::distribution(
                        c.stress.iter().filter(|s| &s.kind == kind).map(|s| s.ari),
                    )
                    .median
                    .is_some_and(|v| v >= 0.65)
                })
        })
        .collect();
    qualified.sort_by(|a, b| {
        let dev = |c: &Candidate| {
            screened
                .iter()
                .find(|x| x.name == c.name)
                .unwrap()
                .profile_distance
        };
        dev(a).total_cmp(&dev(b)).then(a.name.cmp(&b.name))
    });
    if let Some(best) = qualified.first() {
        let labels = read_assignments(&best.directory.join("communities.csv"), &input.tokens)?;
        write_assignments(&out.join("communities.csv"), &input.tokens, &labels)?;
        write_json(
            &out.join("recommendation.json"),
            &serde_json::json!({"name":best.name,"representative_seed":best.representative_seed,"assignment_sha256":sha256(&out.join("communities.csv"))?,"rule":protocol.selection_rule,"shortlist_sha256":sha256(&screening.join("shortlist.json"))?}),
        )?;
    } else {
        write_json(
            &out.join("recommendation.json"),
            &serde_json::json!({"name":null,"reason":"No finalist passed the frozen confirmation guardrails"}),
        )?;
    }
    Ok(())
}
fn main() -> Result<()> {
    let command = Cli::parse().command;
    let output = match &command {
        Command::Screen { output, .. } | Command::Confirm { output, .. } => output.clone(),
    };
    ensure!(
        !output.exists(),
        "Output must be a new directory: {}",
        output.display()
    );
    let t = Instant::now();
    let result = match command {
        Command::Screen {
            input,
            reference,
            output,
        } => screen(&input, &reference, &output),
        Command::Confirm { screening, output } => confirm(&screening, &output),
    };
    if output.exists() {
        write_json(
            &output.join("status.json"),
            &serde_json::json!({"status":if result.is_ok(){"complete"}else{"failed"},"elapsed_seconds":t.elapsed().as_secs_f64(),"error":result.as_ref().err().map(|e|format!("{e:#}"))}),
        )?;
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn profile_distance_tracks_size_distribution_without_count_matching() {
        let a = profile(&[0, 0, 1, 1, 1, 2]);
        let b = profile(&[0, 0, 1, 1, 1, 2, 3, 3, 4, 4, 4, 5]);
        assert!(distance(&a, &b) < 1e-15);
        assert_eq!(a.median, 2.);
        assert!(distance(&a, &profile(&[0, 0, 0, 0, 0, 0])) > 0.1);
        assert_eq!(a.group_fractions[..3], [1. / 3., 1. / 3., 1. / 3.]);
    }
}
