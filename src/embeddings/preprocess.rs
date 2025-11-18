use anyhow::{bail, Context, Result};
use linfa::prelude::Transformer;
use linfa::{traits::Fit, Dataset};
use linfa_reduction::Pca;
use ndarray::{Array1, Array2, Axis};

#[derive(Clone, Copy, Debug)]
pub struct PcaWhiteningConfig {
    pub components: Option<usize>,
    pub whiten: bool,
}

impl PcaWhiteningConfig {
    pub fn whitening(components: Option<usize>) -> Self {
        Self {
            components, // output dimension after dimensionality reduction
            whiten: true,
        }
    }
}

pub fn apply_pca_whitening(vectors: &[Vec<f32>], cfg: PcaWhiteningConfig) -> Result<Vec<Vec<f32>>> {
    if vectors.is_empty() {
        return Ok(Vec::new());
    }
    let n = vectors.len();
    let d = vectors[0].len();
    if d == 0 {
        bail!("Embedding dimension reported as 0; cannot run PCA");
    }
    for (idx, row) in vectors.iter().enumerate() {
        if row.len() != d {
            bail!("Row {} has dimension {} but expected {}", idx, row.len(), d);
        }
    }

    let mut data = Array2::<f64>::zeros((n, d));
    for (i, row) in vectors.iter().enumerate() {
        for (j, value) in row.iter().enumerate() {
            data[[i, j]] = *value as f64;
        }
    }

    let comps = cfg.components.unwrap_or(d).min(d);
    if comps == 0 {
        bail!("PCA components must be >= 1");
    }

    let targets = Array1::<f64>::zeros(n);
    let dataset = Dataset::from((data, targets));
    let fitted = Pca::params(comps)
        .whiten(cfg.whiten)
        .fit(&dataset)
        .context("Failed to fit PCA model for embeddings")?;
    let transformed = fitted.transform(dataset).records;

    let mut out = Vec::with_capacity(n);
    for row in transformed.axis_iter(Axis(0)) {
        let mut v = Vec::with_capacity(row.len());
        for val in row.iter() {
            v.push(*val as f32);
        }
        out.push(v);
    }

    Ok(out)
}
