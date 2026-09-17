use anyhow::{bail, ensure, Context, Result};
use linfa::prelude::Transformer;
use linfa::{traits::Fit, DatasetBase};
use linfa_reduction::Pca;
use ndarray::{Array2, Axis};

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
    ensure!(
        vectors.len() >= 2,
        "PCA requires at least two embedding rows"
    );
    let n = vectors.len();
    let d = vectors[0].len();
    if d == 0 {
        bail!("Embedding dimension reported as 0; cannot run PCA");
    }
    for (idx, row) in vectors.iter().enumerate() {
        if row.len() != d {
            bail!("Row {} has dimension {} but expected {}", idx, row.len(), d);
        }
        ensure!(
            row.iter().all(|v| v.is_finite()),
            "Non-finite PCA input at row {idx}"
        );
    }

    let mut data = Array2::<f64>::zeros((n, d));
    for (i, row) in vectors.iter().enumerate() {
        for (j, value) in row.iter().enumerate() {
            data[[i, j]] = *value as f64;
        }
    }

    let max_components = d.min(n - 1);
    let comps = cfg.components.unwrap_or(max_components);
    ensure!(
        (1..=max_components).contains(&comps),
        "PCA components must be in 1..={max_components}, got {comps}"
    );

    let dataset = DatasetBase::from(data);
    let fitted = Pca::params(comps)
        .whiten(cfg.whiten)
        .fit(&dataset)
        .context("Failed to fit PCA model for embeddings")?;
    let transformed = fitted.transform(dataset).records;

    let mut out = Vec::with_capacity(n);
    for row in transformed.axis_iter(Axis(0)) {
        let mut v = Vec::with_capacity(row.len());
        for val in row.iter() {
            let value = *val as f32;
            ensure!(
                value.is_finite(),
                "PCA produced a value outside the finite f32 range"
            );
            v.push(value);
        }
        out.push(v);
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pca_upgrade_preserves_whitened_covariance() -> Result<()> {
        let vectors = vec![
            vec![1.0, 0.0],
            vec![-1.0, 0.0],
            vec![0.0, 2.0],
            vec![0.0, -2.0],
        ];
        let output = apply_pca_whitening(&vectors, PcaWhiteningConfig::whitening(None))?;
        assert_eq!(output.len(), 4);
        for i in 0..2 {
            let mean = output.iter().map(|row| row[i]).sum::<f32>() / 4.0;
            assert!(mean.abs() < 1e-6);
            for j in 0..2 {
                let covariance = output.iter().map(|row| row[i] * row[j]).sum::<f32>() / 3.0;
                let expected = if i == j { 1.0 } else { 0.0 };
                assert!((covariance - expected).abs() < 1e-5);
            }
        }
        Ok(())
    }

    #[test]
    fn rejects_invalid_pca_shape_and_component_count() {
        let config = PcaWhiteningConfig::whitening(None);
        assert!(apply_pca_whitening(&[], config).is_err());
        assert!(apply_pca_whitening(&[vec![1.0]], config).is_err());
        assert!(apply_pca_whitening(&[vec![1.0], vec![1.0, 2.0]], config).is_err());
        assert!(apply_pca_whitening(
            &[vec![1.0], vec![2.0]],
            PcaWhiteningConfig::whitening(Some(2))
        )
        .is_err());
    }
}
