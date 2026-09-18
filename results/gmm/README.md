# GMM results

Save each experiment under `1-9/<run>/` (or another vocabulary range).
Use `cargo run --release --locked --bin gmm_cluster -- --help` and the
[repository instructions](../../README.md#run-gmm).

All candidates within a run share the same input, retained PCA dimension and
whitening setting. AIC/BIC selection is only within that representation.
`communities.csv` is the selected hard partition, directly usable by
`compare_clusters` alongside saved Leiden assignments. `memberships.csv` retains
the selected model's full soft assignments.

Results from a small verification run are not a tuned recommendation. Record
the search scope and inspect actual groups before drawing semantic conclusions.

The [2026-09-18 staged sweep](1-9/experiments-2026-09-18/REPORT.md) explores GMM
independently, with semantic relevance as the objective and no target granularity.
Its [review index](1-9/experiments-2026-09-18/review-index.csv) links component
catalogues, dictionary-enriched word details and soft memberships.
