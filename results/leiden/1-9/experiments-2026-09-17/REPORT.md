# HSK 1–9 clustering experiments — 2026-09-17

**Recommended starting point: original 300D vectors, weight threshold 0.70, CPM resolution 0.20, gamma 0.20, theta 0.30.** Raising resolution gives much more usable groups without discarding additional graph edges. This is a practical choice balancing coverage and granularity, not a claim of a uniquely correct partition.

Completed **31 new Leiden runs**: a nine-setting raw-vector grid, six PCA screening runs, and four additional runs for each of four finalists. All 10,936 input words were retained in every partition. Existing baseline files, the 1–3 archive, input data and application source were left unchanged.

## Start here

- [Recommended assignments](recommended.communities.csv): exactly one community per word.
- [Recommended cluster catalogue](recommended-clusters.csv): size, representative words, a deterministic sample, boundary words, all members and repeat stability.
- [Word details](recommended-word-details.csv): HSK levels, pinyin, dictionary translations, similarity diagnostics and repeat agreement.
- [All run measurements](run-summary.csv), [configuration averages](config-summary.csv), [graph measurements](graph-summary.csv), [repeat stability](stability-summary.csv), [fixed-anchor comparisons](anchor-review.csv).

The exported partition is `raw-t0.7-r0.2-run05`: **1,372 communities**, largest **175**, **186 singletons**. It was chosen as the most representative of its five repeats (highest mean adjusted Rand agreement with the other four), rather than because it happened to have a favourable score. Canonical community IDs apply only to this partition.

## What improved

The saved baseline has 508 communities, a largest community of 450, and 34.7% of words in groups of 3–30. The recommended configuration averages about **1,351 communities, a largest community of 172, 176 singletons, and 69.8% in groups of 3–30**. The fraction in groups larger than 100 drops from 29.6% to 5.3%. Groups of 3–30 are a study-size diagnostic, **not a semantic accuracy score**.

Threshold 0.725 with resolution 0.10 is also reasonable: slightly tighter groups, but about 330 singletons versus 176 for the recommendation. Threshold 0.725 / resolution 0.20 is useful if small groups are preferred; approximately 11.3% of words end up in singletons or pairs. Threshold 0.75 / resolution 0.20 fragments the vocabulary heavily: 1,112 singletons and 21.8% of words in singletons or pairs in its screening run. I would not use that as the default despite its higher silhouette score.

## How the groups look

These examples refer to the recommended partition. Representative words describe the centre; the catalogue also includes less flattering boundary words and a fixed sample. The examples below are qualitative spot-checks, not a labelled validation set.

| Theme / community | Size | Actual members | Assessment |
|---|---:|---|---|
| Sports / 281 | 40 | 乒乓球, 篮球, 网球, 羽毛球, 排球, 举重, 滑冰 | Clear thematic core; boundaries include exercise, equipment and outdoor activities. |
| Agreements / 762 | 16 | 协议, 签订, 签署, 协议书, 合同, 协定, 契约, 生效 | Useful, fairly compact topic; mixes objects and associated actions as expected for context embeddings. |
| Media and entertainment / 436 | 5 | 影视, 娱乐, 传媒, 文娱, 文化 | A compact useful group, although 文化 is broader than the others. |
| Calm / pleasant states / 881 | 41 | 安逸, 悠闲, 快乐, 安稳, 愉快, 宁静, 舒畅 | Readable theme, covering several related emotional states rather than synonyms only. |
| Questions / 159 | 27 | 怎么, 怎样, 什么, 究竟, 哪里, 哪儿, 谁, 为什么 | Strong grammatical/contextual group; very stable across repeats. |
| Flowers / 1216 | 10 | 菊花, 桃花, 牡丹, 梅花, 玫瑰, 百合, 鲜花, 花, 芦花, 绣 | Mostly coherent; 绣 is a contextual outlier. |
| Food / 69 | 165 | 米饭, 豆腐, 香肠, 豆浆, 饺子, 薯条, 酸奶, 粥 | Clear theme but too broad as one study unit; also contains utensils and cooking-related words. |
| Family and people / 26 | 140 | 姑姑, 舅舅, 姐姐, 妹妹, 爸爸, 妈妈; also 老师 | Stable but overinclusive. 老师 illustrates that contextual association is not the same as the desired dictionary category. |
| Emotional reactions / 128 | 175 | 沮丧, 羞愧, 气愤, 惊讶, 惋惜, 难过, 伤心 | Strong centre but still too large; mixes emotion, reaction and narrative context. |

