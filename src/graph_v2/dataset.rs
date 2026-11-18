//! Exact vocabulary filtering and lossless joins back to source row IDs.
use super::embeddings::{normalize, Embeddings};
use crate::{
    clustering::read_assignments,
    output::{sha256, write_atomic, write_json},
};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub id: String,
    pub token: String,
}

fn read_entries(path: &Path, token_column: &str) -> Result<Vec<Entry>> {
    let mut reader = csv::Reader::from_path(path)?;
    let headers = reader.headers()?;
    let id_column = headers
        .iter()
        .position(|x| x == "ID")
        .context("Missing ID column")?;
    let token_column = headers
        .iter()
        .position(|x| x == token_column)
        .with_context(|| format!("Missing {token_column} column"))?;
    let mut ids = BTreeSet::new();
    let mut entries = vec![];
    for row in reader.records() {
        let row = row?;
        let id = row[id_column].to_string();
        let token = row[token_column].to_string();
        ensure!(
            !id.trim().is_empty(),
            "Empty ID at CSV record {}",
            entries.len() + 2
        );
        ensure!(
            !token.is_empty() && !token.chars().any(char::is_whitespace),
            "Empty token or whitespace in token for ID {id:?}"
        );
        ensure!(ids.insert(id.clone()), "Duplicate ID {id:?}");
        entries.push(Entry { id, token });
    }
    ensure!(!entries.is_empty(), "CSV has no entries");
    Ok(entries)
}

/// Read every source row, but parse coordinates only for selected words.
/// Unselected rows must still be well formed and match the declared row count.
fn filter_source(
    mut reader: impl BufRead,
    wanted: &BTreeSet<String>,
) -> Result<(usize, usize, BTreeMap<String, String>)> {
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let header: Vec<_> = line.split_whitespace().collect();
    ensure!(
        header.len() == 2,
        "Expected <token count> <dimension> embedding header"
    );
    let declared: usize = header[0].parse()?;
    let dimension: usize = header[1].parse()?;
    ensure!(
        declared > 0 && dimension > 0,
        "Empty embedding source or zero dimension"
    );
    let mut selected = BTreeMap::new();
    let mut count = 0;
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        count += 1;
        ensure!(count <= declared, "Extra embedding at line {}", count + 1);
        let trimmed = line.trim_end();
        // The archive contains a NBSP token. Columns use ASCII whitespace;
        // Unicode whitespace must not turn that source token into an empty one.
        let split = trimmed
            .find(|c: char| c.is_ascii_whitespace())
            .with_context(|| format!("Missing vector at line {}", count + 1))?;
        let token = &trimmed[..split];
        let coordinates = trimmed[split..].trim_start();
        ensure!(
            !coordinates.is_empty(),
            "Empty vector at line {}",
            count + 1
        );
        // The original character/ngram archive also contains an ASCII space
        // token. It is outside our validated vocabulary and can be skipped.
        if !wanted.contains(token) {
            continue;
        }
        ensure!(
            !selected.contains_key(token),
            "Duplicate source embedding for {token:?}"
        );
        let mut vector: Vec<f64> = coordinates
            .split_whitespace()
            .map(str::parse)
            .collect::<std::result::Result<_, _>>()
            .with_context(|| format!("Invalid coordinate for {token:?}"))?;
        ensure!(
            vector.len() == dimension,
            "Wrong dimension for {token:?}: expected {dimension}, found {}",
            vector.len()
        );
        normalize(&mut vector).with_context(|| format!("Invalid vector for {token:?}"))?;
        selected.insert(token.to_string(), coordinates.to_string());
    }
    ensure!(
        count == declared,
        "Embedding header declares {declared} rows, found {count}"
    );
    Ok((count, dimension, selected))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Preparation {
    pub dataset: PathBuf,
    pub dataset_sha256: String,
    pub source: PathBuf,
    pub source_sha256: String,
    pub entries_sha256: String,
    pub embeddings_sha256: String,
    pub rows: usize,
    pub unique_tokens: usize,
    pub repeated_spelling_rows: usize,
    pub source_rows: usize,
    pub dimension: usize,
    pub matched_tokens: usize,
    pub missing_tokens: usize,
    pub matched_rows: usize,
    pub missing_rows: usize,
    pub matching: String,
}

