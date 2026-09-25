//! Baseline-only calibration, with disjoint seed cohorts rather than independent ARI pairs.
use super::{
    experiment::{Candidate, Manifest},
    metrics::{distribution, Distribution},
    selection::{MetricTolerance, SelectionMetric, SelectionPolicy},
};
use anyhow::{ensure, Context, Result};
use rand::{seq::SliceRandom, SeedableRng};
use rand_xoshiro::Xoshiro256PlusPlus;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalibrationReference {
    pub report: PathBuf,
    pub report_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CalibrationPlan {
    pub study: Manifest,
    pub solver_seeds: Vec<u64>,
    pub perturbation_seeds: Vec<u64>,
    pub resampling_seed: u64,
    pub resamples: usize,
}
impl Default for CalibrationPlan {
    fn default() -> Self {
        Self {
            study: Manifest {
                ablations: false,
                combinations: false,
                confirmation: false,
                review_anchors: [
                    "苹果",
                    "牛奶",
                    "咖啡",
                    "衬衫",
                    "足球",
                    "老师",
                    "焦虑",
                    "政策",
                    "合同",
                    "银行",
                    "软件",
                    "感冒",
                    "火车",
                    "妈妈",
                    "为什么",
                    "公斤",
                    "花",
                    "行",
                ]
                .into_iter()
                .map(String::from)
                .collect(),
                ..Default::default()
            },
            solver_seeds: (500..530).collect(),
            perturbation_seeds: (40000..40020).collect(),
            resampling_seed: 60000,
            resamples: 10000,
        }
    }
}
impl CalibrationPlan {
    pub fn validate(&self) -> Result<()> {
        self.study.validate()?;
        ensure!(
            self.study.baseline.is_some(),
            "Calibration requires an archived baseline"
        );
        ensure!(
            self.study.selection.is_none() && self.study.calibration.is_none(),
            "Calibration requires an uncalibrated study without a selection policy"
        );
        ensure!(
            self.solver_seeds.len() >= 2 * self.study.screening_seeds.len(),
            "Calibration needs two disjoint solver cohorts of screening size"
        );
        ensure!(
            self.perturbation_seeds.len() >= 2 * self.study.development_perturbation_seeds.len(),
            "Calibration needs two disjoint perturbation cohorts of screening size"
        );
        ensure!(self.resamples >= 1000, "Use at least 1000 cohort splits");
        let mut seen = BTreeSet::new();
        for seeds in [
            &self.study.screening_seeds,
            &self.study.confirmation_seeds,
            &self.study.development_perturbation_seeds,
            &self.study.confirmation_perturbation_seeds,
            &self.study.null_seeds,
            &self.solver_seeds,
            &self.perturbation_seeds,
        ] {
            for &seed in seeds {
                ensure!(
                    seen.insert(seed),
                    "Calibration seed {seed} is duplicated or leaks into the study"
                );
            }
        }
        ensure!(
            !seen.contains(&self.resampling_seed),
            "Resampling seed overlaps a fit or evaluation seed"
        );
        Ok(())
    }
}

pub const METHOD: &str = "100% baseline-only. Shuffle calibration seeds without replacement into two disjoint cohorts matching the planned screening cohort sizes. Compare medians; repeat ARI is the median of the within-cohort pairs, resampling whole partitions, never ARI rows. Absolute tolerance = linear-interpolated 95th percentile of absolute between-cohort differences, with a 1e-6 numerical floor (coverage: at least one token). These are empirical sensitivity tolerances, not confidence intervals, semantic thresholds, or multiplicity-adjusted tests. Coverage floor = minimum calibration coverage minus calibrated coverage tolerance. Largest-share ceiling = observed maximum plus observed range, at least one token of headroom. Minimum communities = 2; reject all singletons. No study candidate or confirmation seed is observed.";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CalibrationReport {
    pub method: String,
    pub plan: CalibrationPlan,
    pub policy: SelectionPolicy,
    pub baseline_distributions: BTreeMap<String, Distribution>,
    pub cohort_absolute_differences: BTreeMap<SelectionMetric, Distribution>,
    pub input_sha256: String,
    pub metadata_sha256: Option<String>,
    pub baseline_graph_sha256: String,
    pub archived_assignments_sha256: String,
    pub code_hashes: BTreeMap<String, String>,
    pub frozen_manifest_without_reference_sha256: String,
}

fn median(values: impl IntoIterator<Item = f64>) -> f64 {
    distribution(values)
        .median
        .expect("validated nonempty cohort")
}

pub struct CalibrationEstimate {
    pub policy: SelectionPolicy,
    pub cohort_absolute_differences: BTreeMap<SelectionMetric, Distribution>,
    pub baseline_distributions: BTreeMap<String, Distribution>,
}

pub fn derive_policy(plan: &CalibrationPlan, baseline: &Candidate) -> Result<CalibrationEstimate> {
    plan.validate()?;
    ensure!(
        baseline.score.valid && baseline.runs.len() == plan.solver_seeds.len(),
        "Incomplete or invalid baseline calibration"
    );
    ensure!(
        baseline.runs.iter().map(|r| r.seed).collect::<Vec<_>>() == plan.solver_seeds,
        "Calibration solver seed mismatch"
    );
    for kind in ["edge_removal", "vocabulary_subsample"] {
        ensure!(
            baseline
                .perturbations
                .iter()
                .filter(|p| p.kind == kind && p.error.is_none() && p.agreement.is_some())
                .map(|p| p.seed)
                .collect::<Vec<_>>()
                == plan.perturbation_seeds,
            "Incomplete calibration perturbations: {kind}"
        );
    }
    let scalar_metrics = [
        SelectionMetric::Cohesion,
        SelectionMetric::Silhouette,
        SelectionMetric::Margin,
        SelectionMetric::NonsingletonCoverage,
    ];
    let mut values = BTreeMap::new();
    for metric in scalar_metrics {
        let rows = baseline
            .runs
            .iter()
            .map(|r| {
                let m = r.metrics.as_ref().context("Missing calibration metrics")?;
                let v = match metric {
                    SelectionMetric::Cohesion => m.cohesion.value,
                    SelectionMetric::Silhouette => m.silhouette.value,
                    SelectionMetric::Margin => m.margin.value,
                    _ => Some(m.nonsingleton_coverage),
                };
                v.context("Undefined baseline metric; cannot calibrate")
            })
            .collect::<Result<Vec<_>>>()?;
        values.insert(metric, rows);
    }
    let n = baseline.runs.len();
    let seed_index: BTreeMap<_, _> = baseline
        .runs
        .iter()
        .enumerate()
        .map(|(i, r)| (r.seed, i))
        .collect();
    let mut ari = vec![vec![None; n]; n];
    for pair in &baseline.repeats.pairs {
        let i = seed_index[&pair.left_seed];
        let j = seed_index[&pair.right_seed];
        ari[i][j] = Some(pair.agreement.ari);
        ari[j][i] = Some(pair.agreement.ari);
    }
    ensure!(
        (0..n).all(|i| (i + 1..n).all(|j| ari[i][j].is_some())),
        "Incomplete baseline repeat agreements"
    );
    let cohort_ari = |cohort: &[usize]| {
        median(cohort.iter().enumerate().flat_map(|(i, &a)| {
            cohort[i + 1..].iter().map({
                let ari = &ari;
                move |&b| ari[a][b].unwrap()
            })
        }))
    };
    for (metric, kind) in [
        (SelectionMetric::EdgeRemovalAri, "edge_removal"),
        (SelectionMetric::SubsampleAri, "vocabulary_subsample"),
    ] {
        values.insert(
            metric,
            baseline
                .perturbations
                .iter()
                .filter(|p| p.kind == kind)
                .map(|p| p.agreement.as_ref().unwrap().ari)
                .collect(),
        );
    }
    let mut differences: BTreeMap<_, Vec<f64>> = values
        .keys()
        .copied()
        .chain([SelectionMetric::RepeatAri])
        .map(|m| (m, vec![]))
        .collect();
    let mut solver_indices: Vec<_> = (0..n).collect();
    let mut perturbation_indices: Vec<_> = (0..plan.perturbation_seeds.len()).collect();
    let mut rng = Xoshiro256PlusPlus::seed_from_u64(plan.resampling_seed);
    let s = plan.study.screening_seeds.len();
    let p = plan.study.development_perturbation_seeds.len();
    for _ in 0..plan.resamples {
        solver_indices.shuffle(&mut rng);
        perturbation_indices.shuffle(&mut rng);
        for metric in scalar_metrics {
            let x = &values[&metric];
            differences.get_mut(&metric).unwrap().push(
                (median(solver_indices[..s].iter().map(|&i| x[i]))
                    - median(solver_indices[s..2 * s].iter().map(|&i| x[i])))
                .abs(),
            );
        }
        differences
            .get_mut(&SelectionMetric::RepeatAri)
            .unwrap()
            .push((cohort_ari(&solver_indices[..s]) - cohort_ari(&solver_indices[s..2 * s])).abs());
        for metric in [
            SelectionMetric::EdgeRemovalAri,
            SelectionMetric::SubsampleAri,
        ] {
            let x = &values[&metric];
            differences.get_mut(&metric).unwrap().push(
                (median(perturbation_indices[..p].iter().map(|&i| x[i]))
                    - median(perturbation_indices[p..2 * p].iter().map(|&i| x[i])))
                .abs(),
            );
        }
    }
    let differences: BTreeMap<_, _> = differences
        .into_iter()
        .map(|(m, x)| (m, distribution(x)))
        .collect();
    let token = 1.0 / baseline.runs[0].metrics.as_ref().unwrap().tokens as f64;
    let tolerances: Vec<_> = differences
        .iter()
        .map(|(&metric, d)| MetricTolerance {
            metric,
            absolute_tolerance: d.q95.unwrap().max(
                if metric == SelectionMetric::NonsingletonCoverage {
                    token
                } else {
                    1e-6
                },
            ),
        })
        .collect();
    let coverage_tolerance = tolerances
        .iter()
        .find(|t| t.metric == SelectionMetric::NonsingletonCoverage)
        .unwrap()
        .absolute_tolerance;
    let largest = distribution(
        baseline
            .runs
            .iter()
            .map(|r| r.metrics.as_ref().unwrap().largest_community_share),
    );
    let policy = SelectionPolicy {
        metrics: tolerances,
        minimum_nonsingleton_coverage: (values[&SelectionMetric::NonsingletonCoverage]
            .iter()
            .copied()
            .fold(1.0, f64::min)
            - coverage_tolerance)
            .max(0.0),
        minimum_communities: 2,
        maximum_largest_community_share: (largest.max.unwrap()
            + (largest.max.unwrap() - largest.min.unwrap()).max(token))
        .min(1.0),
        reject_all_singletons: true,
    };
    policy.validate()?;
    let mut distributions: BTreeMap<_, _> = values
        .into_iter()
        .map(|(m, x)| {
            (
                serde_json::to_value(m)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_string(),
                distribution(x),
            )
        })
        .collect();
    distributions.insert(
        "repeat_ari_dependent_pairs".into(),
        baseline.repeats.ari.clone(),
    );
    distributions.insert("largest_community_share".into(), largest);
    distributions.insert(
        "communities".into(),
        distribution(
            baseline
                .runs
                .iter()
                .map(|r| r.metrics.as_ref().unwrap().communities as f64),
        ),
    );
    distributions.insert(
        "solver_seconds".into(),
        distribution(baseline.runs.iter().map(|r| r.solver_seconds)),
    );
    Ok(CalibrationEstimate {
        policy,
        cohort_absolute_differences: differences,
        baseline_distributions: distributions,
    })
}
