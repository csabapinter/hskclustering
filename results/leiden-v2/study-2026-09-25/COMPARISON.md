# Calibrated graph v2 comparison — 2026-09-25

**0 raw core configurations pass the frozen replacement comparison.** The development nominee is **control t=0.7 unshifted q=0.3**. It still needs fresh-seed confirmation, so the archived recommendation remains the default.

Completed **96 settings**, including **65 raw core settings**, with three common screening seeds each, ten paired edge-removal and ten vocabulary-subsample replicates per setting. **0 failed attempts** out of 5621 recorded attempts. Centering, ABTT, mutual-neighbor ablations and fresh-seed confirmation remain deferred.

The shortlist below identifies development starting points; it is not evidence of independently validated semantic superiority.

## Baseline calibration

Used 30 seeded baseline fits and 20 paired replicates of each perturbation kind, on the archived shifted-cosine threshold 0.70 graph, resolution/gamma 0.20, theta 0.30. Calibration seeds are separate from screening and reserved confirmation seeds. The seeded v2 solver is used for both graph families; the archived assignment is also evaluated directly to expose any backend/partition difference.

| Measurement | Baseline median | Observed min–max | Frozen absolute tolerance |
|---|---:|---:|---:|
| cohesion | 0.439887 | 0.439039–0.441185 | 0.001142 |
| silhouette | 0.061360 | 0.058760–0.063610 | 0.003049 |
| margin | 0.039376 | 0.037599–0.040691 | 0.001957 |
| repeat_ari | 0.754774 | 0.687961–0.798447 | 0.039403 |
| edge_removal_ari | 0.787694 | 0.738496–0.811949 | 0.016295 |
| subsample_ari | 0.724849 | 0.688537–0.762865 | 0.027048 |
| nonsingleton_coverage | 0.984089 | 0.982718–0.985278 | 0.001189 |

Every screening repeat must retain at least **98.1529% nonsingleton coverage**; its largest group must contain no more than **1.7648%** of the vocabulary. At least two groups are required and all-singleton partitions are rejected.

100% baseline-only. Shuffle calibration seeds without replacement into two disjoint cohorts matching the planned screening cohort sizes. Compare medians; repeat ARI is the median of the within-cohort pairs, resampling whole partitions, never ARI rows. Absolute tolerance = linear-interpolated 95th percentile of absolute between-cohort differences, with a 1e-6 numerical floor (coverage: at least one token). These are empirical sensitivity tolerances, not confidence intervals, semantic thresholds, or multiplicity-adjusted tests. Coverage floor = minimum calibration coverage minus calibrated coverage tolerance. Largest-share ceiling = observed maximum plus observed range, at least one token of headroom. Minimum communities = 2; reject all singletons. No study candidate or confirmation seed is observed.

[Frozen manifest](calibration/screening-manifest.json), [calibration evidence](calibration/calibration.json), and [pre-screening analysis protocol](analysis-protocol.json) preserve the numeric rules and fixed review sample. The runner verifies the evidence, study, input, metadata, archived graph/assignment, and compiled source hashes before screening.

The original archived representative has 1372 groups, cohesion 0.4406, silhouette 0.0586, coverage 98.30%, and largest group 175. Its metrics are a single partition, whereas the screening baseline below summarizes fresh common-seed repeats.

Agreement between the archived assignment and the 30 seeded baseline fits: median ARI **0.7379**, range 0.7045–0.7727. This bridge check separates the unchanged archived partition from the baseline distribution under the common seeded solver; it is not a semantic quality score.

## Development shortlist

Keep the baseline. For each of cosine, SNN and local weights, consider valid core candidates on the diagnostic Pareto frontier that satisfy all frozen coverage, concentration and degeneracy limits. Select the candidate with the smallest worst regression relative to the screening baseline, measured in calibrated tolerance units across the seven frozen metrics. Break ties by more metrics improved beyond tolerance, then lower graph-build plus mean solver time, then candidate ID. A shortlisted trade-off is not an accepted replacement. If no candidate in a weight family passes the limits, report that family as having no eligible shortlist entry.

