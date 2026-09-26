# Focused k=10 graph ablations

This follows the [raw core study](../study-2026-09-25/COMPARISON.md).
The six graphs are raw cosine and local union controls, then cosine with
centering, ABTT(1), ABTT(3), or mutual connectivity, all with k=10.
Only one factor changes at a time; each graph's resolution is retuned.

The primary experimental reference is raw cosine k=10, q=0.1. The archived
baseline and unshifted threshold-0.70 q=0.3 control remain separate references.
The existing seven-metric acceptance rule and baseline-calibrated tolerances
are preserved; solver algorithms and embeddings are unchanged.

**Outcome:** keep ABTT(1), cosine union k=10, q=0.1 as the supported experimental
improvement over raw cosine. Fresh edge-removal ARI rises from 0.7489 to 0.7694,
with no other metric regressing beyond the frozen tolerance. Its median largest
group is slightly larger (51.5 versus 46.5 words). Centering has small positive
changes that do not clear the confirmation threshold; ABTT(3) loses coherence,
and mutual pruning loses coverage and margin. Local weighting remains comparable
to raw cosine. The archived default files are unchanged.

The ready-to-inspect option is [recommended-experimental.json](recommended-experimental.json)
with [experimental.communities.csv](experimental.communities.csv), copied from
the confirmed ABTT(1) representative (seed 1000). Its source candidate, assignment
checksum and decision checksum are recorded. The same 18-anchor review includes
both improvements (the school group around 老师) and regressions (政策).

The study completed 46 screening settings and eight frozen confirmation settings.
Screening took 27 minutes; its assignment audit checked 1,622 fit files, and the
confirmation audit checked 348. Rust tests (46), formatting, lint, and a separate
end-to-end synthetic study/report check passed. See engineering-checks.json.

Read [COMPARISON.md](COMPARISON.md), [decisions.json](decisions.json), and
[LEARNING_REVIEW.md](LEARNING_REVIEW.md) for completed outcomes. The protocol
distinguishes keeping an experimental improvement from qualifying a replacement
for the archived default. Mixed trade-offs do not automatically become winners.

## Reproduce

Run from the repository root with the recorded source and Cargo.lock. Every fit
output directory must be new. To reproduce under another study directory, copy
the calibration plan and analysis protocol there and update the commands below.
The calibration runner writes its new evidence paths into the resulting manifest.

```sh
cargo build --release --locked --bin graph_v2
target/release/graph_v2 calibrate \
  --plan results/leiden-v2/ablations-2026-09-26/calibration-plan.json \
  --output results/leiden-v2/ablations-2026-09-26/calibration
target/release/graph_v2 validate-manifest \
  --manifest results/leiden-v2/ablations-2026-09-26/calibration/screening-manifest.json
python3 scripts/report_graph_v2.py results/leiden-v2/ablations-2026-09-26 --seal
target/release/graph_v2 experiment \
  --manifest results/leiden-v2/ablations-2026-09-26/calibration/screening-manifest.json \
  --output results/leiden-v2/ablations-2026-09-26/screening
python3 scripts/report_graph_v2_ablations.py results/leiden-v2/ablations-2026-09-26 --freeze
target/release/graph_v2 confirm \
  --plan results/leiden-v2/ablations-2026-09-26/confirmation-plan.json \
  --output results/leiden-v2/ablations-2026-09-26/confirmation
python3 scripts/report_graph_v2_ablations.py results/leiden-v2/ablations-2026-09-26
```

The report scripts use the Python standard library. The freeze step audits the
screening assignments, reproduces the shortlist rule, records hashes of selected
settings, and exports the development review. The final step verifies the frozen
subset and confirmation assignments, exports comparison tables and review panels,
and records keep/discard decisions. LEARNING_REVIEW.md is a separate desk review
of those panels, not generated semantic ratings or learner feedback.

To inspect only the supported representation, using fresh directories:

```sh
target/release/graph_v2 build --input data/input-embeddings.txt \
  --k 10 --weight cosine --representation abtt --components 1 \
  --output results/leiden-v2/abtt1-k10-inspect
target/release/graph_v2 cluster --input data/input-embeddings.txt \
  --graph results/leiden-v2/abtt1-k10-inspect/graph.json \
  --q 0.1 --seed 1000 --output results/leiden-v2/abtt1-k10-inspect-seed1000
```

This reproduces the selected representative on the recorded build. For a full
study reproduction, the supported assignment is the `representative_assignments`
path in the confirmation candidate whose ID is marked `keep experimental improvement`
in decisions.json; the friendly experimental export is a byte-for-byte copy.

## Design

- `analysis-protocol.json` fixes scope, shortlist, comparisons, decisions, and
  the same 18 review anchors as the previous study before observing new fits.
- Calibration repeats the archived baseline with solver seeds 500–529,
  perturbation seeds 40000–40019, and cohort resampling seed 60000. It verifies
  the new build and freezes the unchanged policy for this six-graph design.
- Screening starts with q=0.01, 0.03, 0.1, 0.3 and up to two factor-three endpoint
  expansions per graph. It uses solver seeds 42–44 and perturbations 10000–10009.
- Confirmation uses the reserved solver seeds 1000–1009 and perturbations
  20000–20009 only after the subset and exact settings are frozen. No retuning
  follows confirmation. Null diagnostics use development seeds 30000–30099.
- Each ablation is compared both at its shortlisted q and at the tested q
  closest to the raw reference's community count. Count matching is diagnostic;
  a match is only labeled successful within 10% relative difference.
- The largest cluster, coverage, fractions of words in groups of 3–30 and over
  100, source-space coherence, repeat agreement, edge removal, and vocabulary
  subsampling remain visible. Vocabulary subsampling refits transforms and
  reconstructs neighbors.

Large graphs, directed-neighbor tables, per-word metrics, full candidate and
confirmation JSON, sampled graph inputs, and individual seed/stress/sensitivity
fits remain local under the ignore rules. Settings, summary measurements,
representative assignments, checksums, and reports are retained for the commit.
A full reproduction regenerates ignored artifacts needed for a new audit or
confirmation run. Paths in saved records resolve from the repository root.
