# Updated-vocabulary Leiden study

Selected recommendation: **threshold-0.40-q0.3**, reviewed using fresh confirmation.

Unique-label assignments: `results/leiden-v2/ablations-2026-11-18/confirmation/candidates/threshold-0.40-q0.3/communities.csv`.

Source-ID assignments: `results/leiden-v2/ablations-2026-11-18/recommended.communities.csv`. All 11,000 source IDs are unique; 10,992 have memberships checked against the selected unique-label assignment. The remaining 8 IDs (8 unique labels) have no embedding and are preserved with a blank community and `missing_embedding` status.

[Recommendation rationale](DECISION.md) · [18-anchor qualitative review](LEARNING_REVIEW.md) · [Assignment integrity audit](assignment-audit.json) · [Implementation checks](engineering-checks.json)

## Objective and scope

The target is a similar distribution of community sizes to the supplied reference, while allowing memberships to change. Distribution distance is a descriptive fit to that preference; it is not semantic accuracy. The Rust protocol records the graph grid, resolution search, seeds and stress checks. This report performs no fitting, parameter tuning or automatic selection.

Cluster repeated surface labels once and map the resulting membership to each source ID. This preserves IDs without giving repeated spellings extra graph weight. Label-only embeddings cannot distinguish the senses represented by separate IDs.

Protocol: `results/leiden-v2/ablations-2026-11-18/screening/protocol.json` (SHA-256 `d986a139c863ca29370a83d5772853ad9abdc7a1f02a6d109ff22dfbda2a0979`).

Frozen selection rule: Profile distance = 0.5 token-size-band TV + 0.3 group-size-band TV + 0.2 abs(ln(mean-size/reference-mean)); bands 1,2,3-5,6-10,11-20,21-30,31-50,51-100,>100. Guardrails every seed >=95% nonsingleton coverage, <=4% largest group, connected nondegenerate partition. Confirm transferred threshold .40 q .3 plus best eligible threshold, raw-neighbor, and transformed-neighbor candidates; choose by development profile distance only if fresh-seed repeat ARI >=.65 and both paired stress median ARIs >=.65; otherwise report no qualified recommendation. Score is descriptive, no significance/semantic claim. No tuning after confirmation; no endpoint expansion.

Reference assignments: `results/leiden-v2/ablations-2026-09-26/confirmation/confirmation/b58dfe04de33ba8c519a49ab8641c0cfd99a5c4022b6a198e1ffe7db1adade67/communities.csv`.

## Community-size distribution

| Measure | Reference unique labels | Selected unique labels | Selected assigned source IDs |
| --- | ---: | ---: | ---: |
| Items | 10936.00 | 10888.00 | 10992.00 |
| Communities | 1360.00 | 1387.00 | 1387.00 |
| Mean community size | 8.04 | 7.85 | 7.93 |
| p25 | 3.00 | 3.00 | 3.00 |
| Median | 4.00 | 4.00 | 5.00 |
| p75 | 9.00 | 9.00 | 9.00 |
| p95 | 24.00 | 24.00 | 24.00 |
| Largest | 163.00 | 171.00 | 171.00 |

| Size band | Reference unique labels: groups / items | Selected unique labels: groups / items | Selected assigned source IDs: groups / items |
| --- | ---: | ---: | ---: |
| 1 | 12.50% / 1.55% | 11.82% / 1.51% | 11.25% / 1.42% |
| 2 | 12.21% / 3.04% | 12.83% / 3.27% | 12.76% / 3.22% |
| 3–5 | 33.24% / 15.69% | 34.39% / 16.90% | 34.32% / 16.76% |
| 6–10 | 20.37% / 18.86% | 20.48% / 19.94% | 21.12% / 20.39% |
| 11–20 | 15.07% / 27.13% | 14.28% / 25.97% | 14.28% / 25.74% |
| 21–30 | 3.16% / 9.64% | 3.32% / 10.66% | 3.39% / 10.83% |
| 31–50 | 2.21% / 10.66% | 1.59% / 7.62% | 1.59% / 7.56% |
| 51–100 | 0.96% / 8.31% | 1.01% / 9.04% | 1.01% / 9.05% |
| >100 | 0.29% / 5.11% | 0.29% / 5.09% | 0.29% / 5.04% |

