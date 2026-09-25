use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectionMetric {
    Cohesion,
    Silhouette,
    Margin,
    RepeatAri,
    EdgeRemovalAri,
    SubsampleAri,
    NonsingletonCoverage,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetricTolerance {
    pub metric: SelectionMetric,
    pub absolute_tolerance: f64,
}

/// Deliberately no Default: accepting a candidate requires a frozen explicit policy.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectionPolicy {
    pub metrics: Vec<MetricTolerance>,
    pub minimum_nonsingleton_coverage: f64,
    pub minimum_communities: usize,
    pub maximum_largest_community_share: f64,
    pub reject_all_singletons: bool,
}
impl SelectionPolicy {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.metrics.is_empty(),
            "Selection metric panel must be explicit and nonempty"
        );
        let mut seen = BTreeSet::new();
        for m in &self.metrics {
            ensure!(seen.insert(m.metric), "Duplicate selection metric");
            ensure!(
                m.absolute_tolerance.is_finite() && m.absolute_tolerance >= 0.0,
                "Tolerances must be finite and nonnegative"
            );
        }
        ensure!(
            (0.0..=1.0).contains(&self.minimum_nonsingleton_coverage),
            "Coverage limit must be in [0,1]"
        );
        ensure!(
            self.minimum_communities > 0,
            "Minimum community count must be explicit and positive"
        );
        ensure!(
            self.maximum_largest_community_share.is_finite()
                && self.maximum_largest_community_share > 0.0
                && self.maximum_largest_community_share <= 1.0,
            "Concentration limit must be in (0,1]"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CandidateScore {
    pub id: String,
    pub graph_id: String,
    pub baseline: bool,
    pub metrics: BTreeMap<SelectionMetric, Option<f64>>,
    pub minimum_communities: usize,
    pub maximum_largest_share: f64,
    pub minimum_coverage: f64,
    pub valid: bool,
    pub error: Option<String>,
    pub transforms: usize,
    pub runtime_seconds: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SelectionReport {
    pub policy: Option<SelectionPolicy>,
    pub pareto: Vec<String>,
    pub nominee: Option<String>,
    pub rejected: BTreeMap<String, String>,
    pub reason: String,
}

const PARETO_PANEL: [SelectionMetric; 7] = [
    SelectionMetric::Cohesion,
    SelectionMetric::Silhouette,
    SelectionMetric::Margin,
    SelectionMetric::RepeatAri,
    SelectionMetric::EdgeRemovalAri,
    SelectionMetric::SubsampleAri,
    SelectionMetric::NonsingletonCoverage,
];

pub fn pareto(candidates: &[CandidateScore]) -> Vec<String> {
    candidates
        .iter()
        .filter(|a| {
            a.valid
                && (a.baseline
                    || (PARETO_PANEL
                        .iter()
                        .all(|m| a.metrics.get(m).copied().flatten().is_some())
                        && !candidates.iter().any(|b| {
                            if !b.valid || b.id == a.id {
                                return false;
                            }
                            let mut better = false;
                            for metric in PARETO_PANEL {
                                match (
                                    a.metrics.get(&metric).copied().flatten(),
                                    b.metrics.get(&metric).copied().flatten(),
                                ) {
                                    (Some(x), Some(y)) => {
                                        if y < x {
                                            return false;
                                        }
                                        better |= y > x;
                                    }
                                    // Missing measurements cannot establish domination.
                                    _ => return false,
                                }
                            }
                            better
                        })))
        })
        .map(|a| a.id.clone())
        .collect()
}

fn eligibility(candidate: &CandidateScore, policy: &SelectionPolicy) -> Option<String> {
    if !candidate.valid {
        return Some(
            candidate
                .error
                .clone()
                .unwrap_or_else(|| "Invalid candidate".into()),
        );
    }
    if candidate.minimum_communities < policy.minimum_communities {
        return Some("Community count below explicit degeneracy limit".into());
    }
    if candidate.maximum_largest_share > policy.maximum_largest_community_share {
        return Some("Largest-community share exceeds explicit limit".into());
    }
    if candidate.minimum_coverage < policy.minimum_nonsingleton_coverage {
        return Some("Nonsingleton coverage below explicit limit".into());
    }
    if policy.reject_all_singletons && candidate.minimum_coverage == 0.0 {
        return Some("All-singleton partition".into());
    }
    for metric in &policy.metrics {
        if candidate
            .metrics
            .get(&metric.metric)
            .copied()
            .flatten()
            .is_none()
        {
            return Some(format!("Required metric {:?} unavailable", metric.metric));
        }
    }
    None
}

pub fn improves(
    candidate: &CandidateScore,
    baseline: &CandidateScore,
    policy: &SelectionPolicy,
) -> bool {
    let mut improves = false;
    for metric in &policy.metrics {
        let (Some(Some(a)), Some(Some(b))) = (
            candidate.metrics.get(&metric.metric),
            baseline.metrics.get(&metric.metric),
        ) else {
            return false;
        };
        if a < &(b - metric.absolute_tolerance) {
            return false;
        }
        improves |= a > &(b + metric.absolute_tolerance);
    }
    improves
}

pub fn select(
    candidates: &[CandidateScore],
    policy: Option<&SelectionPolicy>,
) -> Result<SelectionReport> {
    let mut report = SelectionReport {
        policy: policy.cloned(),
        pareto: pareto(candidates),
        nominee: None,
        rejected: BTreeMap::new(),
        reason: String::new(),
    };
    let Some(policy) = policy else {
        report.reason = "No explicit selection policy: evaluation only; baseline retained".into();
        return Ok(report);
    };
    policy.validate()?;
    let Some(baseline) = candidates.iter().find(|c| c.baseline) else {
        report.reason = "Baseline unavailable; automatic nomination disabled".into();
        return Ok(report);
    };
    if !baseline.valid
        || policy
            .metrics
            .iter()
            .any(|m| baseline.metrics.get(&m.metric).copied().flatten().is_none())
    {
        report.reason = "Baseline lacks valid measurements for the frozen metric panel".into();
        return Ok(report);
    }
    let mut eligible = vec![];
    for candidate in candidates.iter().filter(|c| !c.baseline) {
        if let Some(reason) = eligibility(candidate, policy) {
            report.rejected.insert(candidate.id.clone(), reason);
            continue;
        }
        if !improves(candidate, baseline, policy) {
            report.rejected.insert(
                candidate.id.clone(),
                "Does not improve the baseline within the frozen tolerances".into(),
            );
            continue;
        }
        if report.pareto.contains(&candidate.id) {
            eligible.push(candidate);
        }
    }
    if eligible.is_empty() {
        report.reason =
            "No eligible Pareto candidate improves the baseline; baseline retained".into();
        return Ok(report);
    }
    // Only use simplicity/runtime to break equivalent candidates, never hide trade-offs.
    for a in &eligible {
        for b in &eligible {
            for m in &policy.metrics {
                if (a.metrics[&m.metric].unwrap() - b.metrics[&m.metric].unwrap()).abs()
                    > m.absolute_tolerance
                {
                    report.reason="Several improvements have unresolved metric trade-offs; no automatic default".into();
                    return Ok(report);
                }
            }
        }
    }
    eligible.sort_by(|a, b| {
        a.transforms
            .cmp(&b.transforms)
            .then(a.runtime_seconds.total_cmp(&b.runtime_seconds))
            .then(a.id.cmp(&b.id))
    });
    report.nominee = Some(eligible[0].id.clone());
    report.reason = "Development nominee frozen; fresh-seed confirmation is still required".into();
    Ok(report)
}

pub fn confirmation_accepts(
    candidate: &CandidateScore,
    baseline: &CandidateScore,
    policy: &SelectionPolicy,
) -> bool {
    baseline.valid
        && eligibility(candidate, policy).is_none()
        && improves(candidate, baseline, policy)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn score(id: &str, base: bool, value: f64) -> CandidateScore {
        CandidateScore {
            id: id.into(),
            graph_id: id.into(),
            baseline: base,
            metrics: PARETO_PANEL.into_iter().map(|m| (m, Some(value))).collect(),
            minimum_communities: 2,
            maximum_largest_share: 0.5,
            minimum_coverage: 0.8,
            valid: true,
            error: None,
            transforms: 0,
            runtime_seconds: 1.0,
        }
    }
    #[test]
    fn no_policy_never_nominates_and_confirmation_cannot_retune() -> Result<()> {
        let candidates = vec![score("baseline", true, 0.5), score("new", false, 0.7)];
        assert!(select(&candidates, None)?.nominee.is_none());
        let policy = SelectionPolicy {
            metrics: vec![MetricTolerance {
                metric: SelectionMetric::Cohesion,
                absolute_tolerance: 0.01,
            }],
            minimum_nonsingleton_coverage: 0.5,
            minimum_communities: 2,
            maximum_largest_community_share: 0.9,
            reject_all_singletons: true,
        };
        assert_eq!(
            select(&candidates, Some(&policy))?.nominee,
            Some("new".into())
        );
        assert!(!confirmation_accepts(
            &score("new", false, 0.49),
            &candidates[0],
            &policy
        ));
        Ok(())
    }
}
