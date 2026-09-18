# GMM staged sweep — 2026-09-18

Completed **85 full-vocabulary fits; all 85 converged**. Every fit covers the same
10,936 words. The fitting implementation is commit `8dd55af`; it was not changed
for this experiment. Eight deliberately different configurations were each run
with five independent seeds. **No semantic winner has been selected.**

The objective is semantic relevance, with no prescribed number or size of groups.
A mixture component can contain multiple semantic groups, and a word can have
responsibility in several components. Component identities do not define a
linguistic ontology. This archive contains no cross-method comparison: only CSV
field names from the existing archive were consulted for format consistency.

Start with the [review index](review-index.csv), the
[qualitative observations](linguistic-observations.csv), and the
[ambiguous-word examples](ambiguous-words.csv). These link to complete component
catalogues, dictionary-enriched word details and the locally retained probability
matrices. All [run measurements](run-summary.csv), settings, logs and assignments
are retained, including the concentration-warning cases.

## What was run, and why

| Stage | Fits | Scope and decision |
|---|---:|---|
| Screening | 15 | PCA 20/30/50D × k=10/25/50/100/200; no whitening; ridge 1e-5; seed 42. |
| Sensitivity | 32 | Lower-k controls (2,5), higher dimension (100D), upper-k probe (400), raw ridge 1e-6/1e-4/1e-3 and whitened ridge 0.01/0.1. Exact non-Cartesian grid is in the manifest. |
| Strong-ridge probe | 6 | Raw ridge 0.01 at 20/50/100D × k=50/200, after high-dimensional fits retained almost-hard responsibilities. |
| Independent repeats | 32 | Four additional seeds, 43–46, for each of eight recorded review candidates; seed 42 remains the browsing example. |

Each stage's manifest records its rationale before its fits. All runs use full
covariance, at most 300 EM iterations and tolerance 0.001. The sum of recorded fit
times is about 18 minutes; two jobs ran concurrently. That sum excludes model/CSV
export and review work and is not elapsed wall-clock time.

PCA retained **21.81%, 27.67%, 37.32% and 56.09%** of the centred,
L2-normalized embedding variance at 20, 30, 50 and 100 dimensions respectively
(the exact ratios are in each `metrics.json`). This motivated testing 100D after
the initial screen. Variance retained does not establish semantic relevance.

## Concentration and overlapping membership

The initial 15 fits had no hard components of size 1–5 and no empty hard
components. This describes the initial grid; balanced sizes were never a target.
The k=2 controls split the vocabulary roughly in half, with no tiny tail. A large
share alone therefore does not diagnose the failure pattern of concern.

The strongest raw regularization at **20D, k=200, ridge 0.01** produced a clear
concentration warning: only **11 of 200 components** received hard assignments.
The two largest contain **4,603 and 3,676 words**, or **75.7%** together; **189**
have zero hard members. Its three tiny nonempty components contain `赶忙`, `骂`,
and `门口 | 院子 | 楼道 | 旁边`. This is a concentration/occupancy finding, not a
claim that those small groups are intrinsically incoherent. The 20D k=50 version
also fell to 11 occupied components. Both are retained as diagnostics and are
outside the repeat shortlist.

The same numerical ridge is a different intervention at other dimensionalities:
100D k=50, ridge 0.01 retains all 50 occupied components and substantial overlapping
responsibilities. At 50D k=200, ridge 0.01, the five repeats retain 180–184 occupied
components, with largest shares 6.1–9.1%. This alternative remains in the review
set so its semantic structure and uncertain words can be examined.

More overlap is not automatically more semantic information. For example, in
20D k=50/ridge 0.001, `农业` has near-equal responsibility in a water/land/infrastructure
component and a business/industry component, a plausible boundary. But `宇航员`
has near-equal responsibility in patient/family and sports components, a less
readily explained result. The ambiguous-word table preserves both useful and
awkward cases; it does not equate uncertainty with polysemy.

## Eight candidates for linguistic review

These settings are a varied inspection set, not a ranked recommendation. All
numbers below average five seeds; ARI averages the ten seed-pair comparisons
within that configuration. ARI describes computational repeatability, not
semantic quality, and hard partitions can change while recognizable themes persist.

| PCA | Whitening | Components | Regularization | Largest share, mean | Mean max membership | Mean seed ARI |
|---:|:---:|---:|---:|---:|---:|---:|
| 20 | no | 10 | 1e-05 | 14.97% | 0.9161 | 0.605 |
| 20 | no | 50 | 0.001 | 5.16% | 0.8849 | 0.483 |
| 20 | yes | 50 | 0.1 | 5.55% | 0.8958 | 0.517 |
| 50 | no | 200 | 0.001 | 1.49% | 0.9992 | 0.352 |
| 50 | no | 200 | 0.01 | 7.72% | 0.5872 | 0.584 |
| 50 | no | 400 | 0.001 | 0.79% | 0.9998 | 0.294 |
| 100 | no | 50 | 0.01 | 5.61% | 0.8534 | 0.510 |
| 100 | no | 200 | 0.001 | 1.31% | 1.0000 | 0.331 |