Size bands show the fraction of groups and the fraction of items in those groups. Keeping both views exposes differences that a matched community count or median can hide. Source-ID sizes include every assigned repeated-label row. Missing-embedding rows remain unassigned and are excluded from community-size statistics; unique-label sizes describe the graph actually clustered.

## Screening

65 measured settings. The first 15 by profile distance are shown; `screening-summary.csv` contains every setting and the source directories.

| Candidate | Profile distance² ↓ | Groups¹ | Median² | p95² | Max² | Singletons¹ | 3–30¹ | >100¹ | Cohesion¹ | Silhouette¹ | Repeat ARI | Edge ARI | Subsample ARI |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| threshold-0.40-q0.3 | 0.0216 | 1386.0 | 5.0 | 24.0 | 165 | 1.54% | 71.79% | 5.08% | 0.443 | 0.068 | 0.726 | — | — |
| threshold-0.35-q0.55 | 0.0530 | 1305.0 | 5.0 | 25.0 | 186 | 1.11% | 69.42% | 6.31% | 0.437 | 0.063 | 0.693 | — | — |
| threshold-0.45-q0.15 | 0.0889 | 1527.0 | 4.0 | 21.0 | 144 | 3.33% | 75.99% | 3.66% | 0.448 | 0.059 | 0.777 | — | — |
| threshold-0.40-q0.225 | 0.0902 | 1142.0 | 5.0 | 30.0 | 207 | 1.32% | 66.09% | 8.96% | 0.424 | 0.060 | 0.734 | — | — |
| threshold-0.40-q0.4 | 0.1263 | 1688.0 | 4.0 | 18.0 | 141 | 2.05% | 76.46% | 3.21% | 0.464 | 0.076 | 0.745 | — | — |
| threshold-0.35-q0.4 | 0.1662 | 985.0 | 5.0 | 34.0 | 251 | 0.66% | 60.60% | 14.13% | 0.411 | 0.053 | 0.755 | — | — |
| raw-k20-q0.3 | 0.1945 | 1402.0 | 6.0 | 18.0 | 52 | 0.44% | 94.79% | 0.00% | 0.470 | 0.102 | 0.690 | — | — |
| threshold-0.45-q0.225 | 0.1978 | 1906.0 | 4.0 | 16.0 | 124 | 3.86% | 79.41% | 2.07% | 0.475 | 0.078 | 0.795 | — | — |
| snn-k10-q0.1 | 0.2083 | 1242.0 | 7.0 | 22.0 | 67 | 1.82% | 90.36% | 0.00% | 0.440 | 0.048 | 0.922 | — | — |
| snn-k10-q0.2 | 0.2234 | 1850.0 | 4.0 | 16.0 | 37 | 3.29% | 91.54% | 0.00% | 0.477 | 0.059 | 0.924 | — | — |
| blend-k10-q0.2 | 0.2440 | 1508.0 | 6.0 | 15.0 | 36 | 0.42% | 97.43% | 0.00% | 0.478 | 0.108 | 0.782 | — | — |
| threshold-0.40-q0.15 | 0.2451 | 867.0 | 7.0 | 38.7 | 262 | 1.09% | 57.06% | 16.85% | 0.397 | 0.045 | 0.755 | — | — |
| abtt3-k10-q0.2 | 0.2564 | 1524.0 | 6.0 | 15.0 | 35 | 0.32% | 97.59% | 0.00% | 0.478 | 0.106 | 0.762 | — | — |
| abtt1-k10-q0.2 | 0.2588 | 1521.0 | 6.0 | 15.0 | 36 | 0.31% | 97.76% | 0.00% | 0.480 | 0.110 | 0.775 | — | — |
| center-k10-q0.2 | 0.2592 | 1528.0 | 6.0 | 15.0 | 33 | 0.32% | 97.49% | 0.00% | 0.481 | 0.113 | 0.751 | — | — |

