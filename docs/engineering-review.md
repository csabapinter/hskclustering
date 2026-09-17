# Rust engineering review — 2026-09-17

Scope: dependency maintenance, implementation correctness, resource use, API
boundaries and failure handling. The HSK 1–3 archive is unchanged. The HSK 1–9
baseline uses raw embeddings, weight threshold 0.70, CPM resolution 0.05, theta 0.3
and gamma 0.05. No new preprocessing or clustering model was selected.

## Assessment

The original separation between embeddings and CLI programs was a reasonable
starting point. The principal problems were quadratic graph storage, unchecked
inputs and library assumptions, inconsistent defaults, and no regression tests.
Several small binaries are appropriate here; a framework or a new graph backend
is not needed to fix those problems.

## Dependency audit

Versions were checked against the published crate metadata and downloaded source.
`Cargo.lock` was regenerated with current compatible transitive releases.

| Dependency | Previous lock | Updated | Notes |
| --- | --- | --- | --- |
| linfa | 0.7.1 | 0.8.1 | Removed unused serialization feature. |
| linfa-reduction | 0.7.1 | 0.8.1 | Pure Rust PCA backend retained and tested. |
| ndarray | 0.15.6 | 0.16.1 | Required by Linfa; 0.17.2 uses incompatible array types. |
| graphrs | 0.11.13 | 0.12.0 + local fix | Current release; full-range run required stable refinement weights. |
| clap | 4.5.51 | 4.6.7 | Current compatible CLI parser. |
| anyhow | 1.0.100 | 1.0.104 | Current compatible error handling. |
| csv | — | 1.4.0 | Standard CSV quoting instead of hand-built records. |
| quick-xml | transitive 0.37.5 | direct 0.42.0 | Current streaming XML parser; graphrs still brings 0.37.5. |
| tempfile | — | 3.27.0 | Atomic publication in the output's filesystem. |
| rayon | transitive 1.11.0 | direct 1.12.0 | Bounded parallel similarity computation. |

Linfa 0.8.1 explicitly pins `sprs = 0.11.2` to avoid an ndarray version mismatch.
Do not independently force sprs or ndarray to their newest minor versions.
`linfa-clustering` has not been added as an unused dependency; add its 0.8.1 release
when implementing GMMs. The 0.8 line adds GMM probability prediction. The covariance
update fix was already present in the previously resolved Linfa 0.7.1 ecosystem.

