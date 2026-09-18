use anyhow::{ensure, Context, Result};
use clap::Parser;
use hskclustering::{
    clustering::write_assignments,
    embeddings::prepare_gmm_embeddings,
    gmm::{fit_gmm, write_probabilities, Criterion, GmmConfig, GmmFit, ModelScores},
    output::{sha256, write_atomic, write_json},
};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Debug, Parser)]
#[command(about = "Sweep full-covariance GMMs on one fixed PCA representation; select by AIC/BIC")]
struct Args {
    #[arg(long, short = 'i', default_value = "data/input-embeddings.txt")]
    input: PathBuf,
    /// A new run directory; existing directories are never overwritten
    #[arg(long, short = 'o', default_value = "results/gmm/1-9/run")]
    output: PathBuf,
    /// Number of retained dimensions (use separate runs for 20, 30, 50, etc.)
    #[arg(long, default_value_t = 30)]
    pca_components: usize,
    /// Whiten only retained PCA components
    #[arg(long)]
    whiten: bool,
    /// Candidate numbers of mixture components, comma-separated
    #[arg(long, value_delimiter = ',', num_args = 1.., default_value = "50,100,200")]
    clusters: Vec<usize>,
    /// Positive covariance diagonal regularization values, comma-separated
    #[arg(long, value_delimiter = ',', num_args = 1.., default_value = "0.000001")]
    regularization: Vec<f64>,
    /// Independent initialization seeds; every fit is recorded
    #[arg(long, value_delimiter = ',', num_args = 1.., default_value = "42,43,44")]
    seeds: Vec<u64>,
    #[arg(long, default_value_t = 300)]
    max_iterations: u64,
    #[arg(long, default_value_t = 0.001)]
    tolerance: f64,
    #[arg(long, value_enum, default_value = "bic")]
    criterion: Criterion,
}

#[derive(Serialize)]
struct Candidate {
    config: GmmConfig,
    scores: Option<ModelScores>,
    error: Option<String>,
    elapsed_seconds: f64,
}

fn write_candidates(path: &Path, candidates: &[Candidate]) -> Result<()> {
    write_atomic(path, |output| {
        let mut writer = csv::Writer::from_writer(output);
        writer.write_record([
            "clusters",
            "regularization",
            "seed",
            "converged",
            "log_likelihood",
            "parameters",
            "aic",
            "bic",
            "elapsed_seconds",
            "error",
        ])?;
        for c in candidates {
            writer.serialize((
                c.config.clusters,
                c.config.regularization,
                c.config.seed,
                c.scores.is_some(),
                c.scores.map(|s| s.log_likelihood),
                c.scores.map(|s| s.parameters),
                c.scores.map(|s| s.aic),
                c.scores.map(|s| s.bic),
                c.elapsed_seconds,
                &c.error,
            ))?;
        }
        writer.flush()?;
        Ok(())
    })
}

