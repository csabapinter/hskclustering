# Graph v2

`graph_v2` is an additional Rust executable. It shares the embedding file format
and assignment CSV schema with the old commands, and has its own graph,
preprocessing, metrics and seeded Leiden implementation under `src/graph_v2/`.
The old commands, defaults and archived results are unchanged.

## Build and cluster

```sh
cargo run --release --locked --bin graph_v2 -- build \
  --input data/input-embeddings.txt --output results/leiden-v2/raw-k20 \
  --k 20 --symmetrization union --weight cosine --tau 0

cargo run --release --locked --bin graph_v2 -- cluster \
  --input data/input-embeddings.txt \
  --graph results/leiden-v2/raw-k20/graph.json \
  --output results/leiden-v2/raw-k20-seed42 --seed 42 --q 0.1
```

GraphML can be supplied to `cluster` in place of the JSON graph. All output
directories must be new; an existing directory is rejected without modifying it.
Invalid input and runtime failures are reported, and failures after directory
creation are recorded inside that directory.

Graph options:

| Option | Values / default |
| --- | --- |
| `--representation` | `raw` (default), `center`, `abtt` |
| `--components` | Leading directions removed by ABTT; default 1; use 3 for the other specified ablation |
| `--k` | Requested directed neighbors; default 20; effective count is `min(k,n-1)` |
| `--symmetrization` | `union` (default), `mutual` |
| `--weight` | `cosine` (default), `snn`, `local`, `cosine-snn` |
| `--tau` | Minimum **unshifted cosine**, default 0 |
| `--lambda` | Cosine/SNN blend fraction, default 0.5 |
| `--k-scale` | Local distance rank, default `min(7,k_eff)`; explicit values must fit the effective list |
| `--block-rows` | Parallel row batch, default 32; does not change neighbor results |

In JSON manifests, representations are `"raw"`, `"center"`, or `{"abtt": 1}`
(likewise `{"abtt": 3}`); the blended weight name is `"cosine_snn"`.

The legacy graph's stored weight is `(cosine+1)/2`. Thus its threshold `0.70`
is different from v2's `--tau 0.70`. V2 exports strictly positive weights only,
and retains every token, including isolates.

`cluster` defaults to `r=q*w50` with `q=0.1` and `theta=0.3*w50`. Override
resolution with `--resolution`, or theta with `--theta`. Gamma is always equal
to resolution. Empty-edge graphs produce singleton assignments. The seed defaults
to 42. Iteration, aggregation-level and local-move limits are configurable;
exhausting a limit is a recorded failure, not a successfully converged result.

## Experiments

For a controlled first comparison, calibrate the archived baseline before looking
at core candidates:

```sh
cargo run --release --locked --bin graph_v2 -- calibration-plan \
  --output calibration-plan.json
# Inspect the baseline paths, graph grid, seed cohorts and review anchors first.
cargo run --release --locked --bin graph_v2 -- calibrate \
  --plan calibration-plan.json --output results/leiden-v2/my-study/calibration
cargo run --release --locked --bin graph_v2 -- validate-manifest \
  --manifest results/leiden-v2/my-study/calibration/screening-manifest.json
cargo run --release --locked --bin graph_v2 -- experiment \
  --manifest results/leiden-v2/my-study/calibration/screening-manifest.json \
  --output results/leiden-v2/my-study/screening
```

The calibration plan defaults to 30 baseline solver fits, 20 paired replicates
of each perturbation, and 10,000 deterministic splits into two disjoint cohorts
of the planned screening sizes. The 95th percentile of absolute differences in
cohort medians sets each tolerance. Repeat ARI resamples whole partitions and
recomputes the within-cohort median; its pairwise rows are never treated as
independent replicates. These descriptive tolerances are neither confidence
intervals nor semantic effect-size thresholds. The coverage floor is the minimum
observed baseline coverage minus its tolerance. The concentration ceiling is
the observed maximum plus the observed range (at least one token of headroom).
All-singleton and one-community partitions are ineligible.

Calibration writes the protocol before fitting and freezes an explicit policy
only after every required fit/perturbation succeeds with defined metrics.
Calibration seeds cannot overlap development or reserved confirmation seeds.
The generated manifest references hashed calibration evidence. Validation and
execution reject changes to that study, source build, input, metadata, baseline
graph or archived assignment. JSON floats use exact round-trip parsing so a
saved tolerance cannot change on readback. Use a new plan/calibration directory
when changing the design; do not edit the frozen manifest to retune acceptance.

