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
        .join("graphs/1-9-leiden/hsk1-9-thresh0.70.communities.csv");
    let records = csv::Reader::from_path(path)?
        .records()
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let tokens: HashSet<_> = records.iter().map(|record| &record[0]).collect();
    assert_eq!(tokens, HashSet::from(["一", "二", "独立"]));
    assert_eq!(records.len(), 3);
    assert!(!dir.path().join("graphs/1-3-leiden").exists());
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
