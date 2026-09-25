# Fixed sample: usefulness as learning material

The cosine and local **k=10, q=0.1** configurations produce several more
manageable study units than the archived baseline. They also introduce weak
associations and sense changes. The sample supports further ablations of these
configurations; it does not establish an overall semantic winner or justify
replacing the baseline. Their weaker perturbation agreement remains a real
measured trade-off.

This is an assistant desk review, not learner feedback or a blind human rating.
The 18 anchors were copied from the earlier archive before screening. I inspected
the fixed center/sample/boundary panels and the small-group memberships for the
archived assignment, common-seed baseline representative and all three shortlisted
core representatives. These are 90 anchor/partition views, with duplicate groups
identified in the CSV. No semantic accuracy percentage is inferred. The review
did not change the numeric policy or shortlist.

See [the complete panels](FIXED_GROUPS.md) and [all members](fixed-anchor-review.csv).
Group sizes below refer to the representative partitions, not cohort medians.

## What the fixed anchors show

| Anchor(s) | Observed comparison | Learning-material assessment |
|---|---|---|
| 苹果 | Archived singleton; fresh baseline joins 新款 / 系列 / 问世. Cosine/local place it among fruit and vegetables (33/31 words); SNN uses a broader 48-word ingredients group. | The core representatives recover a food context here, but the units mix fruit, vegetables and other ingredients. A favorable example, not a solution to polysemy. |
| 牛奶, 咖啡 | Both share the archived 165-word food group. Cosine gives a 31-word drinks group; local gives 21. SNN gives 52 and includes 牙膏 and 烟. | Clear practical improvement in granularity for cosine/local. Local still includes 果酱 and 罐头; cosine mixes drinking vocabulary such as 酒鬼. These need a topic label and some cleanup. The two anchors are one group, not two independent successes. |
| 衬衫 | Archived 81 words, fresh baseline 71; cosine/local 46/43, SNN 75. | Cosine/local concentrate on clothing, accessories and wearing. More manageable, although still large and not a list of garment synonyms. SNN retains broader textile/personal-item context. |
| 足球 | Archived 40 words; cosine/local 29 each, SNN 65. | Cosine/local provide a readable sports unit, including equipment and participants. SNN broadens into competitions/results, with 无缘 at the boundary. |
| 老师 | Archived/fresh baseline sit in 140/139-word family/people groups. Cosine/local yield mentor/apprentice groups of 11/9: 师父, 师傅, 徒弟, 弟子, 前辈. SNN still has 122 words. | A useful separation in cosine/local, though the theme is mentorship rather than school alone; cosine also includes 老板 and 木匠. SNN's stability does not solve this overbreadth. |
| 焦虑 | Archived 175-word emotional-reaction group. Cosine gives 15 concern/doubt words; local gives 20 fatigue/irritability words; SNN gives 44 anxiety/alarm words. | Every core setting narrows the group, but the meaning shifts. Cosine adds 异议 / 质疑; local adds 疲惫 / 劳累. These are different teaching contexts, not interchangeable labels or uniformly better categories. |
| 政策 | Archived strategy/ideas group (11); fresh baseline institutions/processes (9). Cosine gives 13 tightening/loosening words, including 紧缩, 宽松, 松弛 and 麻痹. Local gives 8: 措施, 举措, 策略, 对策, 决策, 战略, 部署, 政策. SNN gives a 33-word policy/law implementation group. | Local is the clearest compact general-policy unit in this representative. Cosine follows particular collocations and is harder to use under a general “policy” label. SNN offers a readable but broader topic. This is an important counterexample to treating the cosine/local averages as identical teaching results. |
| 合同 | Archived 16 words; cosine/local 19/17; SNN 21. All retain 协议, 签订, 签署 and associated agreement words. | Useful across configurations. Verbs and nouns belong in a thematic lesson; their mixture is not itself an error. Little evidence that the new topology is needed for this group. |
| 银行 | Archived 30-word banking/document group. Cosine/local give the same 9 members around deposits, cash and balances, but include 宝. SNN gives a 32-word banking/credit group. | Cosine/local narrow the task, but 宝 needs review before teaching. SNN is broader yet readable. Smaller does not automatically mean cleaner. |
| 软件 | Archived 7-word computing/AI group; fresh baseline 12-word software/storage/security group. Cosine/local give 21 computing/peripheral words; SNN gives 35 including 彩电, 闹钟 and 磁带. | No consistent improvement. Cosine/local are useful under a broad computing label, but not software-specific. SNN's consumer-electronics spread is a weaker study unit. Baseline repeats already differ substantially. |
| 感冒 | Archived 46 words, fresh baseline 68. Cosine/local give symptom-related groups of 25/23. SNN gives 59 disease/treatment-context words. | Cosine/local offer a more manageable language lesson, while still mixing the condition with symptoms and related actions. SNN remains broad. This is a vocabulary observation, not a medical classification. |
| 火车 | Archived 42 words, fresh baseline 40. Cosine/local give 25/20 transport words; SNN 46. | Cosine/local make the transport theme more manageable. Trains, buses and taxis remain mixed; a lesson label should reflect that breadth. |
| 妈妈 | Archived 140-word group also contains 老师. Cosine/local separate a 43/36-word relatives/elders group; SNN retains the 122-word people group. | A useful separation, but the core groups still need subdivision and review of ambiguous terms such as 老大. In the baseline and SNN views this duplicates the 老师 group. |
| 为什么 | Archived/fresh baseline form 27-word question/context groups. Cosine/local give 11/12 words around reasons, inference and rhetorical questions: 因为, 所以, 难怪, 难道. SNN retains a broad 29-word question group. | Both kinds can be useful with accurate functional labels. Cosine/local are not simply a “question words” list; this is a changed teaching context. |
| 公斤 | Archived 19 words; cosine/local 26; SNN 33. Mass, area, volume and currencies mix; SNN additionally includes 左右, 余, 秒 and 马力. | No clear improvement for a focused unit. Broad quantity-expression context requires manual organization by meaning. |
| 花 | Archived/fresh baseline have flower groups of 10/9. Cosine/local use money/spending groups of 11/14; SNN uses a 43-word plants/trees group. | A clear limitation of one vector per spelling. The cosine/local result follows the spending sense and cannot serve a flowers lesson without a sense label. The original flower group is preferable for that particular use. |
| 行 | Archived/fresh baseline and SNN leave it singleton. Cosine/local pair it with 哄. | The extra nonsingleton coverage does not create an obvious useful lesson. Keep this as a concrete caution against equating coverage with semantic correctness. |

