use anyhow::{ensure, Result};
use hskclustering::{
    clustering::read_assignments,
    graph_v2::{
        embeddings::Embeddings,
        experiment::{Baseline, Manifest},
        graph::{build, GraphConfig, WeightMode},
        leiden::{self, LeidenConfig},
        metrics,
    },
};
use std::{fs, path::Path, process::Command};

fn cli(cwd: &Path, args: &[&str]) -> Result<std::process::Output> {
    Ok(Command::new(env!("CARGO_BIN_EXE_graph_v2"))
        .current_dir(cwd)
        .args(args)
        .output()?)
}
fn success(cwd: &Path, args: &[&str]) -> Result<()> {
    let out = cli(cwd, args)?;
    ensure!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(())
}

fn mixture() -> String {
    let sizes = [18, 12, 6];
    let mut text = String::from("37 5\n");
    let mut index = 0;
    for (cluster, size) in sizes.into_iter().enumerate() {
        for row in 0..size {
            let spread = [0.01, 0.04, 0.08][cluster];
            let mut vector = [0.0; 5];
            vector[cluster] = 1.0;
            vector[3] = ((row * 7 % 13) as f64 - 6.0) * spread;
            vector[4] = ((row * 5 % 11) as f64 - 5.0) * spread;
            text.push_str(&format!(
                "word{index:02} {} {} {} {} {}\n",
                vector[0], vector[1], vector[2], vector[3], vector[4]
            ));
            index += 1;
        }
    }
    text.push_str("word36 -1 -1 -1 0 0\n");
    text
}

#[test]
fn mixtures_with_unequal_densities_and_outlier_recover_and_remain_connected() -> Result<()> {
    let input = Embeddings::read(mixture().as_bytes())?;
    let built = build(
        &input,
        &GraphConfig {
            k: 8,
            tau: 0.35,
            weight: WeightMode::Local,
            ..Default::default()
        },
    )?;
    let expected: Vec<_> = (0..37)
        .map(|i| {
            if i < 18 {
                0
            } else if i < 30 {
                1
            } else if i < 36 {
                2
            } else {
                3
            }
        })
        .collect();
    for seed in 0..10 {
        let run = leiden::run(
            &built.graph,
            &LeidenConfig {
                seed,
                resolution: 0.01,
                theta: 0.3 * built.graph.median_weight().unwrap(),
                ..Default::default()
            },
        )?;
        let ari = metrics::agreement(&expected, &run.labels)?.ari;
        assert!(
            ari > 0.95,
            "seed {seed}: ARI={ari}, labels={:?}",
            run.labels
        );
        assert!(leiden::community_components(&built.graph, &run.labels)?
            .iter()
            .all(|&n| n == 1));
    }
    Ok(())
}

#[test]
fn cli_is_separate_repeatable_and_never_replaces_existing_output() -> Result<()> {
    let dir = tempfile::tempdir()?;
    fs::write(dir.path().join("input.txt"), mixture())?;
    success(
        dir.path(),
        &[
            "build",
            "-i",
            "input.txt",
            "-o",
            "graph",
            "--k",
            "8",
            "--tau",
            "0.2",
        ],
    )?;
    for output in ["run-a", "run-b"] {
        success(
            dir.path(),
            &[
                "cluster",
                "-i",
                "input.txt",
                "--graph",
                "graph/graph.graphml",
                "-o",
                output,
                "--seed",
                "789",
            ],
        )?;
    }
    assert_eq!(
        fs::read(dir.path().join("run-a/communities.csv"))?,
        fs::read(dir.path().join("run-b/communities.csv"))?
    );
    let before = fs::read(dir.path().join("graph/graph.graphml"))?;
    assert!(
        !cli(dir.path(), &["build", "-i", "input.txt", "-o", "graph"])?
            .status
            .success()
    );
    assert_eq!(fs::read(dir.path().join("graph/graph.graphml"))?, before);
    let input = Embeddings::load(&dir.path().join("input.txt"))?;
    assert_eq!(
        read_assignments(&dir.path().join("run-a/communities.csv"), &input.tokens)?.len(),
        37
    );
    assert!(!dir.path().join("results/leiden").exists());
    Ok(())
}

