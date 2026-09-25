use anyhow::{ensure, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use hskclustering::{
    graph_v2::{
        calibration::CalibrationPlan,
        embeddings::Representation,
        experiment::{self, Manifest},
        graph::{GraphConfig, Symmetrization, WeightMode},
        leiden::LeidenConfig,
    },
    output::write_json,
};
use std::{fs::File, path::PathBuf};

/// Reproducible graph construction and seeded Leiden, alongside the legacy tools.
#[derive(Parser)]
#[command(name = "graph_v2", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Write a baseline-only calibration plan and a core-screening study template.
    CalibrationPlan {
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Measure baseline variability and freeze an evidence-backed screening manifest.
    Calibrate {
        #[arg(long)]
        plan: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Write the full experiment manifest. Selection is disabled until a policy is supplied.
    Manifest {
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Build exact kNN GraphML, transforms and neighbor diagnostics in a new directory.
    Build(Build),
    /// Run seeded weighted CPM on a saved v2 or legacy graph.
    Cluster(Cluster),
    /// Screen, ablate and confirm configurations from a frozen manifest.
    Experiment {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Validate configuration without running or creating artifacts.
    ValidateManifest {
        #[arg(long)]
        manifest: PathBuf,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum Transform {
    Raw,
    Center,
    Abtt,
}
#[derive(Args)]
struct Build {
    #[arg(short, long, default_value = "data/input-embeddings.txt")]
    input: PathBuf,
    #[arg(short, long)]
    output: PathBuf,
    #[arg(long, default_value_t = 20)]
    k: usize,
    #[arg(long,value_enum,default_value_t=Transform::Raw)]
    representation: Transform,
    /// Number of leading directions to remove; only with --representation abtt.
    #[arg(long)]
    components: Option<usize>,
    #[arg(long,value_enum,default_value_t=Symmetrization::Union)]
    symmetrization: Symmetrization,
    #[arg(long,value_enum,default_value_t=WeightMode::Cosine)]
    weight: WeightMode,
    #[arg(long, default_value_t = 0.0)]
    tau: f64,
    #[arg(long, default_value_t = 0.5)]
    lambda: f64,
    /// Defaults to min(7,k_eff); must not exceed k_eff.
    #[arg(long)]
    k_scale: Option<usize>,
    #[arg(long, default_value_t = 32)]
    block_rows: usize,
}
#[derive(Args)]
struct Cluster {
    #[arg(short, long, default_value = "data/input-embeddings.txt")]
    input: PathBuf,
    /// graph.graphml or graph.json, including archived weighted graphs.
    #[arg(long)]
    graph: PathBuf,
    #[arg(short, long)]
    output: PathBuf,
    #[arg(long, default_value_t = 42)]
    seed: u64,
    /// CPM resolution; otherwise q times the median positive edge weight.
    #[arg(long, conflicts_with = "q")]
    resolution: Option<f64>,
    #[arg(long)]
    q: Option<f64>,
    /// Defaults to 0.3 times the median positive edge weight; gamma always equals resolution.
    #[arg(long)]
    theta: Option<f64>,
    #[arg(long, default_value_t = 100)]
    max_iterations: usize,
    #[arg(long, default_value_t = 1000)]
    max_levels: usize,
    #[arg(long, default_value_t = 10_000_000)]
    max_local_moves: usize,
}
fn main() -> Result<()> {
    match Cli::parse().command {
        Command::CalibrationPlan { output } => {
            ensure!(
                !output.exists(),
                "Plan already exists: {}",
                output.display()
            );
            write_json(&output, &CalibrationPlan::default())?;
        }
        Command::Calibrate { plan, output } => {
            let plan: CalibrationPlan = serde_json::from_reader(File::open(plan)?)?;
            experiment::calibrate_command(&plan, &output)?;
        }
        Command::Manifest { output } => {
            let manifest = Manifest::default();
            if let Some(path) = output {
                ensure!(
                    !path.exists(),
                    "Manifest path already exists: {}",
                    path.display()
                );
                write_json(&path, &manifest)?;
            } else {
                println!("{}", serde_json::to_string_pretty(&manifest)?);
            }
        }
        Command::Build(args) => {
            ensure!(
                matches!(args.representation, Transform::Abtt) || args.components.is_none(),
                "--components requires --representation abtt"
            );
            let representation = match args.representation {
                Transform::Raw => Representation::Raw,
                Transform::Center => Representation::Center,
                Transform::Abtt => Representation::Abtt(args.components.unwrap_or(1)),
            };
            experiment::build_command(
                &args.input,
                &args.output,
                &GraphConfig {
                    representation,
                    k: args.k,
                    symmetrization: args.symmetrization,
                    tau: args.tau,
                    weight: args.weight,
                    lambda: args.lambda,
                    k_scale: args.k_scale,
                    block_rows: args.block_rows,
                },
            )?;
            eprintln!("Graph and diagnostics written to {}", args.output.display());
        }
        Command::Cluster(args) => {
            let graph = experiment::load_graph(&args.graph)?;
            let median = graph.median_weight();
            let q = args.q.unwrap_or(0.1);
            ensure!(q.is_finite() && q > 0.0, "q must be finite and positive");
            let config = LeidenConfig {
                seed: args.seed,
                resolution: args
                    .resolution
                    .unwrap_or_else(|| median.map(|w| q * w).unwrap_or(0.0)),
                theta: args
                    .theta
                    .unwrap_or_else(|| median.map(|w| 0.3 * w).unwrap_or(0.3)),
                max_iterations: args.max_iterations,
                max_levels: args.max_levels,
                max_local_moves: args.max_local_moves,
            };
            experiment::cluster_command(&args.input, &args.graph, &args.output, &config)?;
            eprintln!(
                "Assignments and metrics written to {}",
                args.output.display()
            );
        }
        Command::Experiment { manifest, output } => {
            let manifest = serde_json::from_reader(File::open(manifest)?)?;
            experiment::experiment_command(&manifest, &output)?;
        }
        Command::ValidateManifest { manifest } => {
            let manifest: Manifest = serde_json::from_reader(File::open(manifest)?)?;
            manifest.validate()?;
            experiment::verify_calibration(&manifest)?;
            println!(
                "Manifest is valid; automatic selection {}",
                if manifest.selection.is_some() {
                    "configured"
                } else {
                    "disabled (no explicit policy)"
                }
            );
        }
    }
    Ok(())
}