Specific limitations recur across settings:

- **Polysemy/corpus usage:** 苹果 is a singleton in the recommendation but joins 新款 / 系列 in several other settings, consistent with a brand association. 花 moves between flower-related and money/spending-related groups across settings. 行 is a singleton. One vector per spelling cannot separate these senses.
- **Unstable abstract topics:** the recommended computing group (软件 / 硬件 / 计算机 / 芯片 etc.) has mean best-match Jaccard only 0.30 across repeats; policy/strategy is 0.32 and thinking/deliberation 0.27. Treat these boundaries as provisional.
- **Stable does not imply suitable:** food and family/person groups have Jaccard about 0.98 and 0.97 respectively, yet remain broad or overinclusive. A second, finer pass within the few oversized themes is a more targeted next experiment than globally fragmenting every topic.

Advanced vocabulary does not collapse into noise. In the recommended exported run, **89 of 5,592 earliest-level 7–9 spellings are singletons (1.6%)**, and **71.1% are in groups of 3–30**. Level is a descriptive coverage attribute, not a semantic ground-truth label; see [level coverage](level-coverage.csv).

## Five-run stability and tradeoffs

Numbers below average the five runs, except the ARI range, which covers their ten pairwise comparisons. ARI 1 means identical partitions; values around 0.73 indicate substantial residual assignment variability. These repeats measure algorithm randomness on a fixed graph, not robustness to new data or a different embedding model.

| Representation | Threshold | Resolution = gamma | Communities | Singletons | Words in 3–30 | Mean ARI | ARI range |
|---|---:|---:|---:|---:|---:|---:|---|
| PCA 200D | 0.686 | 0.1 | 1288 | 232 | 71.7% | 0.729 | 0.706–0.754 |
| Original 300D | 0.7 | 0.2 | 1351 | 176 | 69.9% | 0.728 | 0.709–0.760 |
| Original 300D | 0.725 | 0.1 | 1457 | 330 | 72.4% | 0.730 | 0.698–0.759 |
| Original 300D | 0.725 | 0.2 | 2175 | 470 | 77.6% | 0.751 | 0.740–0.772 |

## PCA findings

100D and 200D PCA retain **56.0% and 83.3%** of the centred raw-vector variance respectively. PCA was fitted on all 1–9 raw SGNS vectors, without whitening or per-coordinate standardization, then vectors were L2-normalized. The PCA thresholds were recalibrated to approximately the same edge count as the raw 0.725 graph: raw 127,234 edges; PCA100 126,098 at 0.737; PCA200 126,971 at 0.686. The same numerical threshold would have confounded geometry changes with a large change in sparsity.

100D lost substantial information and had weaker original-space cohesion and silhouette at comparable sparsity. 200D was competitive: fewer singletons than raw 0.725, and sensible topical cores, but no clear overall semantic or stability gain over the raw finalists. The recommendation therefore keeps the original vectors. This was a small exploratory comparison, not an exhaustive PCA search. The raw-space diagnostics favour preserving the original geometry and are not independent semantic labels. PCA here also changes centring; that effect was not isolated in a separate centred-300D clustering control.

**Numerical finding:** the direct 200-component Linfa fit returned orthonormal directions with a relative eigen residual of **0.350**, so it was not an accurate principal-component solution. That graph was excluded from clustering. Fitting all 300 directions and keeping the leading 200 gives residual **8.54e-10**, and these are the 200D results reported above. The direct 100D fit passed (4.26e-13) and matched the full-fit variance/edge count. The audit files and rejected projection are preserved for traceability; no application/dependency code was changed. `pca200` is rejected, `pca200-fullfit` is the valid variant. Retained variance was computed from actual projected squared energy relative to total centred squared energy.