| Configuration | Groups | Cohesion | Silhouette | Margin | Repeat ARI | Edge ARI | Subsample ARI | Coverage | Largest | Graph setup s | Solver s |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| seeded baseline | 1339 | 0.4401 | 0.0625 | 0.0400 | 0.7703 | 0.7880 | 0.7227 | 98.45% | 170 | 0.71 | 0.89 |
| cosine k=10 q=0.1 | 918 | 0.4383 | 0.0946 | 0.0591 | 0.7500 | 0.7456 | 0.6621 | 99.93% | 46 | 1.49 | 0.53 |
| snn k=40 q=0.3 | 961 | 0.4189 | 0.0466 | 0.0297 | 0.8390 | 0.8535 | 0.7422 | 98.98% | 122 | 1.73 | 1.70 |
| local k=10 q=0.1 | 898 | 0.4381 | 0.0948 | 0.0592 | 0.7547 | 0.7420 | 0.6539 | 99.98% | 40 | 1.63 | 0.51 |

Partition scores are medians across three fits; solver time is the mean, and graph setup is measured once per graph. Largest is median largest-group size; the stricter max-over-repeats guard is in the comparison CSV. Cohesion/margin exclude singleton words; silhouette assigns them zero. ARI pair values are dependent diagnostics.

Archived graph setup loads, filters and exports an existing graph; it excludes that graph's original all-pairs construction. V2 graph setup includes exact neighbor construction from embeddings. These setup costs therefore have different starting points. The solver timings compare the same seeded backend; cosine/local k=10 fit faster than the baseline here, while the SNN shortlist entry fits more slowly.

- **cosine k=10 q=0.1** (`889a5c819578`): improved beyond tolerance: silhouette, margin, nonsingleton_coverage; regressed beyond tolerance: cohesion, edge_removal_ari, subsample_ari. Worst regression: 2.60 tolerance units. Graph build 1.49s, 74,756 edges; endpoint-limited search: no.
- **snn k=40 q=0.3** (`431881a18b59`): improved beyond tolerance: repeat_ari, edge_removal_ari, nonsingleton_coverage; regressed beyond tolerance: cohesion, silhouette, margin. Worst regression: 18.55 tolerance units. Graph build 1.73s, 281,399 edges; endpoint-limited search: yes.
- **local k=10 q=0.1** (`0e7bc6a16e43`): improved beyond tolerance: silhouette, margin, nonsingleton_coverage; regressed beyond tolerance: cohesion, edge_removal_ari, subsample_ari. Worst regression: 2.83 tolerance units. Graph build 1.63s, 74,756 edges; endpoint-limited search: no.

### Perturbation coverage, null controls and local sensitivity

| Configuration | Edge-removal coverage change | Subsample coverage change | Cohesion above null mean | k −20% ARI | k +20% ARI | r −20% ARI | r +20% ARI |
|---|---:|---:|---:|---:|---:|---:|---:|
| seeded baseline | -0.032 pp | -0.254 pp | 0.2744 | — | — | 0.7780 | 0.7914 |
| cosine k=10 q=0.1 | -0.009 pp | -0.010 pp | 0.2745 | 0.6677 | 0.6936 | 0.7806 | 0.8057 |
| snn k=40 q=0.3 | -0.105 pp | -0.041 pp | 0.2541 | 0.7546 | 0.7320 | 0.8439 | 0.8786 |
| local k=10 q=0.1 | -0.009 pp | 0.000 pp | 0.2741 | 0.6683 | 0.6667 | 0.7942 | 0.8014 |

Coverage changes use paired comparisons; vocabulary-subsample coverage uses only retained words. Sensitivity changes k or resolution while holding other settings fixed and uses the same solver seed. The 100 size-preserving label permutations test association with source geometry, not independent semantic correctness.

HSK and baseline-degree summaries are saved in [shortlist-strata.json](shortlist-strata.json). All 10,936 tokens have HSK metadata; 143 have ambiguous mappings. POS is unavailable for every token, so POS comparisons cannot be made. HSK levels are descriptive strata, never community labels.


