# Recommendation

Use [recommended.communities.csv](recommended.communities.csv): raw cosine
threshold **0.40**, CPM **q=0.30**, resolution and theta
**0.13178174442515958**, fresh representative **seed 1008**. It is the
[verified export](assignment-audit.json) of the [frozen Rust decision](confirmation/recommendation.json).
The CSV has `ID,token,community,status`, in the original input row order.

This setting most closely matches the requested cluster-size character among
the 65 screened settings, and it passed the predeclared fresh-seed and
perturbation requirements. Its development profile distance was 0.0216, compared
with 0.1945 for raw kNN k=20/q=.30 and 0.2564 for ABTT(3) k=10/q=.20.
Confirmation checked the same settings rather than retuning them. The
representative maximizes mean agreement with the other nine fresh partitions;
it was not chosen for a favorable size score or individual word group.

| Unique-spelling measure | Supplied reference | Recommended representative |
| --- | ---: | ---: |
| Assigned spellings | 10,936 | 10,888 |
| Communities | 1,360 | 1,387 |
| Mean size | 8.04 | 7.85 |
| p25 / median / p75 | 3 / 4 / 9 | 3 / 4 / 9 |
| p95 | 24 | 24 |
| Largest group | 163 | 171 |
| Words in groups over 100 | 5.11% | 5.09% |
| Singleton words | 1.55% | 1.51% |

The full band comparison and repeat medians/ranges are in the [study report](README.md).
Across ten fresh seeds the recommended setting produced 1,379–1,415 groups;
the largest ranged from 165–175. Median pairwise repeat ARI was **0.7612**,
paired 5% edge-removal ARI **0.7747**, and paired 90% vocabulary-retention ARI
**0.7211**. Every partition was connected within each community.

The kNN alternatives produce a different distribution: the raw k=20 fresh
representative has 1,410 groups but a maximum size of 38 and no group above 100.
It has better source-space cohesion and silhouette, but its vocabulary-subsample
ARI of 0.6413 misses the frozen .65 requirement. ABTT(3) passes the stability
requirements and has still higher cohesion, but produces median size 6, maximum
35 and no large tail. It is an inspectable alternative for more uniform study
sets rather than the closest match to the requested size profile. ID-bearing
exports of both finalists are retained beside their representative assignments.

Looser threshold .35 produces substantially larger groups at low resolutions;
raising q toward .55 moves it closer to the target but leaves a larger tail and
worse profile fit. Threshold .45 fragments more words into singletons. The
centering, ABTT, mutual, shared-neighbor, local-weight and blended-weight
ablations are all visible in [screening-summary.csv](screening-summary.csv).
The search covers the explicit grid, without a claim of exhaustive optimization.

The [qualitative review](LEARNING_REVIEW.md) finds recognizable food, contracts,
questions, sports and travel themes, along with concrete weak boundaries and
seed-sensitive examples around policy, banking and healthcare. It does not
establish semantic accuracy or change the numerical decision. The same source
embedding space supplies the graph and coherence diagnostics; no independent
semantic labels or learner evaluation were available.

All **11,000 IDs** are preserved. Repeated spellings share their one node's
community and account for 104 additional assigned ID rows. **10,992 IDs** have
memberships; the [eight missing-vector rows](missing.csv) retain blank communities
and `missing_embedding` status. No fallback vectors or substitute spellings were
introduced. Group-size comparisons above count unique spellings; ID-expanded
statistics are separately reported.

The study completed **315 Rust solver fits**, plus 100 label-permutation cohesion
controls for each representative. [Implementation checks](engineering-checks.json)
record 54 passing Rust tests, six Python checks, formatting and lint. The full
[assignment audit](assignment-audit.json) verified 384 assignment files and more
than four million rows with zero errors. Reproduction and ID export commands are
in [the usage guide](../../../docs/updated-vocabulary-leiden.md).
