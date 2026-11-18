# Leiden studies with source IDs

`prepare_dataset` and `leiden_study` are Rust executables for a changed vocabulary
whose desired cluster granularity is illustrated by an existing assignment. They
use the existing v2 seeded CPM solver and source-space evaluation. The older
experiment runner and its baseline-improvement selection policy remain available.

## Prepare and retain IDs

```sh
cargo build --release --locked --bins
target/release/prepare_dataset prepare \
  --dataset data/updated-dataset-2026.csv \
  --source data/download/merge_sgns_bigram_char300.txt.bz2 \
  --output data/updated-2026-prepared
```

The CSV requires `ID,Tilelabels`. IDs are nonempty unique strings, retained
exactly, including their original order. Repeated spellings share a single graph
node and vector. No sense distinction is inferred from the IDs. Words are matched
exactly against the full source archive; no character average, spelling rewrite,
or replacement vector is synthesized. A plain word2vec source also works.
Bzip2 input requires the system `bzip2` executable; there are no new Cargo dependencies.

Preparation writes `embeddings.txt`, `entries.csv`, `missing.csv`, and
`preparation.json`. The report records input/source/output SHA-256 hashes and both
unique-spelling and ID-row coverage. Selected vectors must have the declared
dimension, finite coordinates and nonzero norm. The entire source row count and
decompressor exit status are checked. Irrelevant whitespace-only vocabulary rows
in the original archive are counted without treating them as requested words.
Preparation requires a fresh directory and validates the source before publishing.

## Screen and confirm

```sh
target/release/leiden_study screen \
  --input data/updated-2026-prepared/embeddings.txt \
  --reference results/leiden-v2/ablations-2026-09-26/confirmation/confirmation/b58dfe04de33ba8c519a49ab8641c0cfd99a5c4022b6a198e1ffe7db1adade67/communities.csv \
  --output results/leiden-v2/ablations-2026-11-18/screening

target/release/leiden_study confirm \
  --screening results/leiden-v2/ablations-2026-11-18/screening \
  --output results/leiden-v2/ablations-2026-11-18/confirmation
```

Run from the repository root. Output directories must be new. The protocol is
written before fitting and freezes the graph grid, seed cohorts, size-profile
score and selection guardrails. Screening covers 13 graphs and five resolutions
each, using seeds 42–44:

- Raw cosine threshold graphs at .35, .40 and .45; q=.15,.225,.3,.4,.55.
- Raw union cosine k=10,20,40; centered, ABTT(1), ABTT(3), mutual, SNN,
  local and cosine/SNN blend variants at k=10; q=.03,.06,.1,.2,.3.

Threshold graphs use every positive raw cosine at or above the threshold. They
are computed in parallel without a dense similarity matrix. The linked reference
used an archived shifted-weight threshold of .70, equivalent to raw cosine .40,
with unshifted weights and q=.3. Each new graph uses resolution `q*w50` and theta
`.3*w50`, where w50 is its median edge weight. Objective values are comparable
only on identical graphs and resolutions.

The score combines token-weighted and group-weighted size-band total variation,
and the absolute log ratio of mean group sizes. It compares distributions rather
than old memberships or absolute community counts. Every screening seed must
retain at least 95% nonsingleton coverage, have no group above 4% of the
vocabulary, and contain connected, nondegenerate communities. The representative
maximizes mean repeat ARI, with smaller seeds breaking ties. Scores use that
representative; reports also show seed variation. This is a practical preference
score, not a significance test or a semantic quality score.

The frozen shortlist includes the transferred .40/q=.3 control and the best
eligible threshold, raw-neighbor, and transformed-neighbor settings (deduplicated).
Confirmation verifies screening/input/reference/source hashes and cached graphs,
then uses solver seeds 1000–1009 and paired perturbation seeds 20000–20009. It
never retunes q, resolution, or theta. Each perturbation compares with its own
same-seed full-graph fit: remove 5% of edges, or retain 90% of vocabulary. Vocabulary
subsampling refits transforms and rebuilds kNN; fixed raw threshold graphs induce
exactly. Every representative also has 100 label-permutation cohesion controls.

A finalist must pass the size guardrails and median repeat, edge-removal, and
subsample ARIs of at least .65. The recommendation is the qualifying finalist
with the lowest **development** score; confirmation cannot promote a setting by
its newly observed score. If none qualifies, no recommended assignment is
invented. The .65/.95/.04 cutoffs are explicit practical screening choices, not
recalibrated statistical thresholds from the older same-vocabulary study.

Screening records failed settings in `attempts.json` and continues; incomplete
settings cannot enter the shortlist. Confirmation fails on incomplete required
fits. `status.json` records command completion or failure. Search is bounded to
the explicit grid; endpoint optima remain visible without claiming exhaustive
optimization. There is no automatic legacy-default replacement and no external
semantic benchmark.

## Export community assignments for applications

```sh
target/release/prepare_dataset export \
  --prepared data/updated-2026-prepared \
  --assignments results/leiden-v2/ablations-2026-11-18/confirmation/communities.csv \
  --output results/leiden-v2/ablations-2026-11-18/recommended.communities.csv

python3 scripts/report_updated_leiden.py \
  results/leiden-v2/ablations-2026-11-18 --selected NAME_FROM_RECOMMENDATION_JSON

python3 scripts/audit_updated_leiden.py results/leiden-v2/ablations-2026-11-18
```

The application CSV has `ID,token,community,status`. Every original ID appears
once. Assigned rows use `assigned`; rows without an exact embedding use
`missing_embedding` and a blank community. Internal assignments remain the
compatible `token,community` format. Export rejects changed prepared mappings,
unknown/duplicate assignments, missing matched tokens, and existing output files.
Community numbers are local to a clustering run.

The reporting script uses only Python's standard library. It summarizes all
settings, confirmation, group-size bands, unique-spelling versus ID-expanded
sizes, and full member lists. The actual graph nodes are unique spellings;
ID-expanded counts additionally count repeated source spellings. Large cached
graphs, per-seed and stress files remain local under the existing ignore rules;
protocols, summaries, representative assignments and reports are retained.
The retained summaries are `screening-summary.csv` and `confirmation-summary.csv`.
Full per-seed `screening/summary.json`, `confirmation/summary.json`, and console
logs stay local. After a fresh clone, rerun screening and confirmation to regenerate
these detailed inputs before rerunning the report or full assignment audit.
Confirmation itself also requires the regenerated screening summary and graphs.
The audit checks every saved fit against its vocabulary and aggregate metrics,
the frozen settings and checksums, and exact source-ID coverage and ordering.
It writes `assignment-audit.json` and exits unsuccessfully if an invariant fails.
