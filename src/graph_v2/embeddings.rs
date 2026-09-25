use anyhow::{ensure, Context, Result};
use linfa_linalg::eigh::Eigh;
use ndarray::Array2;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Representation {
    #[default]
    Raw,
    Center,
    Abtt(usize),
}

/// Canonical token order and unchanged, normalized source-space coordinates.
#[derive(Clone, Debug)]
pub struct Embeddings {
    pub tokens: Vec<String>,
    pub vectors: Vec<Vec<f64>>,
}

impl Embeddings {
    pub fn load(path: &Path) -> Result<Self> {
        Self::read(BufReader::new(File::open(path)?))
            .with_context(|| format!("Invalid embeddings in {}", path.display()))
    }

    pub fn read(mut reader: impl BufRead) -> Result<Self> {
        let mut header = String::new();
        reader.read_line(&mut header)?;
        let fields: Vec<_> = header.split_whitespace().collect();
        ensure!(
            fields.len() == 2,
            "Expected <token count> <dimension> header"
        );
        let n: usize = fields[0].parse()?;
        let d: usize = fields[1].parse()?;
        ensure!(n > 0 && d > 0, "Empty input or zero dimension");
        let mut rows = BTreeMap::new();
        for (line, text) in reader.lines().enumerate() {
            let text = text?;
            let mut fields = text.split_whitespace();
            let token = fields
                .next()
                .with_context(|| format!("Missing token at line {}", line + 2))?;
            ensure!(rows.len() < n, "Extra embedding at line {}", line + 2);
            let mut vector = fields
                .map(str::parse::<f64>)
                .collect::<std::result::Result<Vec<_>, _>>()
                .with_context(|| {
                    format!("Invalid coordinate for {token:?} at line {}", line + 2)
                })?;
            ensure!(
                vector.len() == d,
                "Wrong dimension for {token:?} at line {}",
                line + 2
            );
            normalize(&mut vector)
                .with_context(|| format!("Token {token:?} at line {}", line + 2))?;
            ensure!(
                rows.insert(token.to_string(), vector).is_none(),
                "Duplicate token {token:?}"
            );
        }
        ensure!(
            rows.len() == n,
            "Header declares {n} tokens, found {}",
            rows.len()
        );
        let (tokens, vectors) = rows.into_iter().unzip();
        Ok(Self { tokens, vectors })
    }

    pub fn subset(&self, indices: &[usize]) -> Self {
        Self {
            tokens: indices.iter().map(|&i| self.tokens[i].clone()).collect(),
            vectors: indices.iter().map(|&i| self.vectors[i].clone()).collect(),
        }
    }
}

pub fn normalize(vector: &mut [f64]) -> Result<()> {
    ensure!(vector.iter().all(|x| x.is_finite()), "Nonfinite coordinate");
    let scale = vector.iter().map(|v| v.abs()).fold(0.0, f64::max);
    ensure!(scale > 0.0, "Zero vector");
    let norm = vector
        .iter()
        .map(|x| (x / scale).powi(2))
        .sum::<f64>()
        .sqrt();
    for x in vector {
        *x = (*x / scale) / norm;
    }
    Ok(())
}

pub fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(a, b)| a * b).sum()
}

