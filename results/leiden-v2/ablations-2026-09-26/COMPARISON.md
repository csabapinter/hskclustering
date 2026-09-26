# Focused k=10 graph ablations — 2026-09-26

## Decision

- **center**: do not adopt; retain as a measured trade-off. Confirmation regressions versus raw cosine: none.
- **abtt1**: keep experimental improvement. Confirmation regressions versus raw cosine: none.
- **abtt3**: do not adopt; retain as a measured trade-off. Confirmation regressions versus raw cosine: cohesion, silhouette, margin.
- **mutual**: do not adopt; retain as a measured trade-off. Confirmation regressions versus raw cosine: margin, nonsingleton_coverage.

Frozen default nominee confirmed under the unchanged archived-baseline rule: **True**. Archived files are unchanged; this report does not silently install a different default.

## Fresh confirmation

Ten reserved solver seeds and ten paired perturbations of each kind per frozen setting. Values below are cohort medians; Largest is the median largest-community size, not a hard cap.

| Configuration | Groups | Cohesion | Silhouette | Margin | Repeat ARI | Edge ARI | Subsample ARI | Coverage | Largest | Graph setup s | Solver s |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| local k=10 q=0.1 | 898 | 0.4384 | 0.0950 | 0.0592 | 0.7501 | 0.7359 | 0.6459 | 99.97% | 42 | 1.36 | 0.47 |
| cosine k=10 mutual q=0.1 | 1674 | 0.4794 | 0.0953 | 0.0555 | 0.8243 | 0.7810 | 0.6839 | 98.74% | 20 | 1.49 | 0.30 |
| cosine k=10 q=0.1 | 926 | 0.4389 | 0.0944 | 0.0589 | 0.7539 | 0.7489 | 0.6575 | 99.94% | 46 | 1.42 | 0.58 |
| center cosine k=10 q=0.1 | 930 | 0.4399 | 0.0957 | 0.0596 | 0.7791 | 0.7583 | 0.6734 | 99.95% | 57 | 1.55 | 0.60 |
| abtt(1) cosine k=10 q=0.1 | 936 | 0.4388 | 0.0931 | 0.0580 | 0.7865 | 0.7694 | 0.6816 | 99.92% | 52 | 1.54 | 0.53 |
| control t=0.7 unshifted q=0.3 | 1368 | 0.4449 | 0.0693 | 0.0442 | 0.7683 | 0.8115 | 0.7546 | 98.39% | 164 | 0.73 | 1.22 |
| seeded baseline | 1344 | 0.4397 | 0.0614 | 0.0393 | 0.7456 | 0.7791 | 0.7355 | 98.42% | 170 | 0.67 | 1.12 |
| abtt(3) cosine k=10 q=0.1 | 934 | 0.4373 | 0.0908 | 0.0566 | 0.7883 | 0.7716 | 0.6797 | 99.95% | 54 | 1.87 | 0.52 |

## Development shortlist

The raw cosine reference and local comparator were fixed at q=0.1; each ablation's q was selected by the sealed rule before confirmation.

| Configuration | Groups | Cohesion | Silhouette | Margin | Repeat ARI | Edge ARI | Subsample ARI | Coverage | Largest | Graph setup s | Solver s |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| seeded baseline | 1339 | 0.4401 | 0.0625 | 0.0400 | 0.7703 | 0.7880 | 0.7227 | 98.45% | 170 | 0.69 | 0.88 |
| control t=0.7 unshifted q=0.3 | 1372 | 0.4452 | 0.0700 | 0.0446 | 0.7666 | 0.7892 | 0.7266 | 98.36% | 160 | 0.70 | 0.93 |
| cosine k=10 q=0.1 | 918 | 0.4383 | 0.0946 | 0.0591 | 0.7500 | 0.7456 | 0.6621 | 99.93% | 46 | 1.39 | 0.53 |
| local k=10 q=0.1 | 898 | 0.4381 | 0.0948 | 0.0592 | 0.7547 | 0.7420 | 0.6539 | 99.98% | 40 | 1.48 | 0.49 |
| center cosine k=10 q=0.1 | 931 | 0.4398 | 0.0952 | 0.0592 | 0.7859 | 0.7585 | 0.6732 | 99.95% | 57 | 1.40 | 0.58 |
| abtt(1) cosine k=10 q=0.1 | 939 | 0.4391 | 0.0918 | 0.0572 | 0.7981 | 0.7631 | 0.6805 | 99.92% | 47 | 2.00 | 0.41 |
| abtt(3) cosine k=10 q=0.1 | 935 | 0.4372 | 0.0899 | 0.0559 | 0.7987 | 0.7712 | 0.6847 | 99.94% | 48 | 1.70 | 0.58 |
| cosine k=10 mutual q=0.1 | 1679 | 0.4799 | 0.0951 | 0.0555 | 0.8152 | 0.7840 | 0.6893 | 98.73% | 22 | 1.40 | 0.26 |