Core candidates satisfying the no-regression/at-least-one-improvement rule: **0** before endpoint restrictions. Automated development decision: Development nominee frozen; fresh-seed confirmation is still required

## Archived controls

These controls separate neighbor-graph changes from retuning or reweighting the existing threshold graph. Stored thresholds 0.70, 0.725 and 0.75 correspond to unshifted cosine cutoffs 0.40, 0.45 and 0.50. The unshifted controls retain exactly the corresponding edges, change their weights and retune resolution. Expansions are retained in the CSV.

| Configuration | Groups | Cohesion | Silhouette | Margin | Repeat ARI | Edge ARI | Subsample ARI | Coverage | Largest | Graph setup s | Solver s |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| seeded baseline | 1339 | 0.4401 | 0.0625 | 0.0400 | 0.7703 | 0.7880 | 0.7227 | 98.45% | 170 | 0.71 | 0.89 |
| control t=0.7 stored r=0.05 | 529 | 0.3524 | 0.0145 | 0.0097 | 0.7793 | 0.8172 | 0.7030 | 99.14% | 455 | 0.71 | 1.41 |
| control t=0.7 stored r=0.1 | 841 | 0.3941 | 0.0400 | 0.0267 | 0.7288 | 0.7860 | 0.7204 | 98.88% | 271 | 0.71 | 1.41 |
| control t=0.7 unshifted q=0.01 | 199 | 0.2522 | -0.0879 | -0.0681 | 0.8376 | 0.8372 | 0.7608 | 99.20% | 1550 | 0.72 | 0.77 |
| control t=0.7 unshifted q=0.03 | 312 | 0.3038 | -0.0248 | -0.0188 | 0.8339 | 0.8409 | 0.7239 | 99.26% | 897 | 0.72 | 0.91 |
| control t=0.7 unshifted q=0.1 | 660 | 0.3735 | 0.0309 | 0.0208 | 0.7334 | 0.8042 | 0.7171 | 99.07% | 359 | 0.72 | 1.25 |
| control t=0.7 unshifted q=0.3 | 1372 | 0.4452 | 0.0700 | 0.0446 | 0.7666 | 0.7892 | 0.7266 | 98.36% | 160 | 0.72 | 0.94 |
| control t=0.725 stored r=0.05 | 1024 | 0.3999 | 0.0192 | 0.0119 | 0.7651 | 0.8104 | 0.7169 | 97.31% | 225 | 0.58 | 0.66 |
| control t=0.725 stored r=0.1 | 1477 | 0.4439 | 0.0562 | 0.0356 | 0.8113 | 0.8073 | 0.7450 | 96.82% | 154 | 0.58 | 0.64 |
| control t=0.725 stored r=0.2 | 2169 | 0.4903 | 0.0839 | 0.0517 | 0.7796 | 0.8062 | 0.7403 | 95.75% | 107 | 0.58 | 0.73 |
| control t=0.725 unshifted q=0.01 | 420 | 0.2967 | -0.0754 | -0.0569 | 0.7189 | 0.8170 | 0.7454 | 98.16% | 714 | 0.58 | 0.71 |
| control t=0.725 unshifted q=0.03 | 668 | 0.3520 | -0.0184 | -0.0142 | 0.8209 | 0.8172 | 0.7304 | 97.89% | 379 | 0.58 | 0.75 |
| control t=0.725 unshifted q=0.1 | 1232 | 0.4240 | 0.0417 | 0.0265 | 0.8004 | 0.8135 | 0.7344 | 97.06% | 186 | 0.58 | 0.56 |
| control t=0.725 unshifted q=0.3 | 2222 | 0.4964 | 0.0915 | 0.0560 | 0.7893 | 0.7899 | 0.7350 | 95.72% | 101 | 0.58 | 0.79 |
| control t=0.75 stored r=0.05 | 1829 | 0.4467 | 0.0280 | 0.0165 | 0.8354 | 0.8185 | 0.7640 | 92.89% | 136 | 0.53 | 0.41 |
| control t=0.75 stored r=0.1 | 2451 | 0.4931 | 0.0686 | 0.0425 | 0.8029 | 0.8162 | 0.7637 | 91.49% | 91 | 0.53 | 0.33 |
| control t=0.75 stored r=0.2 | 3267 | 0.5393 | 0.1023 | 0.0624 | 0.7943 | 0.8103 | 0.7638 | 89.64% | 70 | 0.53 | 0.37 |
| control t=0.75 unshifted q=0.01 | 1045 | 0.3415 | -0.0863 | -0.0654 | 0.8619 | 0.8088 | 0.7527 | 93.99% | 334 | 0.54 | 0.34 |
| control t=0.75 unshifted q=0.03 | 1408 | 0.4017 | -0.0173 | -0.0144 | 0.8429 | 0.8176 | 0.7371 | 93.59% | 193 | 0.54 | 0.34 |
| control t=0.75 unshifted q=0.1 | 2169 | 0.4759 | 0.0560 | 0.0346 | 0.8178 | 0.8002 | 0.7523 | 92.22% | 108 | 0.54 | 0.39 |
| control t=0.75 unshifted q=0.3 | 3358 | 0.5466 | 0.1104 | 0.0668 | 0.7938 | 0.8092 | 0.7757 | 89.62% | 69 | 0.54 | 0.43 |

