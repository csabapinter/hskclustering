# Local graphrs patch

`graphrs-0.12.0/` is extracted from the published crates.io 0.12.0 archive. Its MIT
license declaration in Cargo.toml and README is retained. Cargo uses it through
`[patch.crates-io]` in the root manifest.

The only upstream source change is in
`src/algorithms/community/leiden/mod.rs`: refinement sampling uses
`exp((delta - max_delta) / theta)` instead of `exp(delta / theta)`.

The HSK 1–9 baseline (10,936 nodes, 303,267 edges, CPM resolution 0.05, theta 0.3,
gamma 0.05) crashed in `rand::Uniform::new` with `range overflow`. Either an
individual exponential or their sum can overflow with the original expression.
Subtracting the maximum gain before division makes every exponential at most 1,
with at least one equal to 1, for finite gains and positive theta. It preserves the
relative probabilities; it does not change the clustering objective or parameters.

Two regression tests cover large gains and invariance of relative probabilities.

```sh
cargo test --locked -p graphrs refinement_sampling
```

When upgrading graphrs, compare this patch with the new upstream implementation.
Remove the override once upstream provides an equivalent fix. No upstream issue,
message or pull request has been submitted as part of this local task.
