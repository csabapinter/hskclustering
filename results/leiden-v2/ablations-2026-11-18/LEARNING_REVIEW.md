# Desk review of the threshold finalist

This reviews the fresh representative of **threshold-0.40-q0.3, seed 1008**. The frozen numerical rule qualified and selected this setting; see [DECISION.md](DECISION.md). These observations do not change the shortlist, thresholds, parameters, or numerical decision.

The 18 historical anchors and rubric come from [review-protocol.json](review-protocol.json). They were copied during current screening, before fresh confirmation. This is an author desk review of concrete memberships, not a semantic accuracy benchmark, learner evaluation, or independent validation. Source embeddings contain one vector per surface label and do not resolve the senses behind separate IDs.

## What the review compares

- The user-supplied reference: threshold-0.70 archived graph with unshifted weights, q=0.3, representative seed 1004.
- The updated-vocabulary screening representative: threshold-0.40 in raw cosine, q=0.3, seed 42.
- The same frozen setting on fresh confirmation seeds, represented by seed 1008.

[anchor-review.csv](anchor-review.csv) contains all members, numerical community labels, sizes, and comparisons for each anchor. Labels are run-specific and are not aligned by their numeric values. The anchors 牛奶/咖啡 and 老师/妈妈 share a group in all three partitions: these are repeated views of the same groups, not independent evidence.

The reference has 10,936 assigned labels; the new partitions have 10,888, with 9,688 assigned labels shared between vocabularies. The old-versus-fresh Jaccard below restricts both groups to that shared vocabulary before taking intersection over union. Removed input words and words lacking new embeddings therefore do not count as reassignment losses. The CSV lists those cases separately. New-only words are visible in full member lists but excluded from this overlap diagnostic. Screening-versus-fresh Jaccard uses the identical updated vocabulary.

| Anchor | Reference size | Screening size | Fresh size | Shared-vocabulary Jaccard | Screening–fresh Jaccard |
| --- | ---: | ---: | ---: | ---: | ---: |
| 苹果 | 4 | 1 | 1 | 0.250 | 1.000 |
| 牛奶 | 156 | 154 | 154 | 0.963 | 1.000 |
| 咖啡 | 156 | 154 | 154 | 0.963 | 1.000 |
| 衬衫 | 78 | 75 | 75 | 0.897 | 0.948 |
| 足球 | 41 | 37 | 39 | 0.811 | 0.810 |
| 老师 | 139 | 125 | 124 | 0.935 | 0.976 |
| 焦虑 | 163 | 165 | 171 | 0.938 | 0.909 |
| 政策 | 7 | 10 | 7 | 0.077 | 0.062 |
| 合同 | 15 | 14 | 15 | 1.000 | 0.933 |
| 银行 | 11 | 18 | 23 | 0.138 | 0.323 |
| 软件 | 19 | 9 | 19 | 0.895 | 0.273 |
| 感冒 | 65 | 62 | 78 | 0.541 | 0.522 |
| 火车 | 39 | 35 | 28 | 0.815 | 0.750 |
| 妈妈 | 139 | 125 | 124 | 0.935 | 0.976 |
| 为什么 | 28 | 22 | 23 | 0.909 | 0.957 |
| 公斤 | 19 | 17 | 19 | 0.900 | 0.800 |
| 花 | 12 | 9 | 9 | 0.700 | 1.000 |
| 行 | 3 | 3 | 3 | 0.500 | 0.500 |

## Recognizable themes and useful continuity

**Food and drink:** 牛奶 and 咖啡 remain together in a 154-word group, close to the reference’s 156. 牛奶、茶、面包、米饭 and 海鲜 preserve a recognizable food theme; the changed vocabulary adds 奶酪、烹饪、甜品、螃蟹. This retains the broad topic-level character the user found useful, though it does not separate beverages, ingredients, and meals.

**Agreements and signing:** 合同 stays in a compact 15-word group with 协议、契约、签约、签署、生效. New 备忘录 fits the theme. All reference members still present in the updated assigned vocabulary remain with 合同; missing 协议书 is a vocabulary change.

**Questions and measurement:** 为什么 remains with 为何、怎么、谁、哪儿 in a 23-word interrogative group. 公斤 has 19 members with 克、千克、斤、吨、厘米、毫米 and volume/area units. These preserve useful grammatical or functional neighborhoods, with boundary issues described below.