Archived controls passing the no-regression/at-least-one-improvement rule before endpoint restrictions: control t=0.7 unshifted q=0.3.

Frozen development nominee: **control t=0.7 unshifted q=0.3**. It has not been confirmed or exported as a new default.

## Full initial core grid

The starting q values were 0.01, 0.03, 0.1 and 0.3; endpoint expansion is capped at two rounds with factor three. Every attempted expansion and archived threshold/resolution control is included in [comparison.csv](comparison.csv). Endpoint limits mean the resolution search is incomplete; they are not evidence of a best q.

| Configuration | Groups | Cohesion | Silhouette | Margin | Repeat ARI | Edge ARI | Subsample ARI | Coverage | Largest | Graph setup s | Solver s |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| cosine k=10 q=0.01 | 165 | 0.3110 | 0.0428 | 0.0306 | 0.7119 | 0.6805 | 0.6064 | 100.00% | 216 | 1.49 | 0.54 |
| cosine k=10 q=0.03 | 393 | 0.3714 | 0.0646 | 0.0434 | 0.7471 | 0.7094 | 0.6000 | 100.00% | 106 | 1.49 | 0.58 |
| cosine k=10 q=0.1 | 918 | 0.4383 | 0.0946 | 0.0591 | 0.7500 | 0.7456 | 0.6621 | 99.93% | 46 | 1.49 | 0.53 |
| cosine k=10 q=0.3 | 2018 | 0.5087 | 0.1226 | 0.0705 | 0.7206 | 0.7362 | 0.6474 | 99.40% | 27 | 1.49 | 0.43 |
| cosine k=20 q=0.01 | 76 | 0.2792 | 0.0371 | 0.0278 | 0.6830 | 0.7002 | 0.6006 | 100.00% | 333 | 1.52 | 1.23 |
| cosine k=20 q=0.03 | 214 | 0.3335 | 0.0551 | 0.0390 | 0.7349 | 0.7227 | 0.6190 | 99.99% | 200 | 1.52 | 0.98 |
| cosine k=20 q=0.1 | 574 | 0.4026 | 0.0814 | 0.0536 | 0.7057 | 0.7407 | 0.6422 | 99.95% | 97 | 1.52 | 0.91 |
| cosine k=20 q=0.3 | 1400 | 0.4720 | 0.1058 | 0.0641 | 0.7143 | 0.7368 | 0.6658 | 99.52% | 53 | 1.52 | 0.81 |
| cosine k=40 q=0.01 | 29 | 0.2417 | 0.0317 | 0.0248 | 0.6134 | 0.6794 | 0.6141 | 100.00% | 940 | 1.84 | 0.73 |
| cosine k=40 q=0.03 | 101 | 0.2973 | 0.0434 | 0.0320 | 0.7237 | 0.7519 | 0.6761 | 99.98% | 347 | 1.84 | 1.12 |
| cosine k=40 q=0.1 | 341 | 0.3636 | 0.0637 | 0.0439 | 0.6698 | 0.7432 | 0.6207 | 99.94% | 163 | 1.84 | 1.27 |
| cosine k=40 q=0.3 | 939 | 0.4337 | 0.0869 | 0.0554 | 0.6966 | 0.7303 | 0.6462 | 99.79% | 79 | 1.84 | 1.82 |
| local k=10 q=0.01 | 162 | 0.3108 | 0.0418 | 0.0299 | 0.7037 | 0.6606 | 0.5796 | 100.00% | 207 | 1.63 | 0.69 |
| local k=10 q=0.03 | 392 | 0.3707 | 0.0627 | 0.0419 | 0.7265 | 0.7065 | 0.5953 | 100.00% | 90 | 1.63 | 0.68 |
| local k=10 q=0.1 | 898 | 0.4381 | 0.0948 | 0.0592 | 0.7547 | 0.7420 | 0.6539 | 99.98% | 40 | 1.63 | 0.51 |
| local k=10 q=0.3 | 1936 | 0.5075 | 0.1239 | 0.0710 | 0.7309 | 0.7317 | 0.6583 | 99.60% | 23 | 1.63 | 0.51 |
| local k=20 q=0.01 | 76 | 0.2788 | 0.0353 | 0.0264 | 0.6747 | 0.6601 | 0.5742 | 100.00% | 362 | 1.53 | 0.83 |
| local k=20 q=0.03 | 203 | 0.3330 | 0.0549 | 0.0388 | 0.6738 | 0.6871 | 0.6081 | 100.00% | 182 | 1.53 | 1.10 |
| local k=20 q=0.1 | 553 | 0.4019 | 0.0834 | 0.0546 | 0.6959 | 0.7211 | 0.6319 | 99.98% | 77 | 1.53 | 0.80 |
| local k=20 q=0.3 | 1338 | 0.4717 | 0.1084 | 0.0654 | 0.7214 | 0.7176 | 0.6397 | 99.79% | 37 | 1.53 | 1.12 |
| local k=40 q=0.01 | 28 | 0.2415 | 0.0314 | 0.0246 | 0.6263 | 0.6538 | 0.5639 | 100.00% | 871 | 1.73 | 1.27 |
| local k=40 q=0.03 | 100 | 0.2981 | 0.0462 | 0.0341 | 0.7236 | 0.7493 | 0.6348 | 100.00% | 338 | 1.73 | 0.96 |
| local k=40 q=0.1 | 324 | 0.3648 | 0.0674 | 0.0464 | 0.6681 | 0.7142 | 0.6196 | 99.98% | 139 | 1.73 | 1.38 |
| local k=40 q=0.3 | 874 | 0.4342 | 0.0902 | 0.0571 | 0.6666 | 0.7007 | 0.6313 | 99.88% | 52 | 1.73 | 1.33 |
| snn k=10 q=0.01 | 277 | 0.3255 | 0.0119 | 0.0069 | 0.8886 | 0.8209 | 0.6201 | 99.77% | 165 | 1.55 | 0.22 |
| snn k=10 q=0.03 | 590 | 0.3798 | 0.0247 | 0.0147 | 0.8873 | 0.8586 | 0.6661 | 99.26% | 90 | 1.55 | 0.28 |
| snn k=10 q=0.1 | 1226 | 0.4416 | 0.0474 | 0.0284 | 0.9198 | 0.8699 | 0.7175 | 98.12% | 51 | 1.55 | 0.25 |
| snn k=10 q=0.3 | 2266 | 0.4995 | 0.0656 | 0.0383 | 0.9350 | 0.8923 | 0.7513 | 95.46% | 37 | 1.55 | 0.34 |
| snn k=20 q=0.01 | 132 | 0.2894 | 0.0123 | 0.0082 | 0.8147 | 0.7881 | 0.6490 | 99.88% | 311 | 1.52 | 0.55 |
| snn k=20 q=0.03 | 310 | 0.3389 | 0.0261 | 0.0173 | 0.8404 | 0.8170 | 0.6922 | 99.75% | 194 | 1.52 | 0.64 |
| snn k=20 q=0.1 | 724 | 0.4030 | 0.0469 | 0.0299 | 0.8722 | 0.8601 | 0.7264 | 99.33% | 103 | 1.52 | 0.61 |
| snn k=20 q=0.3 | 1550 | 0.4633 | 0.0600 | 0.0364 | 0.9032 | 0.8807 | 0.7775 | 97.77% | 75 | 1.52 | 0.74 |
| snn k=40 q=0.01 | 54 | 0.2490 | -0.0043 | -0.0038 | 0.8161 | 0.7806 | 0.6787 | 99.93% | 696 | 1.73 | 1.02 |
| snn k=40 q=0.03 | 153 | 0.2965 | 0.0014 | 0.0003 | 0.8556 | 0.8273 | 0.7283 | 99.80% | 323 | 1.73 | 1.06 |
| snn k=40 q=0.1 | 407 | 0.3574 | 0.0337 | 0.0228 | 0.8661 | 0.8664 | 0.7601 | 99.71% | 182 | 1.73 | 1.15 |
| snn k=40 q=0.3 | 961 | 0.4189 | 0.0466 | 0.0297 | 0.8390 | 0.8535 | 0.7422 | 98.98% | 122 | 1.73 | 1.70 |

