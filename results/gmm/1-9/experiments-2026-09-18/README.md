# GMM staged experiments — 2026-09-18

Objective: explore semantic relevance in the HSK 1–9 vocabulary. No preferred
component count, cluster size, granularity, or one-to-one relationship between
mixture components and semantic groups is assumed. Multiple related or distinct
semantic groups may occur within a component; soft membership is retained.

This experiment evaluates GMM on its own. The existing archive was consulted
only for CSV column names. No other method's assignments, scores, or cluster
structure are used for selection or evaluation.

## Stages

1. Screen PCA dimensions 20, 30, 50 and component counts 10, 25, 50, 100, 200,
   without whitening; covariance regularization 1e-5; seed 42. This is a broad
   exploratory grid, not a statement of desired granularity.
2. Inspect the screening results and record the next stage before running it.
   Probe regularization, whitening, and additional retained dimensions where
   the observations justify them.
3. Repeat a diverse shortlist with independent seeds. Agreement diagnoses
   initialization sensitivity; it does not measure semantic correctness.

Record all successful and failed fits. AIC/BIC are compared only among models
with identical input and PCA/whitening representation. Neither AIC/BIC nor
cosine geometry determines semantic relevance. Size diagnostics describe
concentration, especially a dominant component together with a tiny tail;
balanced sizes and small groups are not objectives.

Cluster review should include central members, deterministic samples, boundary
members, multiple themes, and soft-membership alternatives. Membership weights
are responsibilities under the fitted model, not calibrated semantic confidence.

## Reproduce

From the repository root, build the committed implementation:

```sh
cargo build --release --locked --bin gmm_cluster --example gmm_review
python3 results/gmm/1-9/experiments-2026-09-18/run_experiments.py stage1-screen.json
python3 results/gmm/1-9/experiments-2026-09-18/run_experiments.py stage2-sensitivity.json
python3 results/gmm/1-9/experiments-2026-09-18/run_experiments.py stage2b-strong-regularization.json
python3 results/gmm/1-9/experiments-2026-09-18/run_experiments.py stage3-repeats.json
python3 results/gmm/1-9/experiments-2026-09-18/summarize.py
python3 results/gmm/1-9/experiments-2026-09-18/export_shortlist.py
python3 results/gmm/1-9/experiments-2026-09-18/validate.py
```

The runner skips completed jobs, preserves failed jobs, records each exact
command and enforces identical input, dependency lockfile, binary and source
commit across stages. Use a new experiment directory to repeat from scratch.
Every job contains one fit, so its full probability/model exports are retained
locally for subsequent review, independently of the AIC/BIC rankings.

`provenance.json` records input, dictionary, binary and dependency hashes.
`jobs/` stores execution metadata; `logs/` captures stdout/stderr; `runs/`
contains native GMM outputs. Full models, probability matrices, per-run PCA bases
and raw word geometry remain local and are ignored by Git. In this single-fit
archive, the named candidate assignment CSVs exactly duplicate each run's
`communities.csv`, so only the canonical assignment file is versioned. This
duplicate-file rule does not apply to ordinary multi-fit sweeps.

The saved settings reproduce PCA; `metrics.json` retains the variance summary
and `validation.json` retains each representation's PCA hash. Reports, component
catalogues, dictionary-enriched review tables, settings, assignments and validation
records remain versioned. `checksums.sha256` covers the versioned archive and
review source, excluding ignored local artifacts. Full numerical validation
still requires the local or regenerated model, probability and PCA files.

## Review files

- [Report](REPORT.md): scope, staged decisions, observed concentration behaviour,
  semantic spot-checks and remaining questions.
- [All measurements](run-summary.csv): one row per attempted fit. Scores from
  different PCA/whitening representations must not be ranked together.
- [Review index](review-index.csv): eight deliberately different review candidates
  and links to their catalogues, word details and local probability matrices.
  Seed 42 is retained consistently for browsing; no favourable repeat replaces it.
- [Qualitative observations](linguistic-observations.csv): fourteen authored
  spot-checks, with explicit evidence and limits. These are not accuracy labels.
- [Fixed-anchor review](anchor-review.csv): the same available anchor words across
  all successful fits. Missing anchors are omitted, not assigned invented vectors.
- [Broader review sample](linguistic-review-sample.csv): largest, smallest,
  lowest-cohesion and three hash-selected components per fit. Duplicate choices
  are merged and their selection reasons retained. Its review fields start blank.
- [Ambiguous words](ambiguous-words.csv): the 30 smallest top-two responsibility
  margins per review candidate, with both components' representatives.
- [Repeat summary](stability-summary.csv) and [pairwise results](stability-pairs.csv):
  agreement only between seeds of the **same GMM configuration**.
- [Validation](validation.json), [independent geometry check](independent-geometry-check.json)
  and [checksums](checksums.sha256): numerical/artifact verification.

Every successful run also contains `clusters.csv`, `metrics.json`, and
`review-validation.json`. The catalogue includes every mixture component,
including components with zero hard assignments, and preserves all hard members.
Representatives are ranked by mean original-space cosine to the other members.
Boundary words have the smallest own-versus-best-other cosine margin. Samples
use the SHA-256 ordering of token text. Outside-hard-cluster alternatives are the
largest responsibilities at least 0.01; that threshold is only a display filter.

Review candidates additionally contain `review-clusters.csv`,
`cluster-stability.csv` and `word-details.csv`. Best-match Jaccard matches each
seed-42 component independently to the most overlapping component in each of
the other four seeds; many-to-one matches are allowed. Word repeat fractions
describe membership in these matches. These are computational repeatability
diagnostics, not probabilities of semantic relevance or matches of semantic groups.

`words.csv` is a large regenerable per-token geometry export and is ignored by
Git. `word-details.csv` adds the source dictionary's pinyin, available meanings
and all HSK levels, and remains part of the review archive. Dictionary senses
are context for inspection, not ground-truth cluster labels.

The core GMM implementation was unchanged during the experiment. The fitting
commit is recorded in `provenance.json`; the new `examples/gmm_review.rs` and
archive scripts supply diagnostics. To reproduce from scratch, use that fitting
revision with these saved review tools and a fresh archive directory of the same
depth. Fit durations exclude CSV/model export; job durations include those exports.
