# HSK vocabulary clustering

Group Chinese words with related contexts/meanings into communities for studying,
using Rust: graphrs for Leiden and Linfa for Gaussian mixture models (GMM).

Each method has a small, separate runner. Shared code covers embeddings,
assignment CSVs and comparison metrics; there is no clustering plugin framework.
Adding a third method means adding its runner and writing the same
`token,community` CSV. `community` means a cluster ID local to that particular run.

```text
results/
  leiden/
    1-3/                         # archived experiment
    1-9/                         # baseline and dated experiment archive
  gmm/
    1-9/<run>/                   # model selection, assignments, probabilities
```

The previous `graphs/1-9-leiden/` and `graphs/1-3-leiden/` directories moved to
`results/leiden/1-9/` and `results/leiden/1-3/`. Existing results are preserved.
Graphs are still valid artifacts within Leiden results, not a requirement of
other methods. The old estimate of about 72% distinguishable thematic groups
applies only to the HSK 1–3 archive.

## Input

Use `data/input-embeddings.txt`, prefiltered to the desired vocabulary. The Rust
tools use every token in this file; they do not infer or filter HSK levels.
The source used here is [Chinese Word Vectors](https://github.com/Embedding/Chinese-Word-Vectors),
mixed-large SGNS, 300 dimensions, word + character + ngram.

```text
<number_of_words> <embedding_dimension>
<word> <value_1> <value_2> ... <value_N>
<word> <value_1> <value_2> ... <value_N>
```

Token counts and dimensions must be exact. Tokens must be unique, values finite,
and vectors nonzero. Invalid input produces an error with its line/token context.
The current input covers all HSK 1–9 bands with 10,936 distinct spellings; 36 rows
in `data/hsk-data.csv` have no matched embedding. HSK entries and spellings are not
one-to-one, so these two counts do not sum to the CSV row count.

## Run Leiden

Run from the repository root with Rust 1.87 or newer. Exact dependency versions
are recorded in `Cargo.lock`; use `--locked` when reproducing a run.

```sh
# Complete graph: all input tokens and every pair of distinct tokens.
cargo run --release --locked --bin build_graph

# Stream the complete file and keep edges with weight >= 0.70.
cargo run --release --locked --bin filter_graph

# Inspect the thresholded graph; expensive metrics are opt-in.
cargo run --release --locked --bin observe_graph

# CPM, resolution 0.05, theta 0.3, gamma 0.05.
cargo run --release --locked --bin leiden_community
```

These defaults produce:

- `results/leiden/1-9/hsk1-9-full.graphml`
- `results/leiden/1-9/hsk1-9-thresh0.70.graphml`
- `results/leiden/1-9/hsk1-9-thresh0.70.communities.csv`

For subsequent runs that only need the clustering graph, the following computes
the same thresholded edges directly, without exporting the complete graph first:

```sh
cargo run --release --locked --bin build_graph -- --threshold 0.70
```

All Leiden commands accept `--input` and `--output`. Every node is retained when filtering,
including isolated words. Output directories are created as needed, and completed
files replace prior results atomically. The existing `--preprocess` flag still
whitens raw embeddings before final normalization; it is off for this baseline.
GMM has its own explicitly ordered preprocessing below.

## Run graph v2

The separate `graph_v2` Rust runner implements exact sparse cosine neighbors,
optional centering/ABTT, union or mutual connectivity, cosine/SNN/local weights,
and seeded weighted CPM. The legacy commands above remain available.

```sh
cargo run --release --locked --bin graph_v2 -- build \
  --input data/input-embeddings.txt --output results/leiden-v2/graph \
  --k 20 --weight cosine

cargo run --release --locked --bin graph_v2 -- cluster \
  --graph results/leiden-v2/graph/graph.json \
  --output results/leiden-v2/seed42 --seed 42 --q 0.1
```

Every output directory must be new. Assignments keep the `token,community`
format. Build output includes GraphML, fitted transforms, directed neighbors,
candidate-pair diagnostics and graph metrics; clustering adds source-space
coherence, size/coverage/connectivity metrics and solver provenance.

The manifest-driven experiment runner supports controls, screening, ablations,
combinations, fresh-seed confirmation, perturbations and automated reports.
Automatic default nomination requires an explicit selection policy. No external
evaluation data is available; related metrics are null with reasons.
See [v2 usage and manifest settings](docs/graph-v2-usage.md) and the
[construction specification](docs/graph-construction-v2.md).

## Run GMM

```sh
cargo run --release --locked --bin gmm_cluster -- \
  --pca-components 30 --clusters 50,100,200 \
  --regularization 0.000001,0.00001 --seeds 42,43,44 \
  --criterion bic --output results/gmm/1-9/pca30
```

The pipeline is **L2-normalize each SGNS vector → subtract the global mean →
PCA → optionally whiten the retained components → fit GMM**. Add `--whiten`
to enable whitening. There is no final L2 normalization. PCA fits the normalized
input vocabulary, using an exact symmetric covariance decomposition in `f64`;
zero-variance or numerically rank-deficient projections fail with an error.

One invocation fixes the PCA representation and sweeps cluster counts,
covariance regularization and independent seeds. Full covariance is currently
the only variant supported by Linfa. Fitting and scoring use `f64`, with no BLAS
installation required. The default maximum is 300 EM iterations and tolerance
0.001; see `--help` for controls. Full covariance costs roughly O(n*k*d²) per
iteration, so start with a short grid before trying hundreds of components and
many seeds. Larger PCA dimensions substantially increase the work.

Each run needs a **new output directory** and produces:

| File | Contents |
| --- | --- |
| `candidates.csv` | Every attempted fit, log likelihood, parameter count, AIC, BIC, elapsed time and any failure. |
| `run.json` | Input SHA-256, command, preprocessing/settings, candidates and the AIC/BIC winners. |
| `k*-reg*-seed*.communities.csv` | Hard assignments for every successful candidate. |
| `communities.csv` | Hard assignments from the selected model. |
| `memberships.csv` | Selected model's full probability distribution for each token; column `probability_j` matches assignment/component `j`. |
| `pca.json`, `model.json` | Fitted preprocessing (mean, basis, variances) and selected GMM parameters. |

Rows are token-sorted. Probabilities sum to one per word; a hard assignment is
their argmax. Some mixture components may receive no hard assignments. These
are model responsibilities, not calibrated probabilities of semantic correctness.
Large membership/model files are kept locally and ignored by Git.

To export component contents, deterministic samples, boundary words and
soft-membership diagnostics from one or more completed GMM runs:

```sh
cargo run --release --locked --example gmm_review -- results/gmm/1-9/pca30
```

The [2026-09-18 GMM experiment archive](results/gmm/1-9/experiments-2026-09-18/REPORT.md)
contains a staged sweep and linguistic review tables. It imposes no target group
size and evaluates GMM independently of other clustering methods.

Lower AIC/BIC wins among successfully converged fits; failed/non-converged fits
are recorded and excluded. For full covariance the parameter count is
`p = (k - 1) + k*d + k*d*(d + 1)/2`; `AIC = 2p - 2 log L` and
`BIC = p log n - 2 log L`, using the total training log likelihood. PCA is held
fixed across that sweep. Seeds are separate initializations, not extra model
parameters. Linfa's `n_runs` does not restart initialization in 0.8.1, so this
runner performs independent fits itself. A small numerical patch stabilizes
GMM likelihoods; see [vendor notes](vendor/README.md).

Run 20D, 30D, 50D and whitening choices in separate directories. **Do not rank
their raw AIC/BIC together**: they describe different observations/density scales.
Use the shared original-space metrics and inspect clusters when comparing those
choices or comparing GMM with Leiden. AIC/BIC are density-fit criteria, not
semantic ground truth, and cannot be compared to Leiden's quality objective.

## Compare methods

```sh
cargo run --release --locked --bin compare_clusters -- \
  results/leiden/1-9/experiments-2026-09-17/recommended.communities.csv \
  results/gmm/1-9/pca30/communities.csv \
  --output results/gmm/1-9/pca30/versus-leiden.json
```

Pass any two or more assignment CSVs, including individual GMM candidates or
Leiden repeats. Every file must cover exactly the embedding vocabulary; missing,
unknown and duplicate tokens are errors. The report contains cluster size/coverage
statistics, within-cluster mean cosine, exact cosine silhouette in the original
normalized embeddings, and pairwise adjusted Rand index (ARI). Silhouette uses
batched dot products with cluster means rather than an n-by-n distance matrix.
It is undefined for one cluster or all singletons. ARI measures agreement between
partitions, not which method is better. Numeric IDs have no meaning across runs.

## Scale and reproducibility

For 10,936 words the complete graph contains 59,792,580 edges and occupies several
GB as GraphML. Building and filtering stream to disk; neither holds that complete
graph in memory. Similarities are computed in parallel batches of 32 rows, keeping
the additional edge buffer proportional to the number of words. All pairwise
similarities are still evaluated: O(n²d) work.

Leiden and `observe_graph` still load a graphrs graph into memory. Use the
thresholded graph for those commands. Clustering coefficients require
`--clustering-metrics`; centralities require `--centrality`. Weighted shortest-path
centralities interpret weights as distances, while these graphs store similarities.
Their distance model is deferred; do not interpret those path metrics as semantic
centrality on this baseline.

Graph export order is deterministic. Community IDs and CSV rows are sorted, but
graphrs does not expose a Leiden random seed: rerunning can change the partition.
Canonical IDs are not persistent identities across different partitions. Save a
run's CSV alongside its graph and settings.

The full-range run exposed overflow in graphrs' refinement sampling. A local
0.12.0 patch stabilizes the exponential weights without changing the intended
probabilities or the baseline settings; see [the patch notes](vendor/README.md).

`resolution` controls quality scoring. In graphrs, `gamma` controls refinement
connectivity for both CPM and modularity; the previous “CPM only” description was
incorrect. The baseline parameter values are unchanged.

## Engineering checks

```sh
cargo fmt --check
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked -p graphrs --lib
# Upstream GMM tests use the vendored crate's own development dependencies.
cargo test --manifest-path vendor/linfa-clustering-0.8.1/Cargo.toml \
  --lib gaussian_mixture --target-dir target/vendor-tests
```

`src/leiden.rs` and `src/gmm.rs` contain method-specific code;
`src/clustering.rs` contains the shared file format and comparison metrics.
The binaries handle command-line arguments and reporting.
See [the engineering review](docs/engineering-review.md) for the dependency audit,
fixed issues and remaining library limitations.