## Fixed learning-material inspection

Exported 90 anchor/partition views from the same 18 anchors used in the archive. Several anchors may share one group; the CSV records these duplicates. See [all fixed groups](FIXED_GROUPS.md), [all member lists](fixed-anchor-review.csv), and [qualitative assessment](LEARNING_REVIEW.md). The sample deliberately includes broad themes, function words, singleton/polysemous words and unstable abstract topics. It is a convenience sample, not a random estimate of semantic accuracy.

A [supplementary control view](control-review/FIXED_GROUPS.md) uses the same anchors for the archived controls passing the development rule. It was added after the control results were known, for interpretation; it does not alter the predeclared core shortlist or its 90-view review panel.

## Costs, validity and limits

Screening wall time: 61.8 minutes. Process peak RSS: 0.82 GiB (process lifetime high-water mark). Artifacts: 14.61 GiB. Graph construction is cached within the study; runtime comparisons report construction plus mean solver time separately from metrics/export overhead.

Every valid fit has complete vocabulary assignments and connected induced communities. The stored default is checked against the archived recommendation. Source-space cohesion, silhouette and margins measure the same embedding geometry used to construct the graphs; no external labels, independent embeddings or learner outcomes are available. Coverage, concentration and robustness therefore remain separate constraints. Size bands are diagnostics, not accuracy scores.

The study also exports size-preserving null permutations, HSK/baseline-degree strata, ±20% k/resolution sensitivity, and similar-edge/community-count comparisons. Perturbation ARI measures sensitivity to graph/token removal; it does not quantify uncertainty in the original embedding training data. The many development comparisons make fresh-seed confirmation necessary before accepting a default.

## Follow-up boundaries

Use the shortlisted raw configurations as starting points for one-factor comparisons of centering, ABTT(1), ABTT(3) and mutual neighbors, retuning q for each graph. Keep the archived baseline and any passing unshifted-weight control as references. Resolve flagged resolution endpoints before nominating a default. Freeze the final variants before consuming the reserved solver seeds 1000–1009 and perturbation seeds 20000–20009; confirmation must accept or reject that frozen choice without retuning. This report does not launch those later stages.

See [shortlist.json](shortlist.json), [validation.json](validation.json), [screening outputs](screening/REPORT.md), and the reproduction commands in [README.md](README.md).