**Sports and travel:** 足球 remains in a 39-word sports/competition group with 篮球、网球、运动员 and additions 比赛、联赛、选手. 火车 keeps a 28-word rail/passenger core around 列车、地铁、高铁、站台 and 旅客. Their wider boundaries still span other competitive activities and air travel.

**Computing:** 软件’s fresh 19-word group is close in nature to the old 19-word group: 电脑、硬件、内存、服务器、键盘、鼠标 and 浏览器. The screening representative’s smaller 编程/数据库 neighborhood and separate browser/web group did not survive in this representative, so they should not be claimed as confirmed improvements.

## Breadth, intrusive members, and seed-sensitive boundaries

**Banking broadens into documents:** 银行 grows from 11 to 23 members. 存款、储蓄、汇款、账户、银行卡 are useful banking neighbors, but 护照、身份证 and 病历 make this a banking-and-documents mix. The screening neighborhood was more focused on account operations. In the fresh fit, retained 余额、信贷 and 贷款 sit together elsewhere. Similar global size distributions do not guarantee the same local neighborhood.

**Policy changes frame:** 政策 now groups with 制度、体制、体系、改革、市场经济、秩序. The reference instead grouped it with 方针、策略、战略、举措、决策、部署, and screening was closer to that earlier strategy/planning frame. Both frames have plausible thematic connections; the complete replacement of these neighbors is a concrete local sensitivity rather than an established semantic improvement.

**Illness merges with healthcare:** 感冒 grows from 65 reference members (62 in screening) to 78, incorporating 医生、医院、治疗、手术 and 门诊. That is a recognizable healthcare topic, but retained 发烧、发热、呕吐 now form a separate symptom group. Applications expecting illness/symptom practice sets may prefer some neighbors that this representative separates.

**Large groups remain broad:** 焦虑’s group reaches 171 members, spanning positive and negative emotions and reactions. New 不知所措、哽咽、郁闷 fit broadly, while 心里 and 自言自语 show its looser edge. 妈妈 and 老师 still share a 124-word people/kinship group with 父母、儿子、女儿 but also 老板、仇人 and 小偷儿. 老师 remains separate from 学校、学生 and 教师; the familiar spoken term is not placed in the education group.

**Clothing retains intrusions:** 衬衫’s 75-word group has a clear clothing core—衣服、外套、裤子、汉服、袖子—but also household accessories and 铅笔. This is useful as a broad possessions/clothing neighborhood and weak as a narrowly curated clothing lesson.

**Small and ambiguous cases:** 苹果 is a singleton in both updated representatives. The reference’s 苹果/新款/系列/问世 group reflected a brand/product association; all three former neighbors remain available but move elsewhere, without recovering a fruit reading. 花 retains a small floral core with 玫瑰、牡丹、菊花 and 茉莉, yet 桃 is ambiguous and retained 花瓣/荷花 move to a broad plants group. 公斤 gains 元 among physical units, while retained 立方 is a singleton. 为什么 picks up contextual 知道. 行 changes from 哄/瞒/行 to 哄/行/轰, an opaque small group that illustrates unresolved polysemy and local boundaries.

## Interpretation

The finalist retains the reference’s mixture of compact functional groups and much larger topic groups, with concrete examples of both useful continuity and weak membership boundaries. The fresh representative also changes several anchor neighborhoods relative to screening. These observations provide context for inspection in downstream applications; they do not supply an accuracy rate or override the frozen distribution, coverage, connectivity, repeatability, and perturbation checks.

## Reviewed artifact hashes

- review protocol: `results/leiden-v2/ablations-2026-11-18/review-protocol.json` — SHA-256 `2657c12355c70a89f80331d78b44a44e577c95b1ed70efd2eb957c1c401ac716`
- reference: `results/leiden-v2/ablations-2026-09-26/confirmation/confirmation/b58dfe04de33ba8c519a49ab8641c0cfd99a5c4022b6a198e1ffe7db1adade67/communities.csv` — SHA-256 `615fe49c2c3a6b6022265d80a09e41a29d75ca25bad0ce8c05ee1c0792c4c30b`
- screening: `results/leiden-v2/ablations-2026-11-18/screening/candidates/threshold-0.40-q0.3/communities.csv` — SHA-256 `29c7df03b05cf594fa6919b060eab2191a4ec7b1e11817976e542bfab4abfa9d`
- fresh: `results/leiden-v2/ablations-2026-11-18/confirmation/candidates/threshold-0.40-q0.3/communities.csv` — SHA-256 `dcbcd860bff0bd367ea8ecd28af5531b5c1bae4d9d02ef5df8cbd0f82976ec00`
