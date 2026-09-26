# Fixed-anchor review of the k=10 ablations

This is an assistant desk review of the same 18 anchors fixed before the study.
It is not blind human feedback, learner evaluation, or an estimate of semantic
accuracy. It does not change the sealed quantitative acceptance rule. The full
directed neighbor lists are in [neighbor-review.csv](neighbor-review.csv), and
fresh-confirmation groups are in [FIXED_GROUPS.md](FIXED_GROUPS.md).

## What changed in the direct neighbors

Centering retains 92.7% of the raw graph's edges; ABTT(1) retains 88.4% and
ABTT(3) 84.8%. All three preserve a connected graph with no isolates. Their
incoming-neighbor maxima fall from 53 to 41, 37 and 38 respectively. These are
structural changes, not semantic quality scores.

- **老师:** ABTT(1)'s top ten include 教授、导师、学生、讲课, replacing several
  kinship terms in the raw list. This is a concrete example of a more focused
  education context. ABTT(3) similarly adds 师生 and 师资.
- **政策 and 为什么:** the leading neighbors are largely unchanged or reordered.
  The transformations do not fundamentally improve every neighborhood.
- **苹果:** fruit terms still mix with 手机、新款、芯片. **花:** spending terms
  still mix with 花瓣、盛开、玫瑰. The single vector per spelling continues to
  combine senses.
- **银行:** the ABTT lists replace 旅行社 with 行. This removes one broad
  association but introduces an ambiguous single-character spelling; it is not
  an unqualified improvement.
- **公斤:** mass, area, distance and volume units remain mixed. ABTT(3) adds 元
  to the top ten, further widening the context to currency.

Local weighting reuses the raw directed neighbors and graph edges. Mutual
pruning also reuses the raw rankings but retains only 46.3% of the union edges,
leaving 81 isolates in 91 components. The 847-group mutual setting is within 10%
of the raw reference's 918 groups during development, yet its source-space
silhouette is 0.0492 versus 0.0946. Its coherence losses are therefore visible
even at broadly comparable granularity.

## Fresh representative groups

The raw representative is solver seed 1004; centering and ABTT(1) use seed 1000.
These are each the run with highest mean agreement to the other nine repeats,
not seeds chosen for attractive examples. Counts below are representative-group
sizes, distinct from the cohort medians in the quantitative report.

| Anchor | Raw / center / ABTT(1) size | Observation |
|---|---:|---|
| 老师 | 46 / 13 / 12 | Raw shares a family/person group with 妈妈. ABTT(1) yields a clear school group: 中学生、同学、大学生、学员、学子、学生、小学生、师生、师资、教师、留学生、老师. This is the strongest practical improvement in the fixed panel. |
| 妈妈 | 46 / 64 / 38 | ABTT(1) separates the family context from teachers. Centering alone leaves a larger family unit, showing that improved average scores do not uniformly shrink lessons. Raw duplicates the 老师 group. |
| 政策 | 9 / 8 / 14 | Raw has 一揽子、举措、决策、对策、战略、措施、政策、策略、部署. ABTT(1) switches to tightening/loosening, including 中性、宽松、松弛、紧缩、调控、麻痹. This is a substantial practical regression for a general policy lesson. |
| 苹果 | 33 / 33 / 35 | All use a broad fruit/vegetable/ingredient context. ABTT(1) adds 大米 and 蒜; the result remains readable but is wider, not a cleaner fruit category. |
| 牛奶 / 咖啡 | 31 / 31 / 33 | These anchors share one drinks-related group. ABTT(1) adds 乳制品、沏、罐头 and removes 酒精. 果酱 and 酒鬼 remain. No clear overall teaching improvement. |
| 焦虑 | 22 / 27 / 15 | ABTT(1) narrows fatigue/dejection vocabulary and adds 不安, but still mixes anxiety, depression and tiredness. More manageable, not a precise synonym set. |
| 银行 | 12 / 13 / 13 | ABTT(1) adds 账户 to an otherwise nearly unchanged banking/money group. The ambiguous 宝 remains. |
| 软件 | 22 / 21 / 21 | ABTT(1) removes 框, but the group still covers hardware, peripherals, software and computing. A small boundary improvement, not a software-specific lesson. |
| 衬衫 | 46 / 46 / 46 | Clothing remains a large unit, with no size reduction. |
| 足球 | 30 / 29 / 29 | Only a small size change in the sports unit. |
| 合同 | 20 / 20 / 19 | The agreement/contract context remains useful with little size change. |
| 感冒 | 25 / 28 / 25 | Similar breadth across symptoms and conditions remains. |
| 火车 | 24 / 24 / 25 | A broad transport context remains. |
| 公斤 | 26 / 26 / 26 | Mixed measurement units remain; no granularity gain. |
| 为什么 | 12 / 9 / 11 | All retain a contextual mix of questions, causes and inference. These require an appropriate functional lesson label. |
| 花 | 15 / 13 / 11 | Raw groups it with money/accumulation; centering preserves a flower group. ABTT(1) uses expenditure/consumption, including 浪费、消耗、耗费、节省、节约. The representation changes which sense the lesson supports. |
| 行 | 2 / 5 / 3 | Raw gives 哄、行; ABTT(1) adds 绷. Coverage of this spelling still does not produce an obvious useful lesson. |

ABTT(1)'s supported advantage is resistance to edge removal while preserving
the other quantitative scores within tolerance. The panel supports keeping it
as an experimental representation option, with concrete improvements and
regressions visible. It does not support claiming uniformly cleaner or smaller
learning units. Centering is directionally positive in all seven confirmation
scores, but none clears the frozen improvement tolerance; that is an unresolved
small gain rather than evidence that centering is harmful.

## Other confirmed comparisons

ABTT(3) also produces a school group around 老师, but retains the same problematic
tightening/loosening context for 政策. Its 妈妈 group grows to 58 words and its
行 group mixes 娇惯、宠、哄、惯、娇气、宠爱、绷、行. There is no practical case in
these panels to offset its confirmed coherence losses relative to raw cosine.

Mutual pruning produces some compact readable units: 15 clothing words around
衬衫, 18 sports words around 足球, and separate 12-word 牛奶 and 8-word 咖啡
groups. But 软件 ends up with 计算机、机械、机器、仪表, 苹果 follows 新款、款式,
and 行 is isolated. Smaller groups alone do not establish better study material.

The confirmed unshifted-threshold control passes the separate archived-baseline
numerical rule, but its representative still has a 156-word food group,
139-word people/family group, and 163-word emotional-reaction group. It remains
a useful reference rather than the answer to the large-group priority explored
here. The archived files were not replaced by this experiment.