## Screening grid

One new run per configuration; repeated-run averages are in the preceding table and CSVs. The original saved baseline had 508 communities; its new stochastic rerun below has 518. Every run uses theta 0.30 and gamma equal to CPM resolution. Silhouette uses cosine distance in the original normalized 300D space for every representation; singletons receive zero.

| Representation | Threshold | Resolution | Communities | Largest | Singletons | Words in 3–30 | Mean cosine silhouette |
|---|---:|---:|---:|---:|---:|---:|---:|
| Original 300D | 0.7 | 0.05 | 518 | 455 | 76 | 32.7% | 0.017 |
| Original 300D | 0.7 | 0.1 | 827 | 296 | 102 | 53.3% | 0.041 |
| Original 300D | 0.7 | 0.2 | 1355 | 179 | 171 | 71.1% | 0.059 |
| Original 300D | 0.725 | 0.05 | 978 | 251 | 257 | 57.4% | 0.022 |
| Original 300D | 0.725 | 0.1 | 1446 | 153 | 309 | 72.7% | 0.057 |
| Original 300D | 0.725 | 0.2 | 2166 | 108 | 477 | 78.4% | 0.081 |
| Original 300D | 0.75 | 0.05 | 1792 | 164 | 750 | 72.1% | 0.026 |
| Original 300D | 0.75 | 0.1 | 2411 | 94 | 870 | 78.5% | 0.068 |
| Original 300D | 0.75 | 0.2 | 3256 | 70 | 1112 | 74.7% | 0.101 |
| PCA 100D | 0.737 | 0.05 | 922 | 275 | 235 | 53.2% | 0.004 |
| PCA 100D | 0.737 | 0.1 | 1377 | 186 | 305 | 68.7% | 0.024 |
| PCA 100D | 0.737 | 0.2 | 2069 | 136 | 435 | 75.6% | 0.048 |
| PCA 200D | 0.686 | 0.05 | 847 | 223 | 173 | 53.3% | 0.024 |
| PCA 200D | 0.686 | 0.1 | 1274 | 156 | 219 | 72.2% | 0.054 |
| PCA 200D | 0.686 | 0.2 | 1950 | 105 | 350 | 80.4% | 0.076 |

## Mathematical and validation notes

- Stored edge weight is `(cosine + 1)/2`; raw thresholds 0.70 / 0.725 / 0.75 correspond to cosine 0.40 / 0.45 / 0.50. Their mean degrees are 55.46 / 23.27 / 10.23, and graph isolates are 3 / 89 / 559. Graph isolates and singleton communities are different counts.
- CPM maximizes `sum(internal edge weights) - resolution * sum(n_c*(n_c-1)/2)`. Missing edges count as zero. Its objective is comparable between repeated partitions only when graph and resolution are identical. It is not a semantic accuracy measure.
- Within-community mean pairwise cosine is computed exactly from sums of unit vectors; own-cluster averages exclude self. Cosine silhouette uses each word's average distance to all members of its own and each other cluster. Centroid identities were checked against direct pairwise calculations. Reported mean cohesion weights nonsingleton communities by their word counts, not by their pair counts.
- Every saved partition was checked against all 10,936 graph nodes, with no duplicate or missing assignments; every community induces a connected subgraph, including singleton communities. PCA basis orthonormality and eigen residuals were checked separately.
- Fixed-anchor comparisons, deterministic within-cluster samples and boundary words help avoid inspecting only attractive cluster centres. No numeric claim of semantic purity is made. The old 1–3 “72% thematic” observation is not directly comparable to any size/coverage percentage here.
- The upstream Leiden RNG does not expose a seed. Saved assignments are the record of each random run. Cluster/word repeat-match scores are empirical agreement diagnostics, not probabilities of semantic correctness.

See [reproduction and file guide](README.md) for commands, file roles and provenance.
