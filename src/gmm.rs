use anyhow::{ensure, Context, Result};
use clap::ValueEnum;
use linfa::{traits::Fit, DatasetBase};
use linfa_clustering::GaussianMixtureModel;
use ndarray::Array2;
use rand::SeedableRng;
use rand_xoshiro::Xoshiro256Plus;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::output::write_atomic;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct GmmConfig {
    pub clusters: usize,
    pub regularization: f64,
    pub seed: u64,
    pub max_iterations: u64,
    pub tolerance: f64,
}

impl GmmConfig {
    pub fn validate(&self, samples: usize) -> Result<()> {
        ensure!(
            (1..=samples).contains(&self.clusters),
            "GMM clusters must be in 1..={samples}"
        );
        ensure!(
            self.regularization.is_finite() && self.regularization > 0.0,
            "Regularization must be finite and strictly positive"
        );
        ensure!(
            self.tolerance.is_finite() && self.tolerance > 0.0,
            "Tolerance must be finite and strictly positive"
        );
        ensure!(
            self.max_iterations > 0,
            "Maximum iterations must be positive"
        );
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Criterion {
    Aic,
    Bic,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ModelScores {
    pub log_likelihood: f64,
    pub parameters: usize,
    pub aic: f64,
    pub bic: f64,
}

impl ModelScores {
    /// Full covariance: weights (k-1), means (k*d), symmetric covariances.
    pub fn full_covariance(
        log_likelihood: f64,
        samples: usize,
        dimensions: usize,
        clusters: usize,
    ) -> Result<Self> {
        ensure!(
            samples > 0 && dimensions > 0 && clusters > 0 && log_likelihood.is_finite(),
            "Invalid likelihood or model dimensions"
        );
        let parameters =
            clusters - 1 + clusters * dimensions + clusters * dimensions * (dimensions + 1) / 2;
        let scores = Self {
            log_likelihood,
            parameters,
            aic: 2.0 * parameters as f64 - 2.0 * log_likelihood,
            bic: (samples as f64).ln() * parameters as f64 - 2.0 * log_likelihood,
        };
        ensure!(
            scores.aic.is_finite() && scores.bic.is_finite(),
            "Non-finite information criteria"
        );
        Ok(scores)
    }

    pub fn criterion(self, criterion: Criterion) -> f64 {
        match criterion {
            Criterion::Aic => self.aic,
            Criterion::Bic => self.bic,
        }
    }
}

pub struct GmmFit {
    pub model: GaussianMixtureModel<f64>,
    pub probabilities: Array2<f64>,
    pub assignments: Vec<usize>,
    pub scores: ModelScores,
}

pub fn fit_gmm(records: &Array2<f64>, config: GmmConfig) -> Result<GmmFit> {
    config.validate(records.nrows())?;
    ensure!(
        records.ncols() > 0 && records.iter().all(|v| v.is_finite()),
        "GMM requires finite nonempty observations"
    );
    // Linfa 0.8.1's n_runs continues the same model instead of reinitializing it.
    // The runner supplies independent seeds via separate single-run fits.
    let model = GaussianMixtureModel::params_with_rng(
        config.clusters,
        Xoshiro256Plus::seed_from_u64(config.seed),
    )
    .n_runs(1)
    .reg_covariance(config.regularization)
    .max_n_iterations(config.max_iterations)
    .tolerance(config.tolerance)
    .fit(&DatasetBase::from(records.view()))
    .context("GMM fit failed (including non-convergence)")?;
    ensure!(
        model
            .weights()
            .iter()
            .chain(model.means().iter())
            .chain(model.covariances().iter())
            .chain(model.precisions().iter())
            .all(|v| v.is_finite()),
        "GMM returned non-finite model parameters; increase regularization"
    );
    let likelihoods = model.score_samples(records);
    ensure!(
        likelihoods.iter().all(|v| v.is_finite()),
        "GMM returned non-finite likelihoods"
    );
    let scores = ModelScores::full_covariance(
        likelihoods.sum(),
        records.nrows(),
        records.ncols(),
        config.clusters,
    )?;
    let probabilities = model.predict_proba(records);
    let mut assignments = Vec::with_capacity(records.nrows());
    for row in probabilities.rows() {
        ensure!(
            row.iter().all(|p| p.is_finite() && (0.0..=1.0).contains(p))
                && (row.sum() - 1.0).abs() < 1e-8,
            "GMM returned invalid membership probabilities"
        );
        let mut best = 0;
        for j in 1..row.len() {
            if row[j] > row[best] {
                best = j;
            }
        }
        assignments.push(best);
    }
    Ok(GmmFit {
        model,
        probabilities,
        assignments,
        scores,
    })
}

/// Probability column j and hard assignment j always refer to model component j.
pub fn write_probabilities(
    path: &Path,
    tokens: &[String],
    probabilities: &Array2<f64>,
) -> Result<()> {
    ensure!(
        tokens.len() == probabilities.nrows(),
        "Probability row count must match tokens"
    );
    let mut order: Vec<_> = (0..tokens.len()).collect();
    order.sort_unstable_by(|&a, &b| tokens[a].cmp(&tokens[b]));
    write_atomic(path, |output| {
        let mut writer = csv::Writer::from_writer(output);
        let header = std::iter::once("token".to_owned())
            .chain((0..probabilities.ncols()).map(|j| format!("probability_{j}")));
        writer.write_record(header)?;
        for i in order {
            writer.write_record(
                std::iter::once(tokens[i].clone())
                    .chain(probabilities.row(i).iter().map(ToString::to_string)),
            )?;
        }
        writer.flush()?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn sample() -> Array2<f64> {
        Array2::from_shape_fn((80, 2), |(i, j)| {
            let center = if i < 40 { -3.0 } else { 3.0 };
            center
                + if j == 0 {
                    ((i * 7) % 19) as f64 / 30.0
                } else {
                    ((i * 11) % 23) as f64 / 40.0
                }
        })
    }

    fn config(clusters: usize) -> GmmConfig {
        GmmConfig {
            clusters,
            seed: 42,
            regularization: 1e-6,
            tolerance: 1e-5,
            max_iterations: 200,
        }
    }

    #[test]
    fn seeded_fit_recovers_groups_and_information_criteria_prefer_two() -> Result<()> {
        let data = sample();
        let one = fit_gmm(&data, config(1))?;
        let two = fit_gmm(&data, config(2))?;
        let repeated = fit_gmm(&data, config(2))?;
        assert!(two.scores.aic < one.scores.aic && two.scores.bic < one.scores.bic);
        assert_eq!(two.assignments, repeated.assignments);
        assert_eq!(two.probabilities, repeated.probabilities);
        assert!(two.assignments[..40]
            .iter()
            .all(|&v| v == two.assignments[0]));
        assert!(two.assignments[40..]
            .iter()
            .all(|&v| v != two.assignments[0]));
        let serialized = serde_json::to_string(&two.model)?;
        let restored: GaussianMixtureModel<f64> = serde_json::from_str(&serialized)?;
        for (&a, &b) in restored
            .predict_proba(&data)
            .iter()
            .zip(two.probabilities.iter())
        {
            assert!((a - b).abs() < 1e-12);
        }
        Ok(())
    }

    #[test]
    fn single_gaussian_likelihood_matches_analytic_density() -> Result<()> {
        let data = ndarray::array![[-2.0], [-1.0], [1.0], [2.0]];
        let fitted = fit_gmm(&data, config(1))?;
        let variance = 2.5 + 1e-6;
        let expected: f64 = data
            .iter()
            .map(|&x| -0.5 * ((2.0 * std::f64::consts::PI * variance).ln() + x * x / variance))
            .sum();
        assert!((fitted.scores.log_likelihood - expected).abs() < 1e-10);
        assert_eq!(fitted.scores.parameters, 2);
        assert!((fitted.scores.aic - (4.0 - 2.0 * expected)).abs() < 1e-10);
        assert!((fitted.scores.bic - (2.0 * 4.0_f64.ln() - 2.0 * expected)).abs() < 1e-10);
        Ok(())
    }

    #[test]
    fn nonconvergence_is_an_error_and_extreme_densities_stay_finite() -> Result<()> {
        assert!(fit_gmm(
            &sample(),
            GmmConfig {
                max_iterations: 1,
                ..config(2)
            }
        )
        .is_err());
        let data = Array2::from_shape_fn((20, 120), |(i, j)| ((i + j) % 7) as f64 * 1e-6);
        let fitted = fit_gmm(
            &data,
            GmmConfig {
                regularization: 1e-8,
                ..config(1)
            },
        )?;
        assert!(fitted.scores.log_likelihood > 700.0 * 20.0);
        assert!(fitted
            .probabilities
            .iter()
            .all(|&v| (v - 1.0).abs() < 1e-12));
        assert!(config(100).validate(80).is_err());
        assert!(GmmConfig {
            regularization: f64::NAN,
            ..config(2)
        }
        .validate(80)
        .is_err());
        Ok(())
    }
}