## Supplementary inspection of the development nominee

After the automated nominee was known, I inspected the **same anchors** in the
unshifted threshold-0.70 graph at q=0.3. This supplementary comparison was not part
of the predeclared 90-view core panel and did not affect selection. Its full
[panels and memberships](control-review/FIXED_GROUPS.md) are retained separately.

The nominee has readable compact groups for contracts (17 words), banking (19),
software/security (11), policy implementation (22) and flowers (13). However,
the main practical problems remain: 牛奶/咖啡 share a 157-word food group,
老师/妈妈 share a 134-word people group, and 焦虑 belongs to a 159-word emotions
group. 苹果 still follows product-launch context; 行 is still a singleton.
The automated gains therefore look like a modest refinement of the baseline,
not a demonstrated transformation of the learning experience.

## Priorities for the next stage

1. **Start the centering/ABTT/mutual-neighbor ablations from cosine k=10, q=0.1.**
   It combines near-complete coverage and much lower concentration with readable
   smaller units in several fixed examples. Retune q after each change and keep
   its perturbation-stability deficit visible.
2. **Keep local k=10, q=0.1 as the close alternative.** Its seven cohort scores
   differ from cosine's by less than the corresponding calibrated tolerances.
   Some representative groups, notably 政策, differ meaningfully in the sample;
   this is a reason to retain it for comparison, not proof of superiority.
3. **Treat SNN k=40, q=0.3 as a lower-priority stability comparator.** It passes
   the hard coverage/concentration limits, but loses all three coherence metrics
   beyond tolerance, and its resolution search hit an endpoint budget. Its broad
   groups provide a weaker practical case for expensive ablations at present.
4. **Retain the unshifted archived-edge nominee as a control and confirmation
   candidate.** Its gains pass the frozen development comparison, but it has not
   used the reserved confirmation seeds and does not resolve the large lesson units.

The numeric shortlist remains unchanged. The archived recommendation remains
the exported default. No claim about learner retention, teaching effectiveness
or an overall semantic accuracy rate follows from this desk review.