The calibration plan disables ablations, combinations and confirmation, leaving
the raw core grid, archived controls and development diagnostics enabled.
`review_anchors` fixes the practical inspection sample without making it a
semantic benchmark. The [2026-09-25 study](../results/leiden-v2/study-2026-09-25/COMPARISON.md)
includes the actual calibration, full comparison and a reproducible shortlist.

For the full multi-stage study without this baseline-only calibration workflow:

Generate the complete default manifest, inspect/edit it, then run it:

```sh
cargo run --release --locked --bin graph_v2 -- manifest \
  --output graph-v2-experiment.json

cargo run --release --locked --bin graph_v2 -- validate-manifest \
  --manifest graph-v2-experiment.json

cargo run --release --locked --bin graph_v2 -- experiment \
  --manifest graph-v2-experiment.json --output results/leiden-v2/experiment-01
```

Paths in the manifest resolve relative to the invocation's working directory.
Unknown fields are errors. The normalized manifest, seed lists, input/metadata
hashes, compiled source hashes and backend version are saved before screening.
The supplied `selection` is `null`: construction, evaluation and Pareto reporting
work, but no new default can be nominated.

The default study is substantial: three common screening seeds, at least ten
fresh confirmation seeds, ten development and ten confirmation perturbation
replicates of each kind, and 100 label permutations. Each perturbation is paired
with an unperturbed run using the same solver seed. The first `build`/`cluster`
examples are suitable for checking one configuration without launching the study.

Stages are:

1. Evaluate the archived recommended assignment and repeat its configuration as
   the baseline cohort. Run the stored-weight threshold/resolution controls and
   unshifted weights on those same edges, retuning the latter.
2. Cross the supplied `graphs` with `q`. The generated manifest has the specified
   nine core graphs and four initial q values.
3. If `ablations` is true, vary mutual connectivity, tau, cosine/SNN blending,
   centering and ABTT one factor at a time from the retained configurations.
4. If `combinations` is true, combine retained topology/weight choices at each k
   and repeat raw, center, ABTT(1) and ABTT(3).
5. Freeze development selection, then confirm every finalist and control using
   fresh seeds and fresh perturbations. Confirmation never retunes or substitutes
   a different nominee.

`max_endpoint_expansions` bounds adaptive search (default 2; each expansion uses
`endpoint_expansion_factor`, default 3). An endpoint expands when a Pareto-retained
resolution for that graph lies there. Hitting the budget is recorded in
`search-limits.json` and prevents automatic nomination from that graph. This
budget is an operational bound, not a claim that the initial grid is sufficient.
Lower expansion stops at one community per connected component in every repeat:
no smaller nonnegative resolution can coarsen a connected partition further.

Graphs are built once for each exact configuration and input hash, then reloaded
through a buffered JSON reader across solver settings. Vocabulary subsamples rebuild their fitted transforms and
neighbors; these graphs are also reused across resolutions. For the archived raw
threshold controls, rebuilding on a subset is exactly equivalent to retaining
the corresponding stored edges. Sampling rounds 5% edge removal and 90% token
retention to the nearest integer, with at least one retained token.

The representative assignment maximizes mean ARI to other repeats, breaking ties
by the smaller seed. Cohesion, silhouette and margins use unchanged normalized
source embeddings. External reference agreement and independent-space coherence
are `null` with explicit reasons: no such evaluation data is available.

## Explicit selection policy

To enable automatic nomination, supply all of these fields in `selection`:

| Field | Meaning |
| --- | --- |
| `metrics` | Nonempty list of objects containing `metric` and a finite nonnegative `absolute_tolerance` |
| `minimum_nonsingleton_coverage` | Required minimum fraction across screening repeats, in [0,1] |
| `minimum_communities` | Explicit degeneracy cutoff, a positive integer |
| `maximum_largest_community_share` | Maximum allowed concentration across repeats, in (0,1] |
| `reject_all_singletons` | Explicit boolean degeneracy rule |

Metric names are `cohesion`, `silhouette`, `margin`, `repeat_ari`,
`edge_removal_ari`, `subsample_ari`, and `nonsingleton_coverage`; higher is better
for each. Selection uses medians across successful repeats, requires complete
successful screening/stress coverage, and rejects missing required metrics.
Tolerances are absolute differences in each metric's own units.

The diagnostic Pareto panel uses all seven measurements. Undefined coherence
excludes one-community/all-singleton candidates from this panel; their diagnostics
are still exported. The baseline remains included. Nomination additionally
requires no metric to fall beyond tolerance and at least one to improve beyond
tolerance. Equivalent improvements prefer fewer transforms, then lower runtime.
Unresolved trade-offs do not produce an automatic winner.

