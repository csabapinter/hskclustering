use anyhow::{ensure, Result};
use clap::Parser;
use hskclustering::{
    clustering::{adjusted_rand_index, partition_metrics, read_assignments},
    embeddings::prepare_embeddings,
    output::{sha256, write_json},
};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(about = "Compare saved token,community partitions in the original SGNS embedding space")]
struct Args {
    /// Two or more assignment CSVs; IDs are matched by token, never by number
    #[arg(required = true, num_args = 2..)]
    assignments: Vec<PathBuf>,
    #[arg(long, short = 'i', default_value = "data/input-embeddings.txt")]
    input: PathBuf,
    #[arg(long, short = 'o', default_value = "results/comparison.json")]
    output: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();
    ensure!(
        !args.output.exists(),
        "Output {} already exists; choose a new --output",
        args.output.display()
    );
    let (tokens, vectors) = prepare_embeddings(&args.input, None)?;
    let mut partitions = Vec::new();
    let mut summaries = Vec::new();
    for path in &args.assignments {
        let labels = read_assignments(path, &tokens)?;
        let metrics = partition_metrics(&vectors, &labels)?;
        eprintln!(
            "{}: {} clusters; cosine silhouette {:?}",
            path.display(),
            metrics.clusters,
            metrics.mean_cosine_silhouette
        );
        summaries.push(
            serde_json::json!({"assignments": path, "sha256": sha256(path)?, "metrics": metrics}),
        );
        partitions.push(labels);
    }
    let mut agreement = Vec::new();
    for i in 0..partitions.len() {
        for j in i + 1..partitions.len() {
            agreement.push(
                serde_json::json!({"left": args.assignments[i], "right": args.assignments[j],
            "adjusted_rand_index": adjusted_rand_index(&partitions[i], &partitions[j])?}),
            );
        }
    }
    write_json(
        &args.output,
        &serde_json::json!({
            "input": args.input, "input_sha256": sha256(&args.input)?,
            "evaluation_space": "original L2-normalized SGNS embeddings; no PCA or whitening",
            "partitions": summaries, "agreement": agreement,
            "interpretation": "ARI measures agreement, not semantic quality. Cosine silhouette is null for one cluster or all singletons. AIC/BIC and Leiden objectives are not comparable."
        }),
    )?;
    eprintln!("Saved {}", args.output.display());
    Ok(())
}