Sources: [Linfa changelog](https://github.com/rust-ml/linfa/blob/master/CHANGELOG.md),
[Linfa 0.8.1 dependencies](https://docs.rs/crate/linfa/0.8.1),
[linfa-reduction](https://docs.rs/crate/linfa-reduction/0.8.1),
[ndarray releases](https://docs.rs/crate/ndarray/latest),
[graphrs releases](https://docs.rs/crate/graphrs/0.12.0),
[clap](https://docs.rs/crate/clap/4.6.7),
[anyhow](https://docs.rs/crate/anyhow/1.0.104),
[csv](https://docs.rs/crate/csv/1.4.0),
[quick-xml](https://docs.rs/crate/quick-xml/0.42.0),
[tempfile](https://docs.rs/crate/tempfile/3.27.0).

This is a release/API/source review, not a full RustSec vulnerability audit.
`cargo-audit` is not installed in the environment.

## Validation

- 17 project tests passed (15 library tests and 2 CLI integration tests).
- All 89 graphrs library tests passed, including the 2 added sampling regressions.
- `cargo fmt --check`, `git diff --check`, and
  `cargo clippy --all-targets --locked -- -D warnings` passed. The vendored upstream
  crate emits one existing `unused_parens` compiler warning in its clustering
  module; no unrelated upstream style edits were added to the patch.
- The complete GraphML was parsed through the streaming filter: 10,936 nodes,
  59,792,580 edges read, 303,267 retained. An independent XML parse verified that
  every retained endpoint and weight exactly matches direct thresholded export,
  with no duplicate or self edges. All 10,936 nodes appear in both files.
- The patched full-range Leiden run completed in about 240 seconds with 508
  communities. An independent CSV/XML check verified exactly one assignment per
  input token and that all 508 communities induce connected subgraphs. Detailed
  settings, artifact hashes and coverage are in the [run notes](../graphs/1-9-leiden/README.md).

## Fixed engineering issues

1. **Complete graph memory growth.** The old builder retained every edge in
   graphrs. Its apparent chunking only batched insertion; it did not bound graph
   storage. The GraphML writer then constructed a second, complete XML string.
   Export now streams through a buffered file, with similarities computed in
   ordered parallel batches. At 10,936 words this avoids materializing 59,792,580
   edge objects. Complete export remains available; direct thresholding avoids
   exporting it when only a clustering input is needed.
2. **Filtering memory growth.** The old filter loaded the complete graph and
   copied retained nodes/edges to another graph before serializing it. The filter
   now buffers one edge at a time and copies all node/graph metadata and retained
   edge attributes. Missing/non-finite weights and incomplete XML are errors.
   It supports this pipeline's explicit edge weights, not inherited GraphML
   default weights or nested graphs.
3. **Malformed embeddings.** Extra dimensions, duplicate tokens and non-finite
   values were accepted. Shape/count checks are now strict; errors include the
   failing row/token. Header values no longer trigger immediate large allocations.
4. **Numerical edge cases.** Normalization accumulates squared norms in f64 so
   finite f32 extremes cannot overflow/underflow the norm. Zero/non-finite norms
   are rejected. Cosine results are clamped to their mathematical range before
   weight conversion. This can cause tiny roundoff differences from old exports.
5. **PCA API boundaries.** Invalid shapes/component counts now fail explicitly;
   the maximum centered rank is bounded by min(dimensions, samples − 1). Removed
   fabricated target labels. Regression coverage checks whitened covariance after
   the dependency upgrade. PCA remains opt-in.
6. **Lost nodes and unsafe assumptions.** Nodes are written explicitly, preserving
   isolates and single-node graphs. Empty input no longer underflows edge counts.
   Leiden requires finite non-negative weights, validates numeric parameters, and
   checks that every graph node appears exactly once in the returned partition.
   Zero-total-weight graphs return singleton communities without undefined modularity.
7. **Output correctness.** CSV uses a real serializer; GraphML tokens are escaped.
   Community members, IDs and rows are canonicalized. All file writers flush and
   publish via a temporary file in the destination directory; a failed write
   retains an existing result. Commands reject overwriting their own input.
8. **CLI consistency.** Active defaults use `graphs/1-9-leiden/`, thresholds are
   reflected in filenames without rounding distinct values together, and paths
   use `PathBuf`. Long-running centralities are opt-in. Median community size now
   handles even counts correctly. Edge-weight summaries avoid an extra full copy.
9. **Maintainability.** Community detection and output moved out of the CLI into
   library functions. Small, named configuration and output boundaries replace
   mixed parsing/computation/serialization. Tests exercise parsing failures,
   numerical behavior, streaming round trips, isolates, CSV quoting, atomic writes
   and the default CLI pipeline.
10. **Confirmed upstream overflow.** The first full-range Leiden run panicked in
    `rand::Uniform::new` with `range overflow` after about 225 seconds. Graphrs
    exponentiates raw gains before constructing its sampling distribution. The
    local patch uses `exp((delta - max_delta) / theta)`, preserving relative
    probabilities while making each weight at most one. Large-gain and probability
    invariance regression tests cover the fix. See [vendor/README.md](../vendor/README.md)
    for provenance, the exact changes and the removal plan.

## Remaining tradeoffs

- **Leiden backend.** Upstream's Leiden source in 0.12.0 is unchanged from 0.11.13.
  The local copy fixes the overflow observed on this input. It still creates
  repeated node/partition collections in inner loops, hides its RNG seed, and
  contains other internal unwraps. Carrying a local dependency patch adds a small
  maintenance obligation: review/remove it on future upgrades. Replacing the
  backend, exposing a seed throughout its randomized operations, or substantially
  optimizing its internals would require separate correctness comparisons.
- **GraphML loading.** Observation and Leiden still use graphrs' in-memory reader
  and adjacency structures. They should receive the thresholded graph. Upstream's
  reader also contains unchecked parsing paths; its supported format is narrower
  than the full GraphML specification. Export/filter scalability does not imply
  that arbitrary dense input can be clustered cheaply.
- **No claim of identical reruns.** Ordered export and canonical CSV formatting
  remove incidental output ordering changes, but do not make a randomized
  partition deterministic. Community IDs can shift when membership changes.
- **Data science deferred.** Whitening/rank selection, similarity thresholds,
  resolution, GMM design, distance transforms for weighted path centralities, and
  what to do about the 36 unmatched vocabulary entries remain separate choices.
  No missing embeddings were synthesized and no semantic quality improvement is
  claimed for this engineering pass.