pub fn cosine(a: &[f64], b: &[f64]) -> f64 {
    // Identical normalized vectors have zero distance. A dot product just below
    // one due to rounding must not become a nonzero local scale for duplicates.
    let value = if a == b {
        1.0
    } else {
        dot(a, b).clamp(-1.0, 1.0)
    };
    // Signed zeros are equal scores and must use the same lexical tie rule.
    if value == 0.0 {
        0.0
    } else {
        value
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FittedTransform {
    pub representation: Representation,
    pub singleton_bypass: bool,
    pub mean: Vec<f64>,
    /// Covariance directions in descending eigenvalue order; never whitened.
    pub basis: Vec<Vec<f64>>,
    pub eigenvalues: Vec<f64>,
    pub eigen_residuals: Vec<f64>,
    pub numerical_rank: Option<usize>,
    pub rank_relative_tolerance: f64,
}

pub fn transform(
    input: &Embeddings,
    representation: Representation,
) -> Result<(Vec<Vec<f64>>, FittedTransform)> {
    let n = input.tokens.len();
    ensure!(
        n > 0 && input.vectors.len() == n,
        "Empty or misaligned input"
    );
    let d = input.vectors[0].len();
    ensure!(d > 0, "Zero dimension");
    for row in &input.vectors {
        ensure!(
            row.len() == d
                && row.iter().all(|x| x.is_finite())
                && (dot(row, row) - 1.0).abs() < 1e-10,
            "Transform requires normalized finite vectors"
        );
    }
    let mut model = FittedTransform {
        representation,
        singleton_bypass: n == 1,
        mean: vec![0.0; d],
        basis: vec![],
        eigenvalues: vec![],
        eigen_residuals: vec![],
        numerical_rank: None,
        rank_relative_tolerance: 1e-12,
    };
    if representation == Representation::Raw || n == 1 {
        let mut rows = input.vectors.clone();
        if n > 1 {
            for row in &mut rows {
                normalize(row)?;
            }
        }
        return Ok((rows, model));
    }
    for row in &input.vectors {
        for (mean, &x) in model.mean.iter_mut().zip(row) {
            *mean += x / n as f64;
        }
    }
    let mut centered: Vec<Vec<f64>> = input
        .vectors
        .iter()
        .map(|row| row.iter().zip(&model.mean).map(|(x, m)| x - m).collect())
        .collect();
    if let Representation::Abtt(m) = representation {
        ensure!(
            m > 0 && m <= d.min(n - 1),
            "ABTT components must be in 1..={}",
            d.min(n - 1)
        );
        let records = Array2::from_shape_vec((n, d), centered.iter().flatten().copied().collect())?;
        let covariance = records.t().dot(&records) / (n - 1) as f64;
        let (values, vectors) = covariance
            .clone()
            .eigh()
            .context("Symmetric eigendecomposition failed")?;
        ensure!(
            values.iter().chain(vectors.iter()).all(|v| v.is_finite()),
            "Nonfinite eigensystem"
        );
        let mut order: Vec<_> = (0..d).collect();
        order.sort_by(|&a, &b| values[b].total_cmp(&values[a]).then(a.cmp(&b)));
        let top = values[order[0]];
        let rank = order
            .iter()
            .filter(|&&i| values[i] > top * model.rank_relative_tolerance && values[i] > 0.0)
            .count();
        model.numerical_rank = Some(rank);
        ensure!(
            m <= rank,
            "ABTT component count {m} exceeds numerical rank {rank}"
        );
        // Removing the entire range leaves only floating point noise.
        ensure!(
            m < rank,
            "ABTT removes all variance (zero transformed vectors)"
        );
        for &index in &order[..m] {
            let mut basis = vectors.column(index).to_vec();
            let pivot = (0..d)
                .max_by(|&a, &b| basis[a].abs().total_cmp(&basis[b].abs()).then(b.cmp(&a)))
                .unwrap();
            if basis[pivot] < 0.0 {
                for x in &mut basis {
                    *x = -*x;
                }
            }
            let residual = (0..d)
                .map(|i| {
                    let cv: f64 = (0..d).map(|j| covariance[[i, j]] * basis[j]).sum();
                    (cv - values[index] * basis[i]).powi(2)
                })
                .sum::<f64>()
                .sqrt();
            ensure!(
                residual <= top * 1e-8 + 1e-14,
                "Excessive eigen residual {residual}"
            );
            model.eigenvalues.push(values[index]);
            model.eigen_residuals.push(residual);
            model.basis.push(basis);
        }
        for row in &mut centered {
            let projections: Vec<_> = model.basis.iter().map(|b| dot(row, b)).collect();
            for (basis, p) in model.basis.iter().zip(projections) {
                for (x, b) in row.iter_mut().zip(basis) {
                    *x -= p * b;
                }
            }
        }
    }
    for (token, row) in input.tokens.iter().zip(&mut centered) {
        ensure!(
            dot(row, row) > 1e-28,
            "Zero transformed vector for {token:?} (numerical tolerance 1e-14)"
        );
        normalize(row).with_context(|| format!("Transformed token {token:?}"))?;
    }
    Ok((centered, model))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_and_normalizes_without_f32_roundtrip() -> Result<()> {
        let x = Embeddings::read("2 2\nb 1e300 1e300\na 1e-300 0\n".as_bytes())?;
        assert_eq!(x.tokens, ["a", "b"]);
        assert_eq!(x.vectors[0], [1.0, 0.0]);
        for s in [
            "0 2\n",
            "1 2\na 0 0\n",
            "2 1\na 1\na 2\n",
            "1 1\na NaN\n",
            "1 2\na 1\n",
        ] {
            assert!(Embeddings::read(s.as_bytes()).is_err(), "{s}");
        }
        Ok(())
    }
    #[test]
    fn abtt_residuals_and_small_input() -> Result<()> {
        let x = Embeddings::read(
            "6 3\na 1 0 0\nb -1 0 0\nc 1 1 0\nd -1 -1 0\ne 0 1 2\nf 0 -1 -2\n".as_bytes(),
        )?;
        let (rows, fitted) = transform(&x, Representation::Abtt(1))?;
        assert!(fitted.eigen_residuals[0] < 1e-10);
        for row in rows {
            assert!(dot(&row, &fitted.basis[0]).abs() < 1e-10);
        }
        assert!(transform(&x, Representation::Abtt(4)).is_err());
        let one = x.subset(&[0]);
        assert!(transform(&one, Representation::Abtt(3))?.1.singleton_bypass);
        let same = Embeddings::read("2 1\na 1\nb 1\n".as_bytes())?;
        assert!(transform(&same, Representation::Center).is_err());
        Ok(())
    }
}
