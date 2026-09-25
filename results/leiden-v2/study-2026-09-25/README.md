# Calibrated v2 core screening

Start with [COMPARISON.md](COMPARISON.md), [shortlist.json](shortlist.json),
and [LEARNING_REVIEW.md](LEARNING_REVIEW.md). All measurements use the 10,936
matched HSK 1–9 spellings in the unchanged 300D input embeddings.

This study stops after baseline calibration, archived controls and raw core
screening. It does not run centering, ABTT, mutual-neighbor ablations or
fresh-seed confirmation. The archived recommended partition remains the default.

## Reproduce

Run from the repository root with the recorded Rust source and lockfile.
All fit output directories must be new. To reproduce in another directory,
change both output paths and the corresponding manifest path below.

```sh
cargo run --release --locked --bin graph_v2 -- calibrate \
  --plan results/leiden-v2/study-2026-09-25/calibration-plan.json \
  --output results/leiden-v2/study-2026-09-25/calibration
cargo run --release --locked --bin graph_v2 -- validate-manifest \
  --manifest results/leiden-v2/study-2026-09-25/calibration/screening-manifest.json
python3 scripts/report_graph_v2.py results/leiden-v2/study-2026-09-25 --seal
cargo run --release --locked --bin graph_v2 -- experiment \
  --manifest results/leiden-v2/study-2026-09-25/calibration/screening-manifest.json \
  --output results/leiden-v2/study-2026-09-25/screening
python3 scripts/report_graph_v2.py results/leiden-v2/study-2026-09-25
```

The report generator needs Python 3.9+ and only the standard library. It
recomputes assignment coverage and sizes, checks the initial grid is complete,
checks every recorded attempt and reserved seed, reproduces the deterministic
shortlist, and exports fixed-anchor member lists. `LEARNING_REVIEW.md` is the
separate qualitative inspection of those outputs. For a reproduction directory,
copy `analysis-protocol.json` there before running the study/report.
The seal command requires a new file and refuses to run after screening starts.
The checked-in seal for this run additionally records that calibration stayed
numerically identical after the buffered-read correction.

## Design and files

- `calibration-plan.json`: scope and calibration/study seeds, fixed before fitting.
- `analysis-protocol.json`: shortlist rule and 18 review anchors, fixed before screening.
- `protocol-seal.json`: hashes and creation time for the analysis protocol and plan.
- `calibration/plan.json`, `protocol.json`: saved pre-fit protocol and build provenance.
- `calibration/calibration.json`: baseline distributions, cohort-difference
  distributions and derived selection policy; no core candidates informed these.
- `calibration/screening-manifest.json`: frozen study plus a hash of its calibration evidence.
- `calibration/archive-agreements.json`: archived partition versus each seeded baseline fit.
- `screening/`: complete experiment artifacts, graph construction, attempts,
  assignments, source-space geometry, stability, perturbations, sensitivity,
  null controls, strata and count-matched comparisons.
- `comparison.csv`: all candidate settings and their trade-offs, including expansions.
- `shortlist.json`: up to one guarded Pareto candidate per weight family, using
  the predeclared worst-regression/tie-breaking rule. Inclusion is provisional.
- `fixed-anchor-review.csv`, `FIXED_GROUPS.md`: full member lists and fixed samples,
  including weak boundary words, singletons and duplicate-group disclosures.
- `validation.json`: artifact audit; `engineering-checks.json`: code checks.
- `assignment-checksums.csv`: SHA-256 of each audited fit assignment.

Calibration uses solver seeds 500–529, paired perturbation seeds 40000–40019,
and cohort-splitting seed 60000. Screening uses solver seeds 42–44 and paired
perturbation seeds 10000–10009. Solver seeds 1000–1009 and perturbation seeds
20000–20009 remain reserved for confirmation. The 100 development null-label
permutations use 30000–30099. Repeated calibration null diagnostics share these
development null seeds; they do not enter calibration tolerances.

The initial pilot (`../calibration-2026-09-25-pilot/`) only timed one fit.
`rejected-calibration.json` records an initial calibration rejected by the
manifest readback check before any screening. It exposed a one-ULP JSON
deserialization issue; exact float parsing, a regression test and an explicit
readback check were added, then calibration was rerun from scratch. Rejected
large artifacts remain locally under `calibration-readback-rejected/`.

`interrupted-screening.json` records a subsequent performance correction, before
any core candidates were observed: cached graph JSON needed buffered reads.
One same-graph/same-seed invocation fell from 20.38s to 2.31s with byte-identical
assignments and identical metrics. The interrupted controls and earlier
calibration remain locally under `screening-unbuffered-interrupted/` and
`calibration-before-io-fix/`. The design and numerical calibration policy were
unchanged; calibration and screening were restarted under the final source hash.

Large graphs, neighbor/pair tables, per-word diagnostics, redundant aggregate
JSON/solver labels, and detailed stress/sensitivity fits are regenerable and
ignored by Git. All remain locally available. Settings, each candidate's full
summary record, primary seed/representative assignments, comparisons, fit
assignment checksums and reports remain tracked. Reproducing the full study
restores the ignored artifacts needed for a fresh audit. Paths inside run records resolve from the
repository root. Memory is process lifetime peak RSS; solver time is separated
from graph building and metric/export costs.
