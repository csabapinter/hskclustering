# Local dependency patches

## graphrs 0.12.0

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

## linfa-clustering 0.8.1

`linfa-clustering-0.8.1/` comes from the published crates.io release. It retains
its MIT/Apache-2.0 license declaration, upstream README, manifest and source.
The only modified upstream source is `src/gaussian_mixture/algorithm.rs`:

- Replace direct `ln(sum(exp(log_probability)))` with a maximum-shifted
  log-sum-exp in the E-step and predictions. This prevents finite large/small
  densities from overflowing/underflowing. It is the fix described in
  [upstream issue #442](https://github.com/rust-ml/linfa/issues/442).
- Expose the existing per-observation likelihood through `score_samples`, so
  AIC/BIC use the exact same density evaluation as fitting and prediction.
- Add a regression covering both overflowing and underflowing densities.

```sh
cargo test --manifest-path vendor/linfa-clustering-0.8.1/Cargo.toml \
  --lib gaussian_mixture --target-dir target/vendor-tests
```

Upstream `n_runs` in this release continues fitting the same initialized model.
The application uses `n_runs(1)` with separately seeded fits, keeping the best
converged result and recording errors. No initialization behavior is patched.
Full covariance is the backend's only covariance type. Adding other covariance
types would require a backend change rather than a CLI-only flag.

On an upgrade, remove the normalization patch once the release includes the
fix; retain or replace the likelihood accessor and rerun the numerical tests.
No upstream messages or PRs were submitted by this task.
