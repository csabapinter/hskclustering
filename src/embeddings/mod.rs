mod preprocess;
mod projection;

pub use preprocess::PcaWhiteningConfig;
pub use projection::{prepare_gmm_embeddings, PcaModel, ProjectedEmbeddings};

use anyhow::{bail, ensure, Context, Result};
use preprocess::apply_pca_whitening;
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

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
pub fn load_embeddings(path: impl AsRef<Path>) -> Result<(Vec<String>, Vec<Vec<f32>>)> {
    let path = path.as_ref();
    let file = File::open(path).with_context(|| format!("Failed to open {}", path.display()))?;
    read_embeddings(BufReader::new(file))
        .with_context(|| format!("Invalid embeddings in {}", path.display()))
}

fn read_embeddings(mut reader: impl BufRead) -> Result<(Vec<String>, Vec<Vec<f32>>)> {
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
    ensure!(
        it.next().is_none(),
        "Header must contain exactly a token count and dimension"
    );
    ensure!(d > 0, "Embedding dimension must be positive");

    // Do not trust an unvalidated header enough to allocate its declared size.
    let mut tokens = Vec::new();
    let mut vectors = Vec::new();
    let mut seen = HashSet::new();

    for (line_idx, line) in reader.lines().enumerate() {
        let line = line.with_context(|| format!("Failed to read line {}", line_idx + 2))?;
        let mut parts = line.split_whitespace();

        let token = parts
            .next()
            .with_context(|| format!("Missing token at data line {}", line_idx + 2))?
            .to_string();
        ensure!(
            tokens.len() < n,
            "More rows than the header's {n} tokens (line {})",
            line_idx + 2
        );
        ensure!(
            seen.insert(token.clone()),
            "Duplicate token {token:?} at line {}",
            line_idx + 2
        );

        let mut vec = Vec::new();
        for j in 0..d {
            let v = parts
                .next()
                .with_context(|| format!("Line {} missing dim #{}", line_idx + 2, j))?
                .parse::<f32>()
                .with_context(|| {
                    format!("Failed to parse float at line {} dim {}", line_idx + 2, j)
                })?;
            ensure!(
                v.is_finite(),
                "Non-finite value for {token:?} at line {}, dimension {}",
                line_idx + 2,
                j
            );
            vec.push(v);
        }
        ensure!(
            parts.next().is_none(),
            "Extra values for {token:?} at line {}; expected {d} dimensions",
            line_idx + 2
        );

        tokens.push(token);
        vectors.push(vec);
    }

    if tokens.len() != n {
        bail!(
            "Header said {} tokens but actually loaded {}",
            n,
            tokens.len()
        );
    }

    Ok((tokens, vectors))
}

/// Load embeddings, optionally run PCA whitening, then L2-normalize rows.
pub fn prepare_embeddings(
    path: impl AsRef<Path>,
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
        // f64 accumulation avoids overflow/underflow for finite f32 input.
        let norm_sq: f64 = vec.iter().map(|&x| f64::from(x).powi(2)).sum();
        if !norm_sq.is_finite() || norm_sq == 0.0 {
            bail!("Zero or non-finite vector norm at token index {}", row_idx);
        }
        let norm = norm_sq.sqrt();
        for x in vec.iter_mut() {
            *x = (f64::from(*x) / norm) as f32;
        }
    }
    Ok(())
}

/// Compute cosine similarity between two already-L2-normalized vectors of equal length.
#[inline]
pub fn cosine_normalized(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(
        a.len(),
        b.len(),
        "Cosine vectors must have equal dimensions"
    );
    a.iter()
        .zip(b)
        .map(|(x, y)| x * y)
        .sum::<f32>()
        .clamp(-1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_unicode_tokens_and_exact_shape() -> Result<()> {
        let (tokens, vectors) = read_embeddings("2 2\n你好 3 4\n学习 -1 0\n".as_bytes())?;
        assert_eq!(tokens, ["你好", "学习"]);
        assert_eq!(vectors, [vec![3.0, 4.0], vec![-1.0, 0.0]]);
        Ok(())
    }

    #[test]
    fn rejects_malformed_embeddings() {
        for text in [
            "",
            "1 0\na\n",
            "1 1 extra\na 1\n",
            "1 2\na 1\n",
            "1 1\na 1 2\n",
            "1 1\na NaN\n",
            "1 1\na inf\n",
            "2 1\na 1\na 2\n",
            "2 1\na 1\n",
            "1 1\na 1\nb 2\n",
        ] {
            assert!(
                read_embeddings(text.as_bytes()).is_err(),
                "accepted {text:?}"
            );
        }
    }

    #[test]
    fn normalization_handles_finite_extremes_and_rejects_zero() -> Result<()> {
        let mut vectors = vec![vec![f32::MAX, f32::MAX], vec![f32::MIN_POSITIVE, 0.0]];
        l2_normalize_vectors(&mut vectors)?;
        for vector in &vectors {
            assert!((cosine_normalized(vector, vector) - 1.0).abs() < 1e-6);
        }
        assert!(l2_normalize_vectors(&mut [vec![0.0, 0.0]]).is_err());
        assert!(l2_normalize_vectors(&mut [vec![f32::NAN]]).is_err());
        Ok(())
    }
}