fn main() -> Result<()> {
    let args = Args::parse();
    ensure!(
        !args.output.exists(),
        "Output directory {} already exists; choose a new --output",
        args.output.display()
    );
    // Validate scalar settings before loading data or creating output.
    for &clusters in &args.clusters {
        for &regularization in &args.regularization {
            GmmConfig {
                clusters,
                regularization,
                seed: 0,
                max_iterations: args.max_iterations,
                tolerance: args.tolerance,
            }
            .validate(usize::MAX)?;
        }
    }
    eprintln!(
        "Preparing L2 -> center -> PCA({}){}",
        args.pca_components,
        if args.whiten {
            " -> whiten retained components"
        } else {
            ""
        }
    );
    let prepared = prepare_gmm_embeddings(&args.input, args.pca_components, args.whiten)?;
    ensure!(
        args.clusters.iter().all(|&k| k <= prepared.tokens.len()),
        "GMM clusters cannot exceed the {} input tokens",
        prepared.tokens.len()
    );
    let input_hash = sha256(&args.input)?;
    fs::create_dir_all(
        args.output
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
    )?;
    fs::create_dir(&args.output)
        .with_context(|| format!("Failed to create {}", args.output.display()))?;
    write_json(&args.output.join("pca.json"), &prepared.pca)?;
    eprintln!(
        "{} tokens, {} dimensions; retained {:.2}% of centered variance",
        prepared.tokens.len(),
        prepared.records.ncols(),
        100.0 * prepared.pca.retained_variance_ratio
    );
    let mut candidates = Vec::new();
    let mut best: Option<(GmmConfig, GmmFit)> = None;
    for &clusters in &args.clusters {
        for &regularization in &args.regularization {
            for &seed in &args.seeds {
                let config = GmmConfig {
                    clusters,
                    regularization,
                    seed,
                    max_iterations: args.max_iterations,
                    tolerance: args.tolerance,
                };
                eprintln!("Fitting k={clusters}, regularization={regularization}, seed={seed}");
                let started = Instant::now();
                let result = fit_gmm(&prepared.records, config);
                let mut candidate = Candidate {
                    config,
                    scores: None,
                    error: None,
                    elapsed_seconds: started.elapsed().as_secs_f64(),
                };
                match result {
                    Ok(fit) => {
                        eprintln!(
                            "  AIC {:.3}, BIC {:.3} ({:.1}s)",
                            fit.scores.aic, fit.scores.bic, candidate.elapsed_seconds
                        );
                        candidate.scores = Some(fit.scores);
                        // Save each partition so every successful configuration can be compared.
                        let run = args.output.join(format!(
                            "k{clusters}-reg{regularization}-seed{seed}.communities.csv"
                        ));
                        write_assignments(&run, &prepared.tokens, &fit.assignments)?;
                        if best.as_ref().is_none_or(|(_, current)| {
                            fit.scores.criterion(args.criterion)
                                < current.scores.criterion(args.criterion)
                        }) {
                            best = Some((config, fit));
                        }
                    }
                    Err(error) => {
                        eprintln!("  FAILED: {error:#}");
                        candidate.error = Some(format!("{error:#}"));
                    }
                }
                candidates.push(candidate);
                write_candidates(&args.output.join("candidates.csv"), &candidates)?;
            }
        }
    }
    let best_for = |criterion: Criterion| {
        candidates
            .iter()
            .filter_map(|c| c.scores.map(|s| (c, s.criterion(criterion))))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(c, _)| c)
    };
    let metadata = serde_json::json!({
        "method": "gmm", "covariance": "full", "backend": "linfa-clustering 0.8.1 + local patch",
        "input": args.input, "input_sha256": input_hash, "tokens": prepared.tokens.len(),
        "input_dimensions": prepared.pca.mean.len(), "pca_components": args.pca_components, "whiten": args.whiten,
        "preprocessing": "L2-normalize -> global mean subtraction -> PCA -> optional retained-component whitening; no final normalization",
        "criterion": args.criterion, "selected": best.as_ref().map(|(config, fit)| serde_json::json!({"config": config, "scores": fit.scores})),
        "best_aic": best_for(Criterion::Aic), "best_bic": best_for(Criterion::Bic),
        "candidates": candidates, "command": std::env::args().collect::<Vec<_>>(),
        "score_scope": "Only compare AIC/BIC on identical tokens and preprocessing; not across PCA dimensions, whitening choices, or with Leiden objectives."
    });
    write_json(&args.output.join("run.json"), &metadata)?;
    let (config, fit) = best.context("No GMM converged successfully; see candidates.csv, then adjust iterations, regularization, or cluster counts")?;
    write_assignments(
        &args.output.join("communities.csv"),
        &prepared.tokens,
        &fit.assignments,
    )?;
    write_probabilities(
        &args.output.join("memberships.csv"),
        &prepared.tokens,
        &fit.probabilities,
    )?;
    write_json(&args.output.join("model.json"), &fit.model)?;
    eprintln!(
        "Selected k={}, regularization={}, seed={} by {:?}; saved {}",
        config.clusters,
        config.regularization,
        config.seed,
        args.criterion,
        args.output.display()
    );
    Ok(())
}