/// Filter exact source spellings without synthesizing vectors or discarding IDs.
pub fn prepare(dataset: &Path, source: &Path, output: &Path) -> Result<Preparation> {
    ensure!(
        !output.exists(),
        "Output directory already exists: {}",
        output.display()
    );
    let entries = read_entries(dataset, "Tilelabels")?;
    let wanted = entries.iter().map(|x| x.token.clone()).collect();
    let dataset_sha256 = sha256(dataset)?;
    let source_sha256 = sha256(source)?;
    let (source_rows, dimension, selected) = if source.extension().is_some_and(|x| x == "bz2") {
        let mut process = Command::new("bzip2")
            .args(["-dc", "--"])
            .arg(source)
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .context("Failed to start bzip2; install bzip2 or supply a plain text source")?;
        let result = filter_source(
            BufReader::with_capacity(
                1024 * 1024,
                process.stdout.take().context("Missing bzip2 output")?,
            ),
            &wanted,
        );
        if result.is_err() {
            let _ = process.kill();
        }
        let status = process.wait()?;
        let filtered = result?;
        ensure!(status.success(), "bzip2 failed with {status}");
        filtered
    } else {
        filter_source(
            BufReader::with_capacity(1024 * 1024, File::open(source)?),
            &wanted,
        )?
    };
    ensure!(
        !selected.is_empty(),
        "No exact source embeddings matched the input vocabulary"
    );
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(output)
        .with_context(|| format!("Failed to create fresh directory {}", output.display()))?;
    write_atomic(&output.join("embeddings.txt"), |writer| {
        writeln!(writer, "{} {dimension}", selected.len())?;
        for (token, vector) in &selected {
            writeln!(writer, "{token} {vector}")?;
        }
        Ok(())
    })?;
    write_atomic(&output.join("entries.csv"), |writer| {
        let mut csv = csv::Writer::from_writer(writer);
        csv.write_record(["ID", "token"])?;
        for entry in &entries {
            csv.write_record([&entry.id, &entry.token])?;
        }
        csv.flush()?;
        Ok(())
    })?;
    write_atomic(&output.join("missing.csv"), |writer| {
        let mut csv = csv::Writer::from_writer(writer);
        csv.write_record(["ID", "token", "reason"])?;
        for entry in &entries {
            if !selected.contains_key(&entry.token) {
                csv.write_record([&entry.id, &entry.token, "no_exact_source_embedding"])?;
            }
        }
        csv.flush()?;
        Ok(())
    })?;
    let matched_rows = entries
        .iter()
        .filter(|e| selected.contains_key(&e.token))
        .count();
    let preparation = Preparation {
        dataset: dataset.into(),
        dataset_sha256,
        source: source.into(),
        source_sha256,
        entries_sha256: sha256(&output.join("entries.csv"))?,
        embeddings_sha256: sha256(&output.join("embeddings.txt"))?,
        rows: entries.len(),
        unique_tokens: wanted.len(),
        repeated_spelling_rows: entries.len() - wanted.len(),
        source_rows,
        dimension,
        matched_tokens: selected.len(),
        missing_tokens: wanted.len() - selected.len(),
        matched_rows,
        missing_rows: entries.len() - matched_rows,
        matching: "Exact spelling; cluster once per spelling; retain every ID; no fallback vectors"
            .into(),
    };
    write_json(&output.join("preparation.json"), &preparation)?;
    Ok(preparation)
}

