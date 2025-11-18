mod preprocess;

pub use preprocess::PcaWhiteningConfig;

use anyhow::{bail, Context, Result};
use preprocess::apply_pca_whitening;
use std::fs::File;
use std::io::{BufRead, BufReader};

/// Load a text SGNS-style embedding file (tokens + raw vectors).
///
/// Format assumptions:
/// - Line 1: `<num_tokens> <dim>`
/// - Lines 2..: `<token> v1 v2 ... v_dim`
///
/// Find example file here:
/// https://github.com/Embedding/Chinese-Word-Vectors
/// sgns, 300d, word + character + ngram
///
/// Returns:
/// - `tokens`: all tokens in order (as owned `String`s)
/// - `vectors`: parallel Vec of raw `Vec<f32>` of length `dim`
pub fn load_embeddings(path: &str) -> Result<(Vec<String>, Vec<Vec<f32>>)> {
    let file = File::open(path).with_context(|| format!("Failed to open {}", path))?;
    let mut reader = BufReader::new(file);

    let mut header = String::new();
    reader
        .read_line(&mut header)
        .context("Failed to read header line")?;
    let mut it = header.split_whitespace();
    let n: usize = it
        .next()
        .context("Header missing token count")?
        .parse()
        .context("Failed to parse token count as usize")?;
    let d: usize = it
        .next()
        .context("Header missing dimension")?
        .parse()
        .context("Failed to parse dimension as usize")?;

    let mut tokens = Vec::with_capacity(n);
    let mut vectors = Vec::with_capacity(n);

    for (line_idx, line) in reader.lines().enumerate() {
        let line = line.with_context(|| format!("Failed to read line {}", line_idx + 2))?;
        let mut parts = line.split_whitespace();

        let token = parts
            .next()
            .with_context(|| format!("Missing token at data line {}", line_idx + 2))?
            .to_string();

        let mut vec = Vec::with_capacity(d);
        for j in 0..d {
            let v = parts
                .next()
                .with_context(|| format!("Line {} missing dim #{}", line_idx + 2, j))?
                .parse::<f32>()
                .with_context(|| {
                    format!("Failed to parse float at line {} dim {}", line_idx + 2, j)
                })?;
            vec.push(v);
        }

        tokens.push(token);
        vectors.push(vec);
    }

    if tokens.len() != n {
        bail!(
            "Header said {} tokens but actually loaded {}. If your file truly has {} rows after the header, update the header or remove this check.",
            n,
            tokens.len(),
            tokens.len()
        );
    }

    Ok((tokens, vectors))
}

/// Load embeddings, optionally run PCA whitening, then L2-normalize rows.
pub fn prepare_embeddings(
    path: &str,
    whitening: Option<PcaWhiteningConfig>,
) -> Result<(Vec<String>, Vec<Vec<f32>>)> {
    let (tokens, mut vectors) = load_embeddings(path)?;

    if let Some(cfg) = whitening {
        vectors = apply_pca_whitening(&vectors, cfg)?;
    }

    l2_normalize_vectors(&mut vectors)?;

    Ok((tokens, vectors))
}

fn l2_normalize_vectors(vectors: &mut [Vec<f32>]) -> Result<()> {
    for (row_idx, vec) in vectors.iter_mut().enumerate() {
        let norm_sq: f32 = vec.iter().map(|x| x * x).sum();
        if norm_sq == 0.0 {
            bail!(
                "Zero-norm vector encountered at row {} (token index {})",
                row_idx,
                row_idx
            );
        }
        let norm = norm_sq.sqrt();
        for x in vec.iter_mut() {
            *x /= norm;
        }
    }
    Ok(())
}

/// Compute cosine similarity between two already-L2-normalized vectors of equal length.
#[inline]
pub fn cosine_normalized(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    let mut s: f32 = 0.0;
    for i in 0..a.len() {
        s += a[i] * b[i];
    }
    s
}
