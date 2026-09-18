use anyhow::{ensure, Context, Result};
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn run(binary: &str, cwd: &Path, arguments: &[&str]) -> Result<Output> {
    Command::new(binary)
        .current_dir(cwd)
        .args(arguments)
        .output()
        .context("Failed to run CLI")
}

fn succeeds(binary: &str, cwd: &Path, arguments: &[&str]) -> Result<Output> {
    let output = run(binary, cwd, arguments)?;
    ensure!(
        output.status.success(),
        "{} failed: {}",
        binary,
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output)
}

#[test]
fn default_pipeline_writes_full_range_directory_and_covers_every_token() -> Result<()> {
    let dir = tempfile::tempdir()?;
    fs::create_dir(dir.path().join("data"))?;
    fs::write(
        dir.path().join("data/input-embeddings.txt"),
        "3 2\n一 1 0\n二 1 0\n独立 -1 0\n",
    )?;
    succeeds(env!("CARGO_BIN_EXE_build_graph"), dir.path(), &[])?;
    succeeds(env!("CARGO_BIN_EXE_filter_graph"), dir.path(), &[])?;
    let observation = succeeds(env!("CARGO_BIN_EXE_observe_graph"), dir.path(), &[])?;
    assert!(String::from_utf8_lossy(&observation.stdout).contains("Nodes: 3"));
    succeeds(env!("CARGO_BIN_EXE_leiden_community"), dir.path(), &[])?;
    let path = dir
        .path()
        .join("results/leiden/1-9/hsk1-9-thresh0.70.communities.csv");
    let records = csv::Reader::from_path(path)?
        .records()
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let tokens: HashSet<_> = records.iter().map(|record| &record[0]).collect();
    assert_eq!(tokens, HashSet::from(["一", "二", "独立"]));
    assert_eq!(records.len(), 3);
    assert!(!dir.path().join("results/leiden/1-3").exists());
    Ok(())
}

#[test]
fn invalid_input_and_settings_do_not_replace_output() -> Result<()> {
    let dir = tempfile::tempdir()?;
    fs::write(dir.path().join("bad.txt"), "1 1\nword NaN\n")?;
    fs::write(dir.path().join("existing.graphml"), "keep this")?;
    let failed = run(
        env!("CARGO_BIN_EXE_build_graph"),
        dir.path(),
        &["-i", "bad.txt", "-o", "existing.graphml"],
    )?;
    assert!(!failed.status.success());
    assert_eq!(
        fs::read_to_string(dir.path().join("existing.graphml"))?,
        "keep this"
    );
    let failed = run(
        env!("CARGO_BIN_EXE_leiden_community"),
        dir.path(),
        &["--theta", "0"],
    )?;
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("Theta"));
    let failed = run(
        env!("CARGO_BIN_EXE_build_graph"),
        dir.path(),
        &["-i", "bad.txt", "-o", "bad.txt"],
    )?;
    assert!(!failed.status.success());
    assert_eq!(
        fs::read_to_string(dir.path().join("bad.txt"))?,
        "1 1\nword NaN\n"
    );
    Ok(())
}

fn gmm_input(directory: &Path) -> Result<()> {
    let mut text = String::from("80 3\n");
    for i in 0..80 {
        let center = if i < 40 { -3.0 } else { 3.0 };
        text.push_str(&format!(
            "词{i} {} {} {}\n",
            center + (i % 7) as f64 / 20.0,
            ((i * 11) % 19) as f64 / 25.0,
            1.0 + ((i * 3) % 11) as f64 / 40.0
        ));
    }
    fs::write(directory.join("embeddings.txt"), text)?;
    Ok(())
}