The full-covariance fits can be extremely decisive within a run while changing
substantially between initializations. For example, 50D/k=400/ridge 0.001 has mean
maximum responsibility about 0.9998 but mean seed ARI about 0.294. Posterior
responsibility should not be interpreted as semantic confidence or assurance of
repeatability. No setting was chosen by maximizing certainty, overlap, ARI,
silhouette, or balance of component sizes.

The catalogues include component-level best-overlap matches across the four other
seeds. These permit a more focused review than global agreement alone: the
100D/k=200 animal component containing `猫` has mean best-match Jaccard about
0.888, while the 50D/k=400 computing component containing `软件` is about 0.243.
Both have recognizable centres in the displayed seed. Numeric IDs have meaning
only within their saved run.

## Semantic observations so far

Fourteen explicit, limited spot-checks are recorded in
[linguistic-observations.csv](linguistic-observations.csv). They include central
words, hash-selected samples, boundary words, and diagnostically weak components.
The broader [review sample](linguistic-review-sample.csv) contains 461 component
rows whose linguistic-review fields remain available for a systematic pass.

- **Multiple related groups can coexist usefully.** The 50D/k=200 component
  containing `软件` includes computing devices, audiovisual equipment and
  appliances: `电脑, 显示器, 硬盘, 打印机, 录音机, 相机`, with `洗衣机, 灯泡, 闹钟`
  at the boundary. This can be read as several related device groups.
- **Recognizable thematic cores occur.** At 100D/k=200, an inspected animal
  component contains `狗, 猫, 蛇, 鹅, 青蛙, 鸽子, 鸭子, 狮子`; an operational
  computing component contains `下载, 查询, 扫描, 数据库, 浏览器, 登录, 搜索`.
  These are examples, not a semantic-accuracy estimate for the entire partition.
- **Broad emotional and physical relationships also occur.** The 50D/k=200
  component containing `快乐` centres on pleasant/calm states, while its boundary
  includes thermal comfort and sensory words. Whether this organization is useful
  is a linguistic question; multiple subthemes alone are not a defect.
- **Tiny does not imply incoherent.** In the displayed 50D/k=400 run, a complete
  two-word component is `传人 | 失传`, and another small component is
  `奏效 | 见效 | 管用 | 根治`. The seed repeats produce 0–7 components of size 1–5;
  none of those runs is dominated by one giant component.
- **Less interpretable material remains.** Some weak-cohesion components mix
  single-character items, numbers and possible naming/morphological contexts.
  Samples include `福, 德, 利, 海, 宝` alongside `〇, 二, 百, 管, 族, 港, 电`.
  Attractive animal examples do not resolve these cases.
- **Corpus usage and senses require attention.** `苹果` can fall into food-related
  or market/business contexts; `花` can appear in action or spending contexts.
  The inputs contain one vector per spelling. These observations do not establish
  which sense a future linguistic organization should prefer.

## Density criteria and what remains undecided

AIC/BIC were recomputed from each fit's total training likelihood and full-covariance
parameter count. Rankings are restricted to identical PCA/whitening representations.
The initial screen's BIC minima were all at k=10; adding lower-k controls moved the
50D minimum to k=5 while the initial 20D minimum remained at k=10. These observations
characterize density fit and parameter penalties; they do not select the semantic
review candidates. Search coverage differs between representations, so even each
within-representation minimum is only a result over its recorded candidate set.

This sweep does not supply an exhaustive parameter optimum, held-out semantic
labels, a semantic accuracy percentage, or a selected linguistic taxonomy. The
next useful step is a linguistic pass through the eight review catalogues and
ambiguous words: identify the meaningful groups within and across components,
record unexplained boundaries, and decide which representation deserves further
experimentation. That pass can preserve several themes within one component and
several memberships for one word.

## Verification and reproducibility

All 26 repository tests passed with the review example included in the build;
Clippy passed with warnings denied for project code (the existing vendored
`graphrs` parentheses warning remains). The review exporter checks every token,
probability range/sum, hard-assignment argmax, and catalogue membership. Direct
pairwise cosine calculations in an independent Python check agree with five
exported word silhouettes to within 1.6e-9.

[validation.json](validation.json) records the archive-wide checks: stage/job
coverage, exact vocabulary coverage, input/dictionary/binary hashes, independently
recomputed AIC/BIC, finite serialized models, consistent PCA exports and orthonormal
bases. [README.md](README.md) gives reproduction commands, sampling definitions,
file roles and the limits of the repeat diagnostics. Full models, full probability
matrices, per-run PCA bases and raw per-word geometry remain local and are ignored
by Git. Each run's canonical `communities.csv` is versioned; its identical named
candidate copy is ignored in this single-fit archive. Review tables remain versioned.