### Frozen screening finalists

These candidates were retained before fresh confirmation; the complete screening table may include settings outside the guardrails.

| Candidate | Profile distance² ↓ | Groups¹ | Median² | p95² | Max² | Singletons¹ | 3–30¹ | >100¹ | Cohesion¹ | Silhouette¹ | Repeat ARI | Edge ARI | Subsample ARI |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| threshold-0.40-q0.3 | 0.0216 | 1386.0 | 5.0 | 24.0 | 165 | 1.54% | 71.79% | 5.08% | 0.443 | 0.068 | 0.726 | — | — |
| raw-k20-q0.3 | 0.1945 | 1402.0 | 6.0 | 18.0 | 52 | 0.44% | 94.79% | 0.00% | 0.470 | 0.102 | 0.690 | — | — |
| abtt3-k10-q0.2 | 0.2564 | 1524.0 | 6.0 | 15.0 | 35 | 0.32% | 97.59% | 0.00% | 0.478 | 0.106 | 0.762 | — | — |

## Fresh confirmation

| Candidate | Profile distance² ↓ | Groups¹ | Median² | p95² | Max² | Singletons¹ | 3–30¹ | >100¹ | Cohesion¹ | Silhouette¹ | Repeat ARI | Edge ARI | Subsample ARI |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| threshold-0.40-q0.3 | 0.0324 | 1387.0 | 4.0 | 24.0 | 171 | 1.57% | 72.19% | 4.13% | 0.443 | 0.067 | 0.761 | 0.775 | 0.721 |
| raw-k20-q0.3 | 0.1975 | 1413.0 | 6.0 | 18.0 | 38 | 0.45% | 95.21% | 0.00% | 0.470 | 0.103 | 0.699 | 0.719 | 0.641 |
| abtt3-k10-q0.2 | 0.2566 | 1526.0 | 6.0 | 14.3 | 35 | 0.29% | 97.54% | 0.00% | 0.479 | 0.108 | 0.772 | 0.756 | 0.673 |

### Variation across confirmation seeds

| Candidate | Groups: min / median / max | Singleton token fraction: min / median / max | Largest group: min / median / max |
| --- | ---: | ---: | ---: |
| threshold-0.40-q0.3 | 1379.0 / 1387.0 / 1415.0 | 1.42% / 1.57% / 1.80% | 165.0 / 168.5 / 175.0 |
| raw-k20-q0.3 | 1401.0 / 1413.0 / 1431.0 | 0.39% / 0.45% / 0.52% | 38.0 / 40.5 / 52.0 |
| abtt3-k10-q0.2 | 1515.0 / 1526.0 / 1536.0 | 0.20% / 0.29% / 0.36% | 35.0 / 35.0 / 36.0 |

¹ Median across solver repeats; fractions are fractions of tokens. ² Representative assignment's profile distance and sizes, rather than a median across seeds. Edge/subsample ARI are medians of paired perturbations. Missing measurements are shown as —. Candidate CSVs retain representative values and per-seed minima, medians and maxima separately, including group count, singleton fraction and largest-group size.

The representative seed is chosen by repeat agreement in the Rust study. A repeated fit measures solver sensitivity; 5% edge removal measures graph sensitivity, and 90% vocabulary retention rebuilds the graph for the retained vocabulary. Similar size profiles can still have different members.

## Inspecting the recommendation

`community-sizes.csv` lists every selected group, largest first. `cluster-review.csv` lists every member of every group for qualitative inspection. Memberships and IDs should be joined by their explicit columns, since numerical community labels have no semantic meaning across runs.

Coherence, silhouette and margins use the same source embeddings as the graph. No independent embedding-space semantic quality, external labels, or learner evaluation was measured. Stability and a close size distribution support practical inspection; neither establishes semantic correctness. The new vocabulary may legitimately change topical group sizes.