#[test]
fn gmm_sweep_exports_matching_probabilities_and_compares_saved_partitions() -> Result<()> {
    use hskclustering::{clustering::read_assignments, embeddings::load_embeddings};
    let dir = tempfile::tempdir()?;
    gmm_input(dir.path())?;
    succeeds(
        env!("CARGO_BIN_EXE_gmm_cluster"),
        dir.path(),
        &[
            "--input",
            "embeddings.txt",
            "--output",
            "gmm",
            "--pca-components",
            "2",
            "--whiten",
            "--clusters",
            "1,2",
            "--seeds",
            "7,8",
            "--regularization",
            "0.000001",
            "--criterion",
            "aic",
        ],
    )?;
    let output = dir.path().join("gmm");
    let record: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(output.join("run.json"))?)?;
    let candidates = record["candidates"].as_array().unwrap();
    assert_eq!(candidates.len(), 4);
    let minimum = candidates
        .iter()
        .filter_map(|c| c["scores"]["aic"].as_f64())
        .fold(f64::INFINITY, f64::min);
    assert_eq!(
        record["selected"]["scores"]["aic"].as_f64().unwrap(),
        minimum
    );
    assert_eq!(record["selected"]["config"]["clusters"], 2);
    let (tokens, _) = load_embeddings(dir.path().join("embeddings.txt"))?;
    let labels = read_assignments(&output.join("communities.csv"), &tokens)?;
    let index: std::collections::HashMap<_, _> = tokens.iter().zip(&labels).collect();
    let mut reader = csv::Reader::from_path(output.join("memberships.csv"))?;
    assert_eq!(
        reader.headers()?.iter().collect::<Vec<_>>(),
        ["token", "probability_0", "probability_1"]
    );
    let mut count = 0;
    for row in reader.records() {
        let row = row?;
        let probabilities = row
            .iter()
            .skip(1)
            .map(str::parse::<f64>)
            .collect::<std::result::Result<Vec<_>, _>>()?;
        assert!((probabilities.iter().sum::<f64>() - 1.0).abs() < 1e-8);
        let best = if probabilities[1] > probabilities[0] {
            1
        } else {
            0
        };
        assert_eq!(**index.get(&row[0].to_owned()).unwrap(), best);
        count += 1;
    }
    assert_eq!(count, 80);
    succeeds(
        env!("CARGO_BIN_EXE_compare_clusters"),
        dir.path(),
        &[
            "gmm/communities.csv",
            "gmm/k2-reg0.000001-seed7.communities.csv",
            "--input",
            "embeddings.txt",
            "--output",
            "comparison.json",
        ],
    )?;
    let comparison: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(dir.path().join("comparison.json"))?)?;
    assert_eq!(comparison["partitions"][0]["metrics"]["tokens"], 80);
    assert_eq!(comparison["agreement"][0]["adjusted_rand_index"], 1.0);
    let previous = fs::read(output.join("communities.csv"))?;
    let rerun = run(
        env!("CARGO_BIN_EXE_gmm_cluster"),
        dir.path(),
        &["--output", "gmm"],
    )?;
    assert!(!rerun.status.success());
    assert_eq!(fs::read(output.join("communities.csv"))?, previous);
    Ok(())
}

#[test]
fn gmm_failed_fits_are_recorded_but_never_selected() -> Result<()> {
    let dir = tempfile::tempdir()?;
    gmm_input(dir.path())?;
    let failed = run(
        env!("CARGO_BIN_EXE_gmm_cluster"),
        dir.path(),
        &[
            "--input",
            "embeddings.txt",
            "--output",
            "failed",
            "--pca-components",
            "2",
            "--clusters",
            "2",
            "--seeds",
            "42",
            "--max-iterations",
            "1",
        ],
    )?;
    assert!(!failed.status.success());
    let record: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(dir.path().join("failed/run.json"))?)?;
    assert!(record["selected"].is_null());
    assert!(record["candidates"][0]["scores"].is_null());
    assert!(record["candidates"][0]["error"]
        .as_str()
        .unwrap()
        .contains("converge"));
    assert!(!dir.path().join("failed/communities.csv").exists());
    assert!(!dir.path().join("failed/memberships.csv").exists());
    Ok(())
}
