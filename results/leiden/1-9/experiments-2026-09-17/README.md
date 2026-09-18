# HSK 1–9 experiment archive

Start with [REPORT.md](REPORT.md). This directory contains 31 successful new
Leiden runs over all 10,936 words, a nine-setting raw-vector grid, six valid PCA
screening runs, and five repeats for each of four finalists.

This archive moved from `graphs/1-9-leiden/` to `results/leiden/1-9/` on
2026-09-18. Recorded commands, provenance, logs and numerical results retain
their original contents and paths. The three Python helpers and reproduction
instructions now resolve the new location; `summarize.py` translates the old
graph paths when reading historical run records. `checksums.sha256` was refreshed
only for this README and those three modified helpers. Historical application
source and build hashes still describe the original experiment.

## Files

| File / directory | Purpose |
|---|---|
| `recommended.communities.csv` | Copy of the selected representative partition; baseline outputs were not replaced. |
| `recommended-clusters.csv` | All 1,372 selected communities, largest first, with representative/sample/boundary words, full membership and stability. |
| `recommended-word-details.csv` | Every selected word with HSK level(s), pinyin, original dictionary translations and diagnostics. |
| `recommendation.json` | Exact selected run, parameters, command and hashes. |
| `run-summary.csv` | All 31 runs and their diagnostics. |
| `config-summary.csv` | Means and ranges per parameter setting. |
| `graph-summary.csv` | Edges, degrees, components and graph isolates for the five clustering graphs. |
| `stability-summary.csv`, `stability-pairs.csv` | Four five-run comparisons; ten ARI comparisons per configuration. |
| `anchor-review.csv` | Fixed set of 18 anchor words across all runs. |
| `level-coverage.csv` | Coverage by earliest HSK band of each spelling; explicit `|` spelling alternatives are resolved. |
| `runs/<name>/` | Assignments, stdout/stderr log, command/settings/hash/timing record, cluster and word diagnostics. |
| `baseline/` | Diagnostics for the original saved 508-community baseline. |
| `graphs/` | Derived raw threshold graphs and frozen PCA graphs. The raw 0.70 graph remains in the parent directory. |
| `pca100/`, `pca200-fullfit/` | Valid PCA models, normalized transformed embeddings, variance and eigen-residual audits. |
| `pca100-fullfit/` | Full-fit cross-check for 100D; no additional clustering runs. |
| `pca200/` | Rejected direct 200D fit, retained only to document the numerical finding. **Do not use this graph as a PCA result.** |
| `provenance.json`, `checksums.sha256` | Input/build provenance and archive file hashes. |

`representatives` rank members by mean cosine to their own cluster; `boundary_words`
rank by the lowest margin to the best alternative cluster. `deterministic_sample`
uses a fixed FNV hash of the spelling. These are selection rules, not semantic
labels. The catalogue's `mean_best_match_jaccard` matches each reference community
to its most similar community in each other repeat. Word repeat agreement uses
those same matches. It is not a calibrated membership probability.

Word translations come from the input vocabulary CSV. Two empty primary
translations (`森林`, `表示`) use that same row's CC-CEDICT column; the exported
`dictionary_translation_source` identifies the source column.

The default export is `raw-t0.7-r0.2-run05`, the run with highest mean ARI to its
other four repeats. The other representative partitions are identified in
`stability-summary.csv`; no community number should be compared directly between
different runs.

## Reproduction

Run from the repository root. The existing release binaries were rebuilt with
`cargo build --release --locked --bins --lib`; application and dependency sources
were not edited. Rust version and hashes are recorded in `provenance.json`.

The helper was compiled against the existing release dependencies:

```sh
rustc --edition=2021 -O \
  results/leiden/1-9/experiments-2026-09-17/experiment_tools.rs \
  -L dependency=target/release/deps \
  --extern hskclustering=target/release/libhskclustering.rlib \
  --extern csv=target/release/deps/libcsv-ffe1374c19c431bd.rlib \
  --extern linfa=target/release/deps/liblinfa-ff47695b1f039c29.rlib \
  --extern linfa_reduction=target/release/deps/liblinfa_reduction-1f1042b8827176ad.rlib \
  --extern ndarray=target/release/deps/libndarray-50866da5632b53bb.rlib \
  -o results/leiden/1-9/experiments-2026-09-17/experiment_tools
```

Those dependency fingerprints describe this build; a different toolchain/build
may generate different `.rlib` filenames. The helper uses Linfa PCA and the
repository's normalization/graph conventions; it is not an application change.

The raw screening grid is:

```sh
python3 -B results/leiden/1-9/experiments-2026-09-17/run_experiments.py
```

This filters the saved raw 0.70 graph to 0.725 and 0.75, then calls the current
`leiden_community` binary at resolutions 0.05, 0.10 and 0.20. Every run uses CPM,
theta 0.30 and gamma equal to resolution. Each run's exact command is saved in
its `run.json`. Complete existing run records are skipped; use new repeat numbers
or a fresh copy of the experiment directory to obtain additional random runs.

Valid PCA generation and numerical checks were equivalent to:

```sh
results/leiden/1-9/experiments-2026-09-17/experiment_tools pca \
  data/input-embeddings.txt 100 results/leiden/1-9/experiments-2026-09-17/pca100 127234
results/leiden/1-9/experiments-2026-09-17/experiment_tools pca \
  data/input-embeddings.txt 200 results/leiden/1-9/experiments-2026-09-17/pca200-fullfit 127234 full
results/leiden/1-9/experiments-2026-09-17/experiment_tools audit-pca \
  data/input-embeddings.txt \
  results/leiden/1-9/experiments-2026-09-17/pca200-fullfit/pca-components.csv \
  results/leiden/1-9/experiments-2026-09-17/pca200-fullfit/audit.json
```

The target is 127,234 edges. The helper scans all projected similarities and
chooses the closest threshold on a 0.001 grid, then uses the current graph
builder. It returned 0.737 for 100D and 0.686 for the valid 200D projection.
PCA is centred on raw SGNS coordinates, without whitening or coordinate scaling,
followed by L2 normalization. Frozen means, bases, vectors and graphs are saved.

```sh
python3 -B results/leiden/1-9/experiments-2026-09-17/run_experiments.py \
  --embedding pca100 --thresholds 0.737
python3 -B results/leiden/1-9/experiments-2026-09-17/run_experiments.py \
  --embedding pca200-fullfit --thresholds 0.686
```

Runs 02–05 were added for raw `(threshold, resolution)` values `(0.70, 0.20)`,
`(0.725, 0.10)`, `(0.725, 0.20)` and PCA200 `(0.686, 0.10)`, for example:

```sh
python3 -B results/leiden/1-9/experiments-2026-09-17/run_experiments.py \
  --thresholds 0.7 --resolutions 0.2 --start-repeat 2 --repeats 4
python3 -B results/leiden/1-9/experiments-2026-09-17/summarize.py
python3 -B results/leiden/1-9/experiments-2026-09-17/export_review.py \
  --recommend raw-t0.7-r0.2-run05
```

The solver does not expose a Leiden seed, so a fresh execution reproduces the
procedure, not necessarily identical assignments. Up to seven independent jobs
ran concurrently during this experiment; elapsed times are not clean performance
benchmarks. No new dependencies were installed.

Every assignment set and every community's graph connectivity were verified.
All similarity diagnostics use the original normalized 300D vectors, including
the PCA partitions. Eigen residuals validate the PCA directions independently of
their orthogonality. See the report for interpretation and limitations.