/// Require complete assignments for the prepared vocabulary, then restore IDs.
/// Missing-vector rows remain explicit with a blank community.
pub fn export(prepared: &Path, assignments: &Path, output: &Path) -> Result<usize> {
    ensure!(
        !output.exists(),
        "Output file already exists: {}",
        output.display()
    );
    let report: Preparation =
        serde_json::from_reader(File::open(prepared.join("preparation.json"))?)?;
    let entries_path = prepared.join("entries.csv");
    let embeddings_path = prepared.join("embeddings.txt");
    ensure!(
        sha256(&entries_path)? == report.entries_sha256,
        "Prepared entries checksum mismatch"
    );
    ensure!(
        sha256(&embeddings_path)? == report.embeddings_sha256,
        "Prepared embeddings checksum mismatch"
    );
    let entries = read_entries(&entries_path, "token")?;
    let embeddings = Embeddings::load(&embeddings_path)?;
    let labels = read_assignments(assignments, &embeddings.tokens)?;
    let memberships: BTreeMap<_, _> = embeddings.tokens.iter().zip(labels).collect();
    let words: BTreeSet<_> = entries.iter().map(|e| &e.token).collect();
    ensure!(
        memberships.keys().all(|token| words.contains(token)),
        "Prepared embedding token absent from entries"
    );
    ensure!(
        entries.len() == report.rows
            && words.len() == report.unique_tokens
            && memberships.len() == report.matched_tokens,
        "Prepared dataset counts mismatch"
    );
    write_atomic(output, |writer| {
        let mut csv = csv::Writer::from_writer(writer);
        csv.write_record(["ID", "token", "community", "status"])?;
        for entry in &entries {
            let (community, status) = match memberships.get(&entry.token) {
                Some(label) => (label.to_string(), "assigned"),
                None => (String::new(), "missing_embedding"),
            };
            csv.write_record([&entry.id, &entry.token, &community, status])?;
        }
        csv.flush()?;
        Ok(entries.len())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_duplicate_spellings_ids_quoting_and_explicit_missing_rows() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let csv = temp.path().join("dataset.csv");
        let source = temp.path().join("source.txt");
        fs::write(
            &csv,
            "ID,Tilelabels\n\"001,a\",b\n\"quote\"\"id\",a\nthird,b\nmissing,c\n",
        )?;
        fs::write(&source, "3 2\nb 0 1\na 1 0\nunused 1 1\n")?;
        let prepared = temp.path().join("prepared");
        let report = prepare(&csv, &source, &prepared)?;
        assert_eq!(
            (
                report.rows,
                report.unique_tokens,
                report.matched_tokens,
                report.missing_rows
            ),
            (4, 3, 2, 1)
        );
        assert_eq!(
            fs::read_to_string(prepared.join("embeddings.txt"))?,
            "2 2\na 1 0\nb 0 1\n"
        );
        let assignments = temp.path().join("assignments.csv");
        fs::write(&assignments, "token,community\nb,7\na,5\n")?;
        let exported = temp.path().join("exported.csv");
        assert_eq!(export(&prepared, &assignments, &exported)?, 4);
        let rows = csv::Reader::from_path(exported)?
            .records()
            .collect::<std::result::Result<Vec<_>, _>>()?;
        assert_eq!(&rows[0][0], "001,a");
        assert_eq!(&rows[1][0], "quote\"id");
        assert_eq!(&rows[0][2], &rows[2][2]);
        assert_eq!(&rows[3][2], "");
        assert_eq!(&rows[3][3], "missing_embedding");
        assert!(prepare(&csv, &source, &prepared).is_err());
        Ok(())
    }

    #[test]
    fn rejects_invalid_csv_and_selected_source_vectors() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let csv = temp.path().join("dataset.csv");
        for invalid in [
            "ID,Tilelabels\n1,a\n1,b\n",
            "ID,Tilelabels\n,a\n",
            "ID,Tilelabels\n1, \n",
            "ID,Tilelabels\n1,a b\n",
            "ID,Tilelabels\n",
        ] {
            fs::write(&csv, invalid)?;
            assert!(read_entries(&csv, "Tilelabels").is_err(), "{invalid}");
        }
        let wanted = BTreeSet::from(["a".to_string()]);
        for invalid in [
            "1 2\na 1\n",
            "1 1\na NaN\n",
            "1 1\na inf\n",
            "1 1\na 0\n",
            "2 1\na 1\n",
            "1 1\na 1\nb 1\n",
            "2 1\na 1\na 2\n",
            "1 1\n\n",
            "0 1\n",
        ] {
            assert!(
                filter_source(invalid.as_bytes(), &wanted).is_err(),
                "{invalid}"
            );
        }
        let (_, _, vectors) = filter_source("3 1\n\u{a0} 2\n  3\na 1\n".as_bytes(), &wanted)?;
        assert_eq!(vectors.len(), 1);
        assert_eq!(vectors["a"], "1");
        Ok(())
    }

    #[test]
    fn compressed_source_matches_plain_and_rejects_corrupt_archives() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let csv = temp.path().join("dataset.csv");
        let source = temp.path().join("source.txt");
        let compressed = temp.path().join("source.txt.bz2");
        fs::write(&csv, "ID,Tilelabels\n1,a\n2,b\n")?;
        fs::write(&source, "2 2\na 1 0\nb 0 1\n")?;
        let encoded = Command::new("bzip2")
            .args(["-c", "--"])
            .arg(&source)
            .output()?;
        ensure!(encoded.status.success(), "Fixture compression failed");
        fs::write(&compressed, encoded.stdout)?;
        let plain = prepare(&csv, &source, &temp.path().join("plain"))?;
        let zipped = prepare(&csv, &compressed, &temp.path().join("zipped"))?;
        assert_eq!(plain.embeddings_sha256, zipped.embeddings_sha256);
        assert_eq!(plain.entries_sha256, zipped.entries_sha256);
        let mut corrupt = fs::read(&compressed)?;
        corrupt.truncate(corrupt.len() / 2);
        fs::write(&compressed, corrupt)?;
        let bad_output = temp.path().join("corrupt");
        assert!(prepare(&csv, &compressed, &bad_output).is_err());
        assert!(!bad_output.exists());
        Ok(())
    }

    #[test]
    fn rejects_incomplete_extra_duplicate_or_stale_assignments() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let csv = temp.path().join("dataset.csv");
        let source = temp.path().join("source.txt");
        fs::write(&csv, "ID,Tilelabels\n1,a\n2,b\n")?;
        fs::write(&source, "2 1\na 1\nb 2\n")?;
        let prepared = temp.path().join("prepared");
        prepare(&csv, &source, &prepared)?;
        let assignments = temp.path().join("assignments.csv");
        for invalid in [
            "token,community\na,0\n",
            "token,community\na,0\nb,1\nc,2\n",
            "token,community\na,0\na,1\nb,1\n",
            "token,community\na,-1\nb,1\n",
        ] {
            fs::write(&assignments, invalid)?;
            assert!(export(&prepared, &assignments, &temp.path().join("out.csv")).is_err());
        }
        fs::write(&assignments, "token,community\na,0\nb,1\n")?;
        fs::write(prepared.join("entries.csv"), "ID,token\n1,a\n")?;
        assert!(export(&prepared, &assignments, &temp.path().join("out.csv")).is_err());
        Ok(())
    }
}