Confirmation applies the frozen policy to the nominee and baseline's fresh
cohorts. Without acceptance, the original archived recommended assignments are
exported unchanged. If no baseline was supplied or it could not be loaded, no
baseline assignment is invented. `baseline`, `metadata`, and `selection` can be
`null` for experiments on another vocabulary. Smaller graph grids and disabled
ablation/combination stages are supported; the manifest records that scope.

## Artifacts and conventions

| Artifact | Contents |
| --- | --- |
| `manifest.json`, `provenance.json`, `experiment-status.json` | Frozen settings, input/code hashes, seeds, final status and cost |
| `graphs/<key>/` | Config, fitted transform, GraphML, serialized graph, directed neighbors, candidate-pair data, graph metrics |
| `attempts.json` | Every attempted graph/solver/perturbation/sensitivity setting, including failures |
| `candidates.csv`, `candidates.json` | Candidate settings, status, measurements and costs |
| `candidates/<id>/seed-*/` | Full assignments, per-word metrics, solver settings, objective and convergence data |
| `candidates/<id>/repeats.json` | Pairwise ARI/VI, directional per-community Jaccards, distributions and representative seed |
| `candidates/<id>/stress/` | Paired comparator and perturbed assignments, agreements and coverage |
| `candidates/<id>/sensitivity.json` | Separate k/r increases and decreases of approximately 20%, holding other settings fixed |
| `candidates/<id>/null-control.json`, `strata.json` | Label-permutation cohesion and metadata/degree strata |
| `perturbation-graphs/` | Rebuilt subsample graphs, edge-removal graphs, seeds, retained-token indices and provenance |
| `count-matched-comparisons.json` | Graphs at similar edge counts and partitions at similar community counts |
| `development-selection.json`, `confirmation.json`, `selection-report.json`, `REPORT.md` | Frozen nomination, confirmation, retained baseline/default and trade-offs |
| `communities.csv` | Confirmed representative or retained archived baseline; absent when neither exists |

`pairs.csv` keeps raw cosine, both directed ranks (blank if unselected), shared
neighbor count, Jaccard, proposed weight and retained status. Neighbor statistics
are computed before symmetrization or filtering. No connectivity repair adds
edges. Community connectivity is checked on each community's induced graph.

Quantiles use linear interpolation; dispersion is population standard deviation;
VI uses natural logarithms. Best-match Jaccards are reported in both directions.
Pairwise repeat comparisons are dependent diagnostics, not independent samples.
Strata summarize global memberships by HSK band, baseline-degree quintile, and POS
where supplied. Multiple mappings are reported and may place a token in multiple
strata. Missing POS/HSK data are counted; HSK bands never serve as community labels.

The graph builder uses O(nd+nk) storage and O(n²d) pairwise work. The optional
full covariance eigendecomposition additionally uses O(d²) storage. ABTT fits
normalized input, removes leading centered covariance directions and normalizes
again, following the specified [dominant-direction removal](https://arxiv.org/abs/1702.01417)
variant. Numerical rank uses a 1e-12 relative eigenvalue cutoff; transformed norms
at or below 1e-14 are treated as zero rather than amplifying roundoff. Fitted
directions, eigenvalues and absolute eigen residuals are saved.
Identical normalized vectors have cosine exactly 1 and distance 0, so rounding
in their self-dot product cannot distort duplicate-vector local scales.

The separate sparse CPM solver follows [Leiden Algorithm A.2](https://arxiv.org/html/1810.08473v3),
with seeded Xoshiro256++ randomness, deterministic token/edge/candidate order,
refinement gamma equal to resolution and stable exponential sampling. Strictly
positive local gains use a relative floating-point tolerance. Aggregate internal
weights are constant under subsequent moves; final objective values are always
recomputed on the original graph. Repeatability is verified across independent
processes on the same build; floating-point results need not be bit-identical
across architectures or compiler versions.

Peak memory is the process lifetime high-water RSS (bytes), explicitly labeled
as such; later stages may inherit an earlier peak. Solver time is separated from
metric/export time. CPM objective values are not used to rank different graphs.

## Checks

```sh
cargo fmt --check
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
```

Tests cover brute-force neighbor agreement and ties, union/mutual topology,
weight formulas, duplicate vectors, small vocabularies and isolates, eigensystem
residuals, direct metric calculations, seeded process repeatability, connectivity,
all four-node topologies, unequal-density mixtures and an outlier, manifest
validation, archived fallback, fresh-seed confirmation and recorded failures.
