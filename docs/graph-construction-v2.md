# Graph construction v2

Status: proposed. Output: weighted, undirected vocabulary graphs and Leiden CPM
partitions. Evaluation is automated. Nodes remain distinct spellings; assignments
retain the `token,community` format.

## Graph construction

1. Validate unique tokens, finite coordinates and nonzero vectors; L2-normalize.
   Reject empty input; for one token, bypass subsequent steps and emit a singleton.
   Require `k >= 1`, `0 <= tau <= 1` and `0 <= lambda <= 1`.
2. Apply the selected representation transform, then L2-normalize again:

   - `raw`: identity; initial default.
   - `center`: subtract the vocabulary mean.
   - `abtt(m)`: center, then remove projections onto the leading `m` covariance
     eigenvectors; test `m = 1, 3`. Fit on the input vocabulary using a full `f64`
     symmetric eigendecomposition; save the mean, basis and eigen residuals.

   Reject zero or nonfinite transformed vectors and component counts exceeding
   numerical rank. This ordering defines the
   [dominant-direction removal](https://arxiv.org/abs/1702.01417) variant.

3. Compute exact cosine neighbors in bounded blocks using `f64` accumulation.
   Clamp cosine to `[-1, 1]`; exclude self; break equal-score ties by token order.
   Set `k_eff = min(k, n-1)`.
   Retain neighbor lists and distances, requiring O(nd + nk) working storage
   rather than a dense similarity matrix. Pairwise computation remains O(n²d).
4. Symmetrize the directed lists: `union` retains either-direction selections;
   `mutual` requires both. Compute neighborhood statistics before symmetrization
   and filtering. These are alternative [connectivity rules](https://www.tml.cs.uni-tuebingen.de/team/luxburg/publications/MaiHeiLux07.pdf).
5. Retain candidate pairs with cosine `s_ij >= tau`; initial `tau = 0`.
6. Assign weights below; export strictly positive weights once per unordered pair.
   Preserve every node, including isolates. Do not add edges to force connectivity.

Let `N_i` be the directed neighbor set, `a_ij = max(s_ij, 0)`,
`d_ij = sqrt(max(0, 2 - 2*s_ij))`, and
`J_ij = |N_i intersect N_j| / |N_i union N_j|`.

| Weight mode | Definition | Parameters |
| --- | --- | --- |
| `cosine` | `a_ij` | None |
| `snn` | `J_ij`; shared-neighbor [Jaccard weighting](https://dpeerlab.github.io/dpeerlab-website/pub/cell2015.pdf) | Uses `N_i` above |
| `local` | `exp(-d_ij² / (sigma_i * sigma_j))`; [local scaling](https://papers.nips.cc/paper/2004/file/40173ea48d9567f1f393b20c855bb40b-Paper.pdf) | `sigma_i = max(d(i, kth_scale_neighbor), 1e-12)`; initial `k_scale = min(7, k_eff)` |
| `cosine_snn` | `a_ij * ((1-lambda) + lambda*J_ij)` | Initial `lambda = 0.5`; compare against both constituent modes |

Zero-overlap SNN edges disappear; report their count and resulting isolates. Store
raw cosine, neighbor ranks and overlap separately from final edge weights.

## Leiden and experiment design

Use weighted CPM, `Q = sum_C [sum_internal_weights - r*|C|*(|C|-1)/2]`.
Retune resolution for each graph; missing edges contribute zero. Compare objective
values only on identical graphs at identical resolution. See [Leiden/CPM](https://www.nature.com/articles/s41598-019-41695-z).

Expose seeded Leiden randomness and deterministic traversal/tie handling; save
backend version and seed. Keep the backend's refinement `gamma = r`. For new
graphs, search `r = q*w50`, where `w50` is median positive edge weight, and set
`theta = 0.3*w50`. Initial `q = {0.01, 0.03, 0.1, 0.3}`; expand an endpoint when
the retained candidates accumulate there. This is an initial search grid, not a
fixed resolution prescription. Zero-edge graphs yield singleton assignments.

| Stage | Comparisons |
| --- | --- |
| Controls | Archived raw graphs at stored-weight thresholds `0.70, 0.725, 0.75`, resolutions `0.05, 0.10, 0.20`, `gamma = r`, `theta = 0.30`; include the archived recommended partition. Add unshifted-cosine weights on the same edges, with retuned resolution. |
| Core | `raw`, `union`, `tau = 0`; cross `k = {10, 20, 40}` with `cosine`, `snn`, `local` and the resolution grid. |
| Ablations | On retained configurations, vary one factor at a time: `mutual`, `tau = {0.2, 0.4}`, `cosine_snn`, `center`, `abtt(1)`, `abtt(3)`. Retune resolution after each change. Combine retained changes and repeat component-removal comparisons. |
| Confirmation | At least 10 fresh Leiden seeds per finalist and control; stress tests below. Export the repeat with highest mean ARI to the other repeats, breaking ties by seed. |

Use three common seeds for screening. Construct each graph once; reuse it across
Leiden settings. Compare topology at similar edge counts and partitions at similar
cluster counts in addition to each method's tuned results. Counts are controls,
not optimization targets. Record every attempted setting, including failures.

## Automated evaluation

| Area | Required measurements |
| --- | --- |
| Graph | Nodes, edges, degree/strength/weight quantiles, isolates, components, largest-component fraction, neighbor reciprocity, incoming-neighbor count skewness and maximum. High incoming counts are diagnostics, not error labels. |
| Partition | Community count and size quantiles; singleton/pair fractions; largest-community share; fractions in sizes 3–30 and >100; induced connectivity of every community. |
| Coherence | Exact cosine silhouette, word-weighted nonsingleton cohesion and own-versus-best-other community similarity margins in the unchanged normalized 300D input. Also report in a separately sourced frozen embedding space when available, with coverage. |
| Solver variability | Pairwise ARI and variation of information across repeats; per-community best-match Jaccard distributions. Report median, dispersion and range; pairwise comparisons are not independent replicates. |
| Construction sensitivity | Edge-set overlap and partition ARI after changing `k` and `r` by approximately ±20%, holding other settings fixed. Preserve the parameter direction in reports. |
| Perturbation robustness | Ten common-seed replicates each of 5% random edge removal and 90% vocabulary subsampling. Rebuild transforms and graphs for vocabulary subsamples; compare partitions on retained tokens. Run the unperturbed comparator with the same solver seed. Report coverage changes alongside agreement. |
| Reference agreement | Existing lexical benchmarks: Recall@10/20 and MAP where candidate universes are defined; coassignment precision/recall on labeled pairs; Spearman correlation for graded similarity/relatedness scores. Keep relation types separate and retrieval cutoffs fixed across graphs. Unlisted pairs are unknown, not negative examples. |
| Null controls | Compare cohesion and available reference-agreement metrics with 100 token-label permutations preserving community sizes. Report observed-minus-null-mean and null quantiles. |
| Stratification | Repeat coverage/coherence summaries by HSK band, baseline-degree quantile and available part of speech. Record missing metadata and ambiguous mappings. HSK level is not a community label. |
| Cost | Graph construction and Leiden time, peak memory, artifact size and convergence/failure status. |

Singleton silhouette is zero; silhouette is undefined for one community or all
singletons. Undefined metrics and missing references are `null` with reasons.
Original-space coherence favors the source geometry; agreement, stability and
cluster sizes remain separate measurements. Perturbations measure sensitivity,
not uncertainty from embedding-training corpora.

Reference assets require version, license, checksum, vocabulary mapping and
coverage. Freeze benchmark splits before tuning, separating development and test
tokens; omit cross-split pairs. Test labels and any duplicate supervised relations
must be excluded from graph fitting and parameter selection. Test vectors may
remain in the unlabeled vocabulary graph. Reserve fresh seeds and perturbation
replicates for confirmation. Evaluate frozen candidates on test assets once.

## Selection

Reject candidates with invalid graph/partition invariants or degenerate partitions.
During development, retain the Pareto set over coherence, repeat agreement,
perturbation agreement and nonsingleton coverage; include the baseline. Add
reference-agreement metrics only when coverage and relation definitions are common
across candidates. Report large-community concentration alongside this set.

Freeze the metric panel, coverage limits and comparison tolerances in the experiment
manifest before screening. Nominate a default on development results: no metric
degrades beyond tolerance and at least one improves beyond tolerance relative to
the baseline. Resolve equivalent candidates by fewer transforms, then lower
runtime. Confirmation accepts or rejects that nomination without retuning; absent
confirmation, retain the baseline and publish trade-offs. Graph density, community
size bands and CPM objective alone do not select a winner.

## Optional extensions

| Extension | Integration requirement |
| --- | --- |
| [Mutual proximity](https://www.jmlr.org/papers/v13/schnitzer12a.html) | Separate neighbor-ranking variant; specify the distance-distribution estimator and compare with local scaling alone before combining corrections. |
| [Lexical retrofitting](https://aclanthology.org/N15-1184/) | Optional representation transform using versioned relation types and weights. Exclude supervised relations involving test tokens; compare with the unmodified representation. |
| [Consensus graph](https://www.nature.com/articles/srep00336) | On original candidate edges, use coassignment frequency across a fixed-setting ensemble as weight; evaluate the complete procedure using independent ensembles. Frequency is not a correctness probability. |

Sense-specific nodes require a separate assignment schema. PCA truncation and
whitening remain separate representation experiments.

## Artifacts and verification

Each experiment uses a new directory containing the configuration, input and code
hashes, fitted transforms, neighbor data, GraphML, run assignments, metric tables,
benchmark splits, seed lists and selection report. Cache keys include all graph
parameters and input hashes. Preserve legacy commands and archived artifacts.

Required checks: exact neighbor agreement against brute force; deterministic ties;
union/mutual and weight-formula fixtures; duplicate-vector/local-scale handling;
small-vocabulary and isolate preservation; eigensystem residuals; metric agreement
with direct calculations; seeded repeatability; complete assignment coverage and
community connectivity. Controlled vector mixtures with known labels, unequal
densities and outliers provide additional end-to-end recovery checks.