#[test]
fn experiment_records_failures_caches_graphs_and_retains_baseline_without_policy() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let text = "6 3\na 1 0 0\nb 1 0 0\nc 1 0 0\nd 0 1 0\ne 0 1 0\nf 0 1 0\n";
    let input_path = dir.path().join("input.txt");
    fs::write(&input_path, text)?;
    let input = Embeddings::load(&input_path)?;
    let built = build(
        &input,
        &GraphConfig {
            k: 2,
            ..Default::default()
        },
    )?;
    let graph_path = dir.path().join("baseline.graphml");
    built.graph.write_graphml(&graph_path)?;
    let assignment_path = dir.path().join("baseline.csv");
    hskclustering::clustering::write_assignments(
        &assignment_path,
        &input.tokens,
        &[0, 0, 0, 1, 1, 1],
    )?;
    let manifest = Manifest {
        input: input_path,
        metadata: None,
        baseline: Some(Baseline {
            graph: graph_path,
            recommended_assignments: assignment_path.clone(),
            threshold: 0.7,
            resolution: 0.2,
            theta: 0.3,
        }),
        graphs: vec![
            GraphConfig {
                k: 2,
                ..Default::default()
            },
            GraphConfig {
                k: 2,
                representation: hskclustering::graph_v2::embeddings::Representation::Abtt(3),
                ..Default::default()
            },
        ],
        q: vec![0.1, 0.3],
        archived_thresholds: vec![],
        ablations: false,
        combinations: false,
        max_endpoint_expansions: 0,
        ..Default::default()
    };
    let manifest_path = dir.path().join("manifest.json");
    hskclustering::output::write_json(&manifest_path, &manifest)?;
    success(
        dir.path(),
        &[
            "experiment",
            "--manifest",
            "manifest.json",
            "-o",
            "experiment",
        ],
    )?;
    let report: serde_json::Value = serde_json::from_reader(fs::File::open(
        dir.path().join("experiment/selection-report.json"),
    )?)?;
    assert!(report["development"]["nominee"].is_null());
    assert_eq!(report["default"], "archived-recommended");
    assert_eq!(
        fs::read(dir.path().join("experiment/communities.csv"))?,
        fs::read(&assignment_path)?
    );
    let attempts: serde_json::Value =
        serde_json::from_reader(fs::File::open(dir.path().join("experiment/attempts.json"))?)?;
    assert!(attempts
        .as_array()
        .unwrap()
        .iter()
        .any(|a| a["status"] == "failed" && a["error"].as_str().unwrap().contains("ABTT")));
    let candidates: Vec<hskclustering::graph_v2::experiment::Candidate> = serde_json::from_reader(
        fs::File::open(dir.path().join("experiment/candidates.json"))?,
    )?;
    assert_eq!(candidates.iter().filter(|c| c.stage == "core").count(), 2);
    assert_eq!(candidates[1].graph_key, candidates[2].graph_key);
    for c in &candidates {
        assert_eq!(c.runs.len(), 3);
        assert_eq!(c.perturbations.len(), 20);
        assert!(c.perturbations.iter().all(|p| p.agreement.is_some()));
        assert!(c.sensitivity.iter().any(|s| s.direction == "decrease"));
    }
    let confirmation: Vec<hskclustering::graph_v2::experiment::Candidate> =
        serde_json::from_reader(fs::File::open(
            dir.path().join("experiment/confirmation.json"),
        )?)?;
    assert!(!confirmation.is_empty());
    assert!(confirmation.iter().all(|c| c.runs.len() == 10));
    assert!(dir
        .path()
        .join("experiment/count-matched-comparisons.json")
        .exists());
    Ok(())
}

#[test]
fn manifest_disallows_seed_leakage_and_incomplete_selection_policy() -> Result<()> {
    let mut manifest = Manifest::default();
    manifest.confirmation_seeds[0] = manifest.screening_seeds[0];
    assert!(manifest.validate().is_err());
    let mut json = serde_json::to_value(Manifest::default())?;
    json["selection"] = serde_json::json!({"metrics":[]});
    assert!(serde_json::from_value::<Manifest>(json).is_err());
    Ok(())
}

#[test]
fn explicit_policy_freezes_and_confirms_an_improvement() -> Result<()> {
    use hskclustering::graph_v2::{
        graph::{Edge, Graph},
        selection::{MetricTolerance, SelectionMetric, SelectionPolicy},
    };
    let dir = tempfile::tempdir()?;
    let input_path = dir.path().join("input.txt");
    fs::write(
        &input_path,
        "6 2\na 1 0\nb 1 0\nc 1 0\nd 0.6 0.8\ne 0.6 0.8\nf 0.6 0.8\n",
    )?;
    let input = Embeddings::load(&input_path)?;
    // Legacy shifted cosines: within-group 1; cross-group (0.6+1)/2 = 0.8.
    let graph = Graph {
        tokens: input.tokens.clone(),
        edges: (0..6)
            .flat_map(|u| {
                (u + 1..6).map(move |v| Edge {
                    u,
                    v,
                    weight: if u / 3 == v / 3 { 1.0 } else { 0.8 },
                })
            })
            .collect(),
    };
    let graph_path = dir.path().join("baseline.graphml");
    graph.write_graphml(&graph_path)?;
    let assignments = dir.path().join("baseline.csv");
    hskclustering::clustering::write_assignments(&assignments, &input.tokens, &[0; 6])?;
    let manifest = Manifest {
        input: input_path,
        metadata: None,
        baseline: Some(Baseline {
            graph: graph_path,
            recommended_assignments: assignments,
            threshold: 0.7,
            resolution: 0.01,
            theta: 0.3,
        }),
        graphs: vec![GraphConfig {
            k: 2,
            ..Default::default()
        }],
        q: vec![0.1, 2.0],
        archived_thresholds: vec![],
        ablations: false,
        combinations: false,
        max_endpoint_expansions: 0,
        selection: Some(SelectionPolicy {
            metrics: vec![MetricTolerance {
                metric: SelectionMetric::Cohesion,
                absolute_tolerance: 0.01,
            }],
            minimum_nonsingleton_coverage: 0.5,
            minimum_communities: 2,
            maximum_largest_community_share: 0.9,
            reject_all_singletons: true,
        }),
        ..Default::default()
    };
    hskclustering::output::write_json(&dir.path().join("manifest.json"), &manifest)?;
    success(
        dir.path(),
        &["experiment", "--manifest", "manifest.json", "-o", "study"],
    )?;
    let report: serde_json::Value = serde_json::from_reader(fs::File::open(
        dir.path().join("study/selection-report.json"),
    )?)?;
    assert!(report["development"]["nominee"].is_string());
    assert_eq!(report["confirmation_accepted"], true);
    assert_eq!(report["default"], report["development"]["nominee"]);
    assert_eq!(
        read_assignments(&dir.path().join("study/communities.csv"), &input.tokens)?,
        vec![0, 0, 0, 1, 1, 1]
    );
    Ok(())
}
