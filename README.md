# HSK vocabulary clustering

Group Chinese words with related contexts/meanings into communities for studying,
using graphrs for Leiden and Linfa for optional embedding preprocessing.

The active output directory is `graphs/1-9-leiden/`. `graphs/1-3-leiden/` is an
archive; the earlier observation that about 72% of words formed distinguishable
thematic groups applies only to that experiment.

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

## Run the baseline

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

- `graphs/1-9-leiden/hsk1-9-full.graphml`
- `graphs/1-9-leiden/hsk1-9-thresh0.70.graphml`
- `graphs/1-9-leiden/hsk1-9-thresh0.70.communities.csv`

For subsequent runs that only need the clustering graph, the following computes
the same thresholded edges directly, without exporting the complete graph first:

```sh
cargo run --release --locked --bin build_graph -- --threshold 0.70
```

All commands accept `--input` and `--output`. Every node is retained when filtering,
including isolated words. Output directories are created as needed, and completed
files replace prior results atomically. `--preprocess` still enables PCA whitening;
it is off for this baseline. Preprocessing and clustering choices will be revisited
separately from the engineering changes.

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
```

The library separates embeddings, streaming graph I/O, community detection and
atomic output; the binaries handle command-line arguments and reporting.
See [the engineering review](docs/engineering-review.md) for the dependency audit,
fixed issues and remaining library limitations.
