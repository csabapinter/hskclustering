use anyhow::{ensure, Context, Result};
use linfa_linalg::eigh::Eigh;
use ndarray::{Array2, Axis};
use serde::{Deserialize, Serialize};
use std::path::Path;

use super::prepare_embeddings;

#[derive(Debug, Serialize, Deserialize)]
pub struct PcaModel {
    pub mean: Vec<f64>,
    /// Unit PCA directions, largest variance first; rows correspond to components.
    pub components: Vec<Vec<f64>>,
    pub explained_variance: Vec<f64>,
    pub retained_variance_ratio: f64,
    pub whiten: bool,
}

pub struct ProjectedEmbeddings {
    pub tokens: Vec<String>,
    pub records: Array2<f64>,
    pub pca: PcaModel,
}

/// GMM uses L2 -> center -> PCA -> optional retained-component whitening.
/// There is deliberately no second row normalization after projection.
pub fn prepare_gmm_embeddings(
    path: &Path,
    components: usize,
    whiten: bool,
) -> Result<ProjectedEmbeddings> {
    let (tokens, vectors) = prepare_embeddings(path, None)?;
    let (records, pca) = project_normalized(&vectors, components, whiten)?;
    Ok(ProjectedEmbeddings {
        tokens,
        records,
        pca,
    })
}

fn project_normalized(
    vectors: &[Vec<f32>],
    components: usize,
    whiten: bool,
) -> Result<(Array2<f64>, PcaModel)> {
    let n = vectors.len();
    ensure!(n >= 2, "PCA requires at least two embedding rows");
    let d = vectors[0].len();
    let maximum = d.min(n - 1);
    ensure!(
        (1..=maximum).contains(&components),
        "PCA components must be in 1..={maximum}, got {components}"
    );
    let data = Array2::from_shape_vec(
        (n, d),
        vectors.iter().flatten().map(|&x| f64::from(x)).collect(),
    )?;
    let mean = data.mean_axis(Axis(0)).context("Empty PCA input")?;
    let centered = data - &mean;
    let covariance = centered.t().dot(&centered) / (n - 1) as f64;
    let total_variance = covariance.diag().sum();
    ensure!(
        total_variance.is_finite() && total_variance > 0.0,
        "No variance after normalizing and centering embeddings"
    );

    // At 300 dimensions the full symmetric decomposition is small and avoids the
    // truncated iterative PCA accuracy issue documented in the Leiden archive.
    let (values, vectors) = covariance
        .eigh()
        .context("Failed to decompose PCA covariance")?;
    ensure!(
        values.iter().all(|v| v.is_finite()) && vectors.iter().all(|v| v.is_finite()),
        "Non-finite PCA decomposition"
    );
    let mut order: Vec<_> = (0..d).collect();
    order.sort_unstable_by(|&a, &b| values[b].total_cmp(&values[a]));
    ensure!(
        values[order[components - 1]] > values[order[0]] * 1e-12,
        "Requested PCA dimension exceeds the numerical rank; retain fewer components"
    );
    let basis = Array2::from_shape_fn((components, d), |(i, j)| vectors[[j, order[i]]]);
    let explained_variance: Vec<_> = order[..components].iter().map(|&i| values[i]).collect();
    let mut projected = centered.dot(&basis.t());
    if whiten {
        for (mut column, &variance) in projected.columns_mut().into_iter().zip(&explained_variance)
        {
            column /= variance.sqrt();
        }
    }
    ensure!(
        projected.iter().all(|v| v.is_finite()),
        "Non-finite projected embeddings"
    );
    let pca = PcaModel {
        mean: mean.to_vec(),
        components: basis.rows().into_iter().map(|row| row.to_vec()).collect(),
        retained_variance_ratio: explained_variance.iter().sum::<f64>() / total_variance,
        explained_variance,
        whiten,
    };
    Ok((projected, pca))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preprocessing_normalizes_before_centering_and_whitens_only_retained_axes() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("input.txt");
        std::fs::write(
            &path,
            "6 3\na 4 0 0\nb -2 0 0\nc 0 3 0\nd 0 -1 0\ne 1 1 2\nf -1 -2 -1\n",
        )?;
        let raw = prepare_gmm_embeddings(&path, 2, false)?;
        let white = prepare_gmm_embeddings(&path, 2, true)?;
        let (_, normalized) = prepare_embeddings(&path, None)?;
        for j in 0..3 {
            let expected = normalized.iter().map(|v| f64::from(v[j])).sum::<f64>() / 6.0;
            assert!((raw.pca.mean[j] - expected).abs() < 1e-12);
        }
        for j in 0..2 {
            assert!(raw.records.column(j).mean().unwrap().abs() < 1e-12);
            for (i, vector) in normalized.iter().enumerate() {
                let projected: f64 = (0..3)
                    .map(|d| (f64::from(vector[d]) - raw.pca.mean[d]) * raw.pca.components[j][d])
                    .sum();
                assert!((raw.records[[i, j]] - projected).abs() < 1e-12);
                assert!(
                    (white.records[[i, j]] - projected / raw.pca.explained_variance[j].sqrt())
                        .abs()
                        < 1e-12
                );
            }
        }
        let covariance = white.records.t().dot(&white.records) / 5.0;
        for i in 0..2 {
            for j in 0..2 {
                assert!((covariance[[i, j]] - if i == j { 1.0 } else { 0.0 }).abs() < 1e-10);
            }
        }
        assert!(raw
            .records
            .rows()
            .into_iter()
            .any(|r| (r.dot(&r) - 1.0).abs() > 0.1));
        assert!(raw.pca.retained_variance_ratio > 0.5 && raw.pca.retained_variance_ratio < 1.0);
        assert!(prepare_gmm_embeddings(&path, 0, false).is_err());
        assert!(prepare_gmm_embeddings(&path, 4, false).is_err());
        Ok(())
    }

    #[test]
    fn rejects_zero_variance_and_rank_deficient_whitening() {
        assert!(
            project_normalized(&[vec![1.0, 0.0], vec![1.0, 0.0], vec![1.0, 0.0]], 1, false)
                .is_err()
        );
        assert!(
            project_normalized(&[vec![1.0, 0.0], vec![-1.0, 0.0], vec![1.0, 0.0]], 2, true)
                .is_err()
        );
    }
}