## Comparisons at similar granularity

Closest median community counts to raw cosine q=0.1; matches outside 10% are explicitly marked in count-matched.csv. This diagnoses whether apparent gains simply reflect finer partitioning.

| Configuration | Groups | Cohesion | Silhouette | Margin | Repeat ARI | Edge ARI | Subsample ARI | Coverage | Largest | Graph setup s | Solver s |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| cosine k=10 q=0.1 | 918 | 0.4383 | 0.0946 | 0.0591 | 0.7500 | 0.7456 | 0.6621 | 99.93% | 46 | 1.39 | 0.53 |
| center cosine k=10 q=0.1 | 931 | 0.4398 | 0.0952 | 0.0592 | 0.7859 | 0.7585 | 0.6732 | 99.95% | 57 | 1.40 | 0.58 |
| abtt(1) cosine k=10 q=0.1 | 939 | 0.4391 | 0.0918 | 0.0572 | 0.7981 | 0.7631 | 0.6805 | 99.92% | 47 | 2.00 | 0.41 |
| abtt(3) cosine k=10 q=0.1 | 935 | 0.4372 | 0.0899 | 0.0559 | 0.7987 | 0.7712 | 0.6847 | 99.94% | 48 | 1.70 | 0.58 |
| cosine k=10 mutual q=0.03 | 847 | 0.4088 | 0.0492 | 0.0297 | 0.7991 | 0.7295 | 0.6290 | 99.25% | 48 | 1.40 | 0.37 |

## Graph structure

Edge overlap is measured against raw cosine k=10. Directed-neighbor statistics describe the lists before union/mutual filtering; mutual pruning therefore leaves their values unchanged. Weight scales differ by representation/kernel and are not quality scores.

| Graph | Edges | Isolates | Components | Max degree | Incoming-neighbor max | Edge Jaccard with raw |
|---|---:|---:|---:|---:|---:|---:|
| cosine | 74756 | 0 | 1 | 53 | 53 | 1.0000 |
| local | 74756 | 0 | 1 | 53 | 53 | 1.0000 |
| center | 73144 | 0 | 1 | 41 | 41 | 0.8820 |
| abtt1 | 73070 | 0 | 1 | 37 | 37 | 0.8085 |
| abtt3 | 72384 | 0 | 1 | 38 | 38 | 0.7562 |
| mutual | 34604 | 81 | 91 | 10 | 53 | 0.4629 |

The original 18 anchors' full top-10 directed lists are in neighbor-review.csv. Cosines/distances there use each graph's fitted representation, so their magnitudes are not directly comparable across representations. Local weighting and mutual pruning reuse raw neighbor rankings; their effect is in edge weights or retention.

## Scope and interpretation

Six graphs: raw cosine/local union, centered cosine, ABTT(1) cosine, ABTT(3) cosine, and raw mutual cosine; all k=10. Only one factor changes at a time. The archived shifted-weight baseline and unshifted threshold-0.70 control provide continuity with the earlier study. No combinations, new embedding model or solver algorithm change were tested.

Each graph starts with q=0.01, 0.03, 0.1, 0.3 and has up to two endpoint expansions. All attempted q values, endpoint flags, source-space coherence, coverage, size bands, repeats and perturbations are retained in development-comparison.csv. Full graph diagnostics include isolates, components, reciprocity, incoming-neighbor skewness and degree/weight distributions.

The existing baseline calibration is rerun on this build. Its tolerances remain descriptive sensitivity scales. Applying them to raw-cosine comparisons is an explicit conservative common yardstick, not a kNN-specific uncertainty estimate. All seven metrics and the existing coverage/concentration guards must pass in both development and confirmation to support an experimental improvement. Mixed trade-offs do not displace the raw reference.

The same 18-anchor desk review is exported in FIXED_GROUPS.md and fixed-anchor-review.csv. It supplies qualitative context, not a human benchmark or an estimated accuracy rate. Source-space metrics reuse the graph's input embeddings. Neither stable partitions nor higher silhouette independently establish semantic usefulness; single-vector polysemy persists.

See analysis-protocol.json and protocol-seal.json for pre-fit rules; confirmation-plan.json and shortlist.json for the frozen subset; decisions.json for the machine-readable outcome; validation.json and confirmation-validation.json for artifact audits; LEARNING_REVIEW.md for the example-group inspection.
