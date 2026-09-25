//! Manifest-driven experiments, with graph reuse and immutable output directories.
use super::{
    embeddings::{Embeddings, Representation},
    graph::{self, Graph, GraphConfig, Symmetrization, WeightMode},
    leiden::{self, LeidenConfig},
    metrics::{self, GraphMetrics, PartitionMetrics, RepeatMetrics, TokenMetadata},
    selection::{self, CandidateScore, SelectionMetric, SelectionPolicy},
    VERSION,
};
use crate::{
    clustering::{read_assignments, write_assignments},
    output::{sha256, write_atomic, write_json},
};
use anyhow::{ensure, Context, Result};
use rand::{seq::SliceRandom, SeedableRng};
use rand_xoshiro::Xoshiro256PlusPlus;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::Instant,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchiveGraph {
    pub path: PathBuf,
    pub threshold: f64,
    pub unshift: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "config", rename_all = "snake_case")]
pub enum GraphSource {
    Neighbors(GraphConfig),
    Archived(ArchiveGraph),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Baseline {
    pub graph: PathBuf,
    pub recommended_assignments: PathBuf,
    pub threshold: f64,
    pub resolution: f64,
    pub theta: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Manifest {
    pub schema_version: u32,
    pub input: PathBuf,
    pub metadata: Option<PathBuf>,
    pub baseline: Option<Baseline>,
    pub graphs: Vec<GraphConfig>,
    pub q: Vec<f64>,
    pub screening_seeds: Vec<u64>,
    pub confirmation_seeds: Vec<u64>,
    pub development_perturbation_seeds: Vec<u64>,
    pub confirmation_perturbation_seeds: Vec<u64>,
    pub null_seeds: Vec<u64>,
    pub archived_thresholds: Vec<f64>,
    pub archived_resolutions: Vec<f64>,
    pub ablations: bool,
    pub combinations: bool,
    pub confirmation: bool,
    pub max_endpoint_expansions: usize,
    pub endpoint_expansion_factor: f64,
    pub similar_edge_count_relative_tolerance: f64,
    pub similar_community_count_relative_tolerance: f64,
    pub selection: Option<SelectionPolicy>,
    pub solver: LeidenConfig,
}
impl Default for Manifest {
    fn default() -> Self {
        Self {
            schema_version: 1,
            input: "data/input-embeddings.txt".into(),
            metadata: Some("data/hsk-data.csv".into()),
            baseline: Some(Baseline {
                graph: "results/leiden/1-9/hsk1-9-thresh0.70.graphml".into(),
                recommended_assignments:
                    "results/leiden/1-9/experiments-2026-09-17/recommended.communities.csv".into(),
                threshold: 0.70,
                resolution: 0.20,
                theta: 0.30,
            }),
            graphs: [10, 20, 40]
                .into_iter()
                .flat_map(|k| {
                    [WeightMode::Cosine, WeightMode::Snn, WeightMode::Local]
                        .into_iter()
                        .map(move |weight| GraphConfig {
                            k,
                            weight,
                            ..Default::default()
                        })
                })
                .collect(),
            q: vec![0.01, 0.03, 0.1, 0.3],
            screening_seeds: vec![42, 43, 44],
            confirmation_seeds: (1000..1010).collect(),
            development_perturbation_seeds: (10000..10010).collect(),
            confirmation_perturbation_seeds: (20000..20010).collect(),
            null_seeds: (30000..30100).collect(),
            archived_thresholds: vec![0.70, 0.725, 0.75],
            archived_resolutions: vec![0.05, 0.10, 0.20],
            ablations: true,
            combinations: true,
            confirmation: true,
            max_endpoint_expansions: 2,
            endpoint_expansion_factor: 3.0,
            similar_edge_count_relative_tolerance: 0.1,
            similar_community_count_relative_tolerance: 0.1,
            selection: None,
            solver: LeidenConfig::default(),
        }
    }
}
impl Manifest {
    pub fn validate(&self) -> Result<()> {
        ensure!(self.schema_version == 1, "Unsupported manifest schema");
        ensure!(
            !self.graphs.is_empty(),
            "At least one graph configuration is required"
        );
        for g in &self.graphs {
            g.validate()?;
        }
        ensure!(
            !self.q.is_empty() && self.q.iter().all(|q| q.is_finite() && *q > 0.0),
            "q grid must be nonempty and positive"
        );
        ensure!(
            self.screening_seeds.len() >= 3,
            "Screening requires at least three common seeds"
        );
        ensure!(
            !self.confirmation || self.confirmation_seeds.len() >= 10,
            "Confirmation requires at least ten fresh solver seeds"
        );
        ensure!(
            self.development_perturbation_seeds.len() >= 10
                && (!self.confirmation || self.confirmation_perturbation_seeds.len() >= 10),
            "Stress tests require at least ten replicates"
        );
        ensure!(
            self.null_seeds.len() >= 100,
            "Null control requires at least 100 label permutations"
        );
        let mut seeds = BTreeSet::new();
        for list in [
            &self.screening_seeds,
            &self.confirmation_seeds,
            &self.development_perturbation_seeds,
            &self.confirmation_perturbation_seeds,
            &self.null_seeds,
        ] {
            for &seed in list {
                ensure!(
                    seeds.insert(seed),
                    "Seed {seed} is duplicated or reused between development and confirmation"
                );
            }
        }
        ensure!(
            self.endpoint_expansion_factor.is_finite() && self.endpoint_expansion_factor > 1.0,
            "Expansion factor must exceed 1"
        );
        for tolerance in [
            self.similar_edge_count_relative_tolerance,
            self.similar_community_count_relative_tolerance,
        ] {
            ensure!(
                tolerance.is_finite() && (0.0..=1.0).contains(&tolerance),
                "Comparison tolerances must be in [0,1]"
            );
        }
        ensure!(
            self.archived_thresholds
                .iter()
                .all(|&v| v.is_finite() && (0.5..=1.0).contains(&v)),
            "Invalid control threshold"
        );
        ensure!(
            self.archived_resolutions
                .iter()
                .all(|&v| v.is_finite() && v >= 0.0),
            "Invalid control resolution"
        );
        if let Some(b) = &self.baseline {
            ensure!(
                b.threshold.is_finite() && b.threshold > 0.5 && b.threshold <= 1.0,
                "Baseline threshold must be in (0.5,1]"
            );
            LeidenConfig {
                resolution: b.resolution,
                theta: b.theta,
                ..self.solver.clone()
            }
            .validate()?;
        }
        if let Some(policy) = &self.selection {
            policy.validate()?;
        }
        self.solver.validate()?;
        Ok(())
    }
}

pub fn fresh_directory(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(path)
        .with_context(|| format!("Output must be a new directory: {}", path.display()))
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn code_hashes() -> BTreeMap<String, String> {
    [
        ("Cargo.toml", include_bytes!("../../Cargo.toml").as_slice()),
        ("Cargo.lock", include_bytes!("../../Cargo.lock").as_slice()),
        (
            "bin/graph_v2.rs",
            include_bytes!("../bin/graph_v2.rs").as_slice(),
        ),
        ("graph_v2/mod.rs", include_bytes!("mod.rs").as_slice()),
        (
            "graph_v2/embeddings.rs",
            include_bytes!("embeddings.rs").as_slice(),
        ),
        ("graph_v2/graph.rs", include_bytes!("graph.rs").as_slice()),
        ("graph_v2/leiden.rs", include_bytes!("leiden.rs").as_slice()),
        (
            "graph_v2/metrics.rs",
            include_bytes!("metrics.rs").as_slice(),
        ),
        (
            "graph_v2/selection.rs",
            include_bytes!("selection.rs").as_slice(),
        ),
        (
            "graph_v2/experiment.rs",
            include_bytes!("experiment.rs").as_slice(),
        ),
        (
            "clustering.rs",
            include_bytes!("../clustering.rs").as_slice(),
        ),
        ("graph_io.rs", include_bytes!("../graph_io.rs").as_slice()),
        ("output.rs", include_bytes!("../output.rs").as_slice()),
    ]
    .into_iter()
    .map(|(name, bytes)| (name.into(), digest(bytes)))
    .collect()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Cost {
    pub elapsed_seconds: f64,
    pub process_peak_memory_bytes: Option<u64>,
    pub peak_memory_scope: String,
    pub artifact_bytes: u64,
}
pub fn peak_memory_bytes() -> Option<u64> {
    #[cfg(unix)]
    {
        let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
        // getrusage writes the complete struct on success; RUSAGE_SELF has no pointers to caller data.
        let status = unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) };
        if status != 0 {
            return None;
        }
        let peak = unsafe { usage.assume_init() }.ru_maxrss;
        #[cfg(target_os = "macos")]
        let multiplier = 1;
        #[cfg(not(target_os = "macos"))]
        let multiplier = 1024;
        u64::try_from(peak).ok().map(|p| p * multiplier)
    }
    #[cfg(not(unix))]
    {
        None
    }
}
pub fn artifact_bytes(path: &Path) -> Result<u64> {
    if path.is_file() {
        return Ok(path.metadata()?.len());
    }
    let mut total = 0;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if !entry.file_type()?.is_symlink() {
            total += artifact_bytes(&entry.path())?;
        }
    }
    Ok(total)
}
fn cost(start: Instant, path: &Path) -> Result<Cost> {
    Ok(Cost {elapsed_seconds:start.elapsed().as_secs_f64(),process_peak_memory_bytes:peak_memory_bytes(),
    peak_memory_scope:"Process lifetime high-water RSS, including earlier stages; not a per-candidate allocation estimate".into(),artifact_bytes:artifact_bytes(path)?})
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphRecord {
    pub key: String,
    pub source: GraphSource,
    pub input_sha256: String,
    pub source_graph_sha256: Option<String>,
    pub median_weight: Option<f64>,
    pub metrics: GraphMetrics,
    pub cost: Cost,
    pub directory: PathBuf,
}

pub fn construct(
    input: &Embeddings,
    input_hash: &str,
    source: &GraphSource,
    directory: &Path,
) -> Result<(Graph, GraphRecord)> {
    let start = Instant::now();
    let source_hash = match source {
        GraphSource::Archived(a) => Some(sha256(&a.path)?),
        _ => None,
    };
    let key = digest(&serde_json::to_vec(&(
        VERSION,
        code_hashes(),
        input_hash,
        source,
        &source_hash,
    ))?);
    fs::create_dir_all(directory)?;
    let (graph, metrics) = match source {
        GraphSource::Neighbors(config) => {
            let built = graph::build(input, config)?;
            built.save(directory)?;
            let metrics = metrics::graph_metrics(&built.graph, Some(&built))?;
            (built.graph, metrics)
        }
        GraphSource::Archived(config) => {
            let graph = Graph::from_graphml(&config.path, config.threshold, config.unshift)?;
            ensure!(
                graph.tokens == input.tokens,
                "Archived graph vocabulary differs from the input"
            );
            graph.write_graphml(&directory.join("graph.graphml"))?;
            let metrics = metrics::graph_metrics(&graph, None)?;
            (graph, metrics)
        }
    };
    write_json(&directory.join("graph.json"), &graph)?;
    let record = GraphRecord {
        key,
        source: source.clone(),
        input_sha256: input_hash.into(),
        source_graph_sha256: source_hash,
        median_weight: graph.median_weight(),
        metrics,
        cost: cost(start, directory)?,
        directory: directory.into(),
    };
    write_json(&directory.join("graph-record.json"), &record)?;
    Ok((graph, record))
}

pub fn build_command(input_path: &Path, output: &Path, config: &GraphConfig) -> Result<()> {
    config.validate()?;
    fresh_directory(output)?;
    write_json(
        &output.join("invocation.json"),
        &serde_json::json!({"version":VERSION,"input":input_path,"config":config,"code_hashes":code_hashes(),"executable_sha256":sha256(&std::env::current_exe()?)?,"command":std::env::args().collect::<Vec<_>>()}),
    )?;
    let result = (|| {
        let input = Embeddings::load(input_path)?;
        construct(
            &input,
            &sha256(input_path)?,
            &GraphSource::Neighbors(config.clone()),
            output,
        )?;
        Ok(())
    })();
    if let Err(error) = &result {
        write_json(&output.join("failure.json"), &format!("{error:#}"))?;
    }
    result
}

pub fn cluster_command(
    input_path: &Path,
    graph_path: &Path,
    output: &Path,
    config: &LeidenConfig,
) -> Result<()> {
    config.validate()?;
    fresh_directory(output)?;
    let result = (|| {
        let input = Embeddings::load(input_path)?;
        let graph = load_graph(graph_path)?;
        ensure!(
            input.tokens == graph.tokens,
            "Graph vocabulary differs from the input"
        );
        let start = Instant::now();
        let result = leiden::run(&graph, config)?;
        let solver_seconds = start.elapsed().as_secs_f64();
        write_assignments(
            &output.join("communities.csv"),
            &input.tokens,
            &result.labels,
        )?;
        let (metrics, words) = metrics::partition_metrics(&input, &graph, &result.labels)?;
        write_json(&output.join("metrics.json"), &metrics)?;
        write_words(&output.join("words.csv"), &input.tokens, &words)?;
        write_json(
            &output.join("run.json"),
            &serde_json::json!({"version":VERSION,"code_hashes":code_hashes(),"executable_sha256":sha256(&std::env::current_exe()?)?,"input_sha256":sha256(input_path)?,"graph_sha256":sha256(graph_path)?,
            "solver":result,"solver_seconds":solver_seconds,"cost":cost(start,output)?}),
        )?;
        Ok(())
    })();
    if let Err(error) = &result {
        write_json(&output.join("failure.json"), &format!("{error:#}"))?;
    }
    result
}
pub fn load_graph(path: &Path) -> Result<Graph> {
    if path.extension().is_some_and(|s| s == "json") {
        let graph: Graph = serde_json::from_reader(fs::File::open(path)?)?;
        graph.validate()?;
        Ok(graph)
    } else {
        Graph::from_graphml(path, 0.0, false)
    }
}
fn write_words(path: &Path, tokens: &[String], words: &[metrics::WordMetrics]) -> Result<()> {
    write_atomic(path, |w| {
        let mut writer = csv::Writer::from_writer(w);
        writer.write_record([
            "token",
            "community",
            "size",
            "cohesion",
            "silhouette",
            "margin",
        ])?;
        for (token, word) in tokens.iter().zip(words) {
            writer.serialize((
                token,
                word.community,
                word.size,
                word.cohesion,
                word.silhouette,
                word.margin,
            ))?;
        }
        writer.flush()?;
        Ok(())
    })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunRecord {
    pub seed: u64,
    pub assignments: Option<PathBuf>,
    pub metrics: Option<PartitionMetrics>,
    pub solver_seconds: f64,
    pub error: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Perturbation {
    pub kind: String,
    pub seed: u64,
    pub retained_tokens: usize,
    pub vocabulary_coverage: f64,
    pub comparator_nonsingleton_coverage: Option<f64>,
    pub perturbed_nonsingleton_coverage: Option<f64>,
    pub edge_overlap: metrics::Metric,
    pub agreement: Option<metrics::Agreement>,
    pub error: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Sensitivity {
    pub parameter: String,
    pub direction: String,
    pub requested_factor: f64,
    pub original: f64,
    pub changed: f64,
    pub edge_overlap: metrics::Metric,
    pub agreement: Option<metrics::Agreement>,
    pub error: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Candidate {
    pub id: String,
    pub stage: String,
    pub graph_key: String,
    pub graph_directory: PathBuf,
    pub source: GraphSource,
    pub q: Option<f64>,
    pub resolution: f64,
    pub theta: f64,
    pub baseline: bool,
    pub control: bool,
    pub runs: Vec<RunRecord>,
    pub repeats: RepeatMetrics,
    pub perturbations: Vec<Perturbation>,
    pub sensitivity: Vec<Sensitivity>,
    pub score: CandidateScore,
    pub representative_assignments: Option<PathBuf>,
    pub directory: PathBuf,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Attempt {
    pub stage: String,
    pub setting: serde_json::Value,
    pub status: String,
    pub error: Option<String>,
}

struct Runner {
    manifest: Manifest,
    input: Embeddings,
    input_hash: String,
    output: PathBuf,
    metadata: BTreeMap<String, TokenMetadata>,
    baseline_degrees: Option<Vec<usize>>,
    baseline_labels: Option<Vec<usize>>,
    graphs: BTreeMap<String, GraphRecord>,
    candidates: Vec<Candidate>,
    attempts: Vec<Attempt>,
    expansion_limited: BTreeSet<String>,
}

impl Runner {
    fn attempt(
        &mut self,
        stage: &str,
        setting: impl Serialize,
        error: Option<String>,
    ) -> Result<()> {
        self.attempts.push(Attempt {
            stage: stage.into(),
            setting: serde_json::to_value(setting)?,
            status: if error.is_some() {
                "failed"
            } else {
                "complete"
            }
            .into(),
            error,
        });
        write_json(&self.output.join("attempts.json"), &self.attempts)
    }
    fn graph(&mut self, source: &GraphSource) -> Result<(Graph, GraphRecord)> {
        let source_hash = match source {
            GraphSource::Archived(a) => Some(sha256(&a.path)?),
            _ => None,
        };
        let key = digest(&serde_json::to_vec(&(
            VERSION,
            code_hashes(),
            &self.input_hash,
            source,
            source_hash,
        ))?);
        if let Some(record) = self.graphs.get(&key) {
            return Ok((
                load_graph(&record.directory.join("graph.json"))?,
                record.clone(),
            ));
        }
        let directory = self.output.join("graphs").join(&key);
        let (graph, record) = construct(&self.input, &self.input_hash, source, &directory)?;
        self.graphs.insert(key, record.clone());
        Ok((graph, record))
    }
    fn solver_config(&self, resolution: f64, theta: f64, seed: u64) -> LeidenConfig {
        LeidenConfig {
            resolution,
            theta,
            seed,
            ..self.manifest.solver.clone()
        }
    }
    fn run_seed(
        &mut self,
        graph: &Graph,
        resolution: f64,
        theta: f64,
        seed: u64,
        directory: &Path,
        stage: &str,
    ) -> Result<RunRecord> {
        fs::create_dir_all(directory)?;
        let cfg = self.solver_config(resolution, theta, seed);
        let start = Instant::now();
        let mut solver_seconds = 0.0;
        let result = (|| -> Result<(PathBuf, PartitionMetrics)> {
            let run = leiden::run(graph, &cfg)?;
            solver_seconds = start.elapsed().as_secs_f64();
            let assignments = directory.join("communities.csv");
            write_assignments(&assignments, &graph.tokens, &run.labels)?;
            let input = if graph.tokens == self.input.tokens {
                self.input.clone()
            } else {
                let index: BTreeMap<_, _> = self
                    .input
                    .tokens
                    .iter()
                    .enumerate()
                    .map(|(i, t)| (t, i))
                    .collect();
                self.input
                    .subset(&graph.tokens.iter().map(|t| index[t]).collect::<Vec<_>>())
            };
            let (metrics, words) = metrics::partition_metrics(&input, graph, &run.labels)?;
            write_json(&directory.join("solver.json"), &run)?;
            write_words(&directory.join("words.csv"), &graph.tokens, &words)?;
            write_json(&directory.join("metrics.json"), &metrics)?;
            Ok((assignments, metrics))
        })();
        let record = match result {
            Ok((assignments, metrics)) => RunRecord {
                seed,
                assignments: Some(assignments),
                metrics: Some(metrics),
                solver_seconds,
                error: None,
            },
            Err(e) => RunRecord {
                seed,
                assignments: None,
                metrics: None,
                solver_seconds: start.elapsed().as_secs_f64(),
                error: Some(format!("{e:#}")),
            },
        };
        write_json(
            &directory.join("run.json"),
            &serde_json::json!({"config":cfg,"run":record,"cost":cost(start,directory)?}),
        )?;
        self.attempt(
            stage,
            serde_json::json!({"directory":directory,"config":cfg}),
            record.error.clone(),
        )?;
        Ok(record)
    }

    fn perturbed_graph(
        &mut self,
        graph: &Graph,
        record: &GraphRecord,
        kind: &str,
        seed: u64,
    ) -> Result<(Graph, Vec<usize>)> {
        let mut rng = Xoshiro256PlusPlus::seed_from_u64(seed);
        let mut indices: Vec<_> = (0..graph.tokens.len()).collect();
        if kind == "vocabulary_subsample" {
            indices.shuffle(&mut rng);
            indices.truncate(((indices.len() as f64 * 0.9).round() as usize).max(1));
            indices.sort_unstable();
        }
        let key = digest(&serde_json::to_vec(&(&record.key, kind, seed, &indices))?);
        let directory = self.output.join("perturbation-graphs").join(key);
        let path = directory.join("graph.json");
        if path.exists() {
            return Ok((load_graph(&path)?, indices));
        }
        let start = Instant::now();
        fs::create_dir_all(&directory)?;
        let perturbed = if kind == "edge_removal" {
            let mut order: Vec<_> = (0..graph.edges.len()).collect();
            order.shuffle(&mut rng);
            let remove: BTreeSet<_> = order
                .into_iter()
                .take((graph.edges.len() as f64 * 0.05).round() as usize)
                .collect();
            Graph {
                tokens: graph.tokens.clone(),
                edges: graph
                    .edges
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| !remove.contains(i))
                    .map(|(_, e)| e.clone())
                    .collect(),
            }
        } else {
            match &record.source {
                GraphSource::Neighbors(config) => {
                    let subset = self.input.subset(&indices);
                    let built = graph::build(&subset, config)?;
                    built.save(&directory)?;
                    built.graph
                }
                // Raw archived thresholds have no vocabulary-fitted transform: rebuilding
                // those exact stored-weight edges on the subset equals an induced graph.
                GraphSource::Archived(_) => graph.induced(&indices),
            }
        };
        perturbed.validate()?;
        perturbed.write_graphml(&directory.join("graph.graphml"))?;
        write_json(&path, &perturbed)?;
        write_json(
            &directory.join("provenance.json"),
            &serde_json::json!({"source_graph_key":record.key,"kind":kind,"seed":seed,"indices":indices,
            "retained_tokens":perturbed.tokens.len(),"edges":perturbed.edges.len(),"cost":cost(start,&directory)?,"sampling":"nearest integer; vocabulary retains at least one token"}),
        )?;
        Ok((perturbed, indices))
    }

    #[allow(clippy::too_many_arguments)]
    fn stress(
        &mut self,
        graph: &Graph,
        record: &GraphRecord,
        resolution: f64,
        theta: f64,
        seeds: &[u64],
        directory: &Path,
        stage: &str,
    ) -> Result<Vec<Perturbation>> {
        let mut results = vec![];
        for &seed in seeds {
            let comparator = self.run_seed(
                graph,
                resolution,
                theta,
                seed,
                &directory.join(format!("comparator-{seed}")),
                stage,
            )?;
            for kind in ["edge_removal", "vocabulary_subsample"] {
                let mut p = Perturbation {
                    kind: kind.into(),
                    seed,
                    retained_tokens: 0,
                    vocabulary_coverage: 0.0,
                    comparator_nonsingleton_coverage: None,
                    perturbed_nonsingleton_coverage: None,
                    edge_overlap: metrics::Metric::missing("Perturbation failed"),
                    agreement: None,
                    error: None,
                };
                let result = (|| -> Result<()> {
                    let (perturbed, indices) = self.perturbed_graph(graph, record, kind, seed)?;
                    p.retained_tokens = indices.len();
                    p.vocabulary_coverage = indices.len() as f64 / graph.tokens.len() as f64;
                    p.edge_overlap = metrics::edge_overlap(&graph.induced(&indices), &perturbed);
                    let baseline_path = comparator.assignments.as_ref().with_context(|| {
                        format!("Unperturbed comparator failed: {:?}", comparator.error)
                    })?;
                    let baseline = read_assignments(baseline_path, &graph.tokens)?;
                    let retained: Vec<_> = indices.iter().map(|&i| baseline[i]).collect();
                    let original_sizes = leiden::groups(&baseline);
                    let canonical = leiden::canonicalize(&baseline);
                    p.comparator_nonsingleton_coverage = Some(
                        indices
                            .iter()
                            .filter(|&&i| original_sizes[canonical[i]].len() > 1)
                            .count() as f64
                            / indices.len() as f64,
                    );
                    let run = self.run_seed(
                        &perturbed,
                        resolution,
                        theta,
                        seed,
                        &directory.join(format!("{kind}-{seed}")),
                        stage,
                    )?;
                    let path = run
                        .assignments
                        .as_ref()
                        .with_context(|| format!("Perturbed solver failed: {:?}", run.error))?;
                    let labels = read_assignments(path, &perturbed.tokens)?;
                    p.perturbed_nonsingleton_coverage =
                        run.metrics.as_ref().map(|m| m.nonsingleton_coverage);
                    p.agreement = Some(metrics::agreement(&retained, &labels)?);
                    Ok(())
                })();
                if let Err(e) = result {
                    p.error = Some(format!("{e:#}"));
                }
                self.attempt(stage,serde_json::json!({"graph":record.key,"resolution":resolution,"kind":kind,"seed":seed}),p.error.clone())?;
                results.push(p);
            }
        }
        write_json(&directory.join("perturbations.json"), &results)?;
        Ok(results)
    }

    #[allow(clippy::too_many_arguments)]
    fn evaluate(
        &mut self,
        source: &GraphSource,
        q: Option<f64>,
        fixed: Option<(f64, f64)>,
        stage: &str,
        baseline: bool,
        control: bool,
        seeds: &[u64],
        perturbation_seeds: &[u64],
    ) -> Result<Candidate> {
        let (graph, record) = self.graph(source)?;
        let (resolution, theta) = fixed.unwrap_or_else(|| match record.median_weight {
            Some(median) => (q.unwrap_or(0.1) * median, 0.3 * median),
            None => (0.0, 0.3),
        });
        let id = digest(&serde_json::to_vec(&(&record.key, resolution, theta))?);
        let directory = self
            .output
            .join(if stage == "confirmation" {
                "confirmation"
            } else {
                "candidates"
            })
            .join(&id);
        fs::create_dir_all(&directory)?;
        eprintln!(
            "{stage}: {} r={resolution:.6} theta={theta:.6}; {} nodes, {} edges",
            &id[..12],
            graph.tokens.len(),
            graph.edges.len()
        );
        write_json(
            &directory.join("setting.json"),
            &serde_json::json!({"id":id,"stage":stage,"source":source,"q":q,"resolution":resolution,"theta":theta,
            "gamma":resolution,"graph_key":record.key,"seeds":seeds,"perturbation_seeds":perturbation_seeds}),
        )?;
        let mut runs = vec![];
        let mut partitions = vec![];
        for &seed in seeds {
            let run = self.run_seed(
                &graph,
                resolution,
                theta,
                seed,
                &directory.join(format!("seed-{seed}")),
                stage,
            )?;
            if let Some(path) = &run.assignments {
                partitions.push((seed, read_assignments(path, &graph.tokens)?));
            }
            runs.push(run);
        }
        let repeats = metrics::repeats(&partitions)?;
        write_json(&directory.join("repeats.json"), &repeats)?;
        let representative_assignments = if let Some(index) = repeats.representative_index {
            let path = directory.join("communities.csv");
            write_assignments(&path, &graph.tokens, &partitions[index].1)?;
            let (m, words) = metrics::partition_metrics(&self.input, &graph, &partitions[index].1)?;
            write_json(&directory.join("representative-metrics.json"), &m)?;
            write_json(
                &directory.join("strata.json"),
                &metrics::stratify(
                    &self.input.tokens,
                    &words,
                    &self.metadata,
                    self.baseline_degrees.as_deref(),
                ),
            )?;
            write_json(
                &directory.join("null-control.json"),
                &metrics::null_control(
                    &self.input,
                    &partitions[index].1,
                    &self.manifest.null_seeds,
                ),
            )?;
            Some(path)
        } else {
            None
        };
        let perturbations = if representative_assignments.is_some() {
            self.stress(
                &graph,
                &record,
                resolution,
                theta,
                perturbation_seeds,
                &directory.join("stress"),
                stage,
            )?
        } else {
            vec![]
        };
        let mut measurements = BTreeMap::new();
        for metric in [
            SelectionMetric::Cohesion,
            SelectionMetric::Silhouette,
            SelectionMetric::Margin,
            SelectionMetric::NonsingletonCoverage,
        ] {
            let values: Vec<_> = runs
                .iter()
                .filter_map(|r| r.metrics.as_ref())
                .filter_map(|m| match metric {
                    SelectionMetric::Cohesion => m.cohesion.value,
                    SelectionMetric::Silhouette => m.silhouette.value,
                    SelectionMetric::Margin => m.margin.value,
                    _ => Some(m.nonsingleton_coverage),
                })
                .collect();
            let complete = values.len() == runs.len();
            measurements.insert(
                metric,
                if complete {
                    metrics::distribution(values).median
                } else {
                    None
                },
            );
        }
        measurements.insert(SelectionMetric::RepeatAri, repeats.ari.median);
        measurements.insert(
            SelectionMetric::EdgeRemovalAri,
            metrics::distribution(
                perturbations
                    .iter()
                    .filter(|p| p.kind == "edge_removal")
                    .filter_map(|p| p.agreement.as_ref().map(|a| a.ari)),
            )
            .median,
        );
        measurements.insert(
            SelectionMetric::SubsampleAri,
            metrics::distribution(
                perturbations
                    .iter()
                    .filter(|p| p.kind == "vocabulary_subsample")
                    .filter_map(|p| p.agreement.as_ref().map(|a| a.ari)),
            )
            .median,
        );
        let valid = runs.iter().all(|r| {
            r.metrics
                .as_ref()
                .is_some_and(|m| m.all_communities_connected)
        }) && perturbations.len() == 2 * perturbation_seeds.len()
            && perturbations.iter().all(|p| p.error.is_none());
        let score = CandidateScore {
            id: id.clone(),
            graph_id: record.key.clone(),
            baseline,
            metrics: measurements,
            minimum_communities: runs
                .iter()
                .filter_map(|r| r.metrics.as_ref().map(|m| m.communities))
                .min()
                .unwrap_or(0),
            maximum_largest_share: runs
                .iter()
                .filter_map(|r| r.metrics.as_ref().map(|m| m.largest_community_share))
                .fold(0.0, f64::max),
            minimum_coverage: runs
                .iter()
                .filter_map(|r| r.metrics.as_ref().map(|m| m.nonsingleton_coverage))
                .fold(1.0, f64::min),
            valid,
            error: (!valid).then(|| {
                "Failed solver, incomplete stress tests, or partition invariant failure".into()
            }),
            transforms: match source {
                GraphSource::Neighbors(g) => match g.representation {
                    Representation::Raw => 0,
                    Representation::Center => 1,
                    Representation::Abtt(_) => 2,
                },
                _ => 0,
            },
            runtime_seconds: record.cost.elapsed_seconds
                + runs.iter().map(|r| r.solver_seconds).sum::<f64>() / runs.len().max(1) as f64,
        };
        let candidate = Candidate {
            id,
            stage: stage.into(),
            graph_key: record.key,
            graph_directory: record.directory,
            source: source.clone(),
            q,
            resolution,
            theta,
            baseline,
            control,
            runs,
            repeats,
            perturbations,
            sensitivity: vec![],
            score,
            representative_assignments,
            directory,
        };
        write_json(&candidate.directory.join("candidate.json"), &candidate)?;
        Ok(candidate)
    }

    fn screen_one(
        &mut self,
        source: &GraphSource,
        q: Option<f64>,
        fixed: Option<(f64, f64)>,
        stage: &str,
        baseline: bool,
        control: bool,
    ) -> Result<Option<usize>> {
        let setting = serde_json::json!({"source":source,"q":q,"fixed_resolution_theta":fixed});
        let result = (|| -> Result<Candidate> {
            let (_, record) = self.graph(source)?;
            let (r, t) = fixed.unwrap_or_else(|| {
                record
                    .median_weight
                    .map(|w| (q.unwrap_or(0.1) * w, 0.3 * w))
                    .unwrap_or((0.0, 0.3))
            });
            if let Some(existing) = self
                .candidates
                .iter()
                .find(|c| c.graph_key == record.key && c.resolution == r && c.theta == t)
            {
                return Ok(existing.clone());
            }
            self.evaluate(
                source,
                q,
                fixed,
                stage,
                baseline,
                control,
                &self.manifest.screening_seeds.clone(),
                &self.manifest.development_perturbation_seeds.clone(),
            )
        })();
        match result {
            Ok(mut candidate) => {
                if let Some(index) = self.candidates.iter().position(|c| c.id == candidate.id) {
                    if baseline {
                        self.candidates[index].baseline = true;
                        self.candidates[index].score.baseline = true;
                    }
                    return Ok(Some(index));
                }
                candidate.baseline = baseline;
                candidate.score.baseline = baseline;
                let error = candidate.score.error.clone();
                self.candidates.push(candidate);
                self.attempt(stage, setting, error)?;
                self.save_tables()?;
                Ok(Some(self.candidates.len() - 1))
            }
            Err(e) => {
                self.attempt(stage, setting, Some(format!("{e:#}")))?;
                Ok(None)
            }
        }
    }

    fn screen_grid(&mut self, source: &GraphSource, stage: &str, control: bool) -> Result<()> {
        let mut grid = self.manifest.q.clone();
        grid.sort_by(f64::total_cmp);
        grid.dedup();
        let mut local = BTreeSet::new();
        for expansion in 0..=self.manifest.max_endpoint_expansions {
            for &q in &grid {
                if let Some(index) =
                    self.screen_one(source, Some(q), None, stage, false, control)?
                {
                    local.insert(index);
                }
            }
            let scores: Vec<_> = local
                .iter()
                .map(|&i| self.candidates[i].score.clone())
                .collect();
            let retained = selection::pareto(&scores);
            let low = *grid.first().unwrap();
            let high = *grid.last().unwrap();
            let at = |q: f64| {
                local.iter().any(|&i| {
                    self.candidates[i].q == Some(q) && retained.contains(&self.candidates[i].id)
                })
            };
            // A connected partition with one community per graph component
            // cannot coarsen further at any smaller nonnegative resolution.
            let at_component_floor = local.iter().any(|&i| {
                let c = &self.candidates[i];
                c.q == Some(low)
                    && c.score.valid
                    && c.runs.iter().all(|r| {
                        r.metrics.as_ref().is_some_and(|m| {
                            m.communities == self.graphs[&c.graph_key].metrics.components
                        })
                    })
            });
            let expand_low = at(low) && !at_component_floor;
            let expand_high = at(high);
            if !expand_low && !expand_high {
                break;
            }
            if expansion == self.manifest.max_endpoint_expansions {
                if let Some(&i) = local.first() {
                    self.expansion_limited
                        .insert(self.candidates[i].graph_key.clone());
                }
                break;
            }
            if expand_low {
                let q = low / self.manifest.endpoint_expansion_factor;
                if q > 0.0 {
                    grid.push(q);
                }
            }
            if expand_high {
                let q = high * self.manifest.endpoint_expansion_factor;
                if q.is_finite() {
                    grid.push(q);
                }
            }
            grid.sort_by(f64::total_cmp);
            grid.dedup();
        }
        Ok(())
    }

    fn save_tables(&self) -> Result<()> {
        write_json(&self.output.join("candidates.json"), &self.candidates)?;
        write_atomic(&self.output.join("candidates.csv"), |w| {
            let mut csv = csv::Writer::from_writer(w);
            csv.write_record([
                "id",
                "stage",
                "graph_key",
                "q",
                "resolution",
                "theta",
                "baseline",
                "valid",
                "cohesion",
                "silhouette",
                "margin",
                "repeat_ari",
                "edge_removal_ari",
                "subsample_ari",
                "nonsingleton_coverage",
                "largest_community_share",
                "seconds",
                "error",
            ])?;
            for c in &self.candidates {
                let m = |metric| {
                    c.score
                        .metrics
                        .get(&metric)
                        .copied()
                        .flatten()
                        .map(|v| v.to_string())
                        .unwrap_or_default()
                };
                csv.write_record([
                    c.id.clone(),
                    c.stage.clone(),
                    c.graph_key.clone(),
                    c.q.map(|v| v.to_string()).unwrap_or_default(),
                    c.resolution.to_string(),
                    c.theta.to_string(),
                    c.baseline.to_string(),
                    c.score.valid.to_string(),
                    m(SelectionMetric::Cohesion),
                    m(SelectionMetric::Silhouette),
                    m(SelectionMetric::Margin),
                    m(SelectionMetric::RepeatAri),
                    m(SelectionMetric::EdgeRemovalAri),
                    m(SelectionMetric::SubsampleAri),
                    m(SelectionMetric::NonsingletonCoverage),
                    c.score.maximum_largest_share.to_string(),
                    c.score.runtime_seconds.to_string(),
                    c.score.error.clone().unwrap_or_default(),
                ])?;
            }
            csv.flush()?;
            Ok(())
        })
    }

    fn retained_configs(&self) -> Vec<GraphConfig> {
        let retained = selection::pareto(
            &self
                .candidates
                .iter()
                .map(|c| c.score.clone())
                .collect::<Vec<_>>(),
        );
        let mut configs = BTreeMap::new();
        for candidate in &self.candidates {
            if retained.contains(&candidate.id) {
                if let GraphSource::Neighbors(g) = &candidate.source {
                    configs.insert(serde_json::to_string(g).unwrap(), g.clone());
                }
            }
        }
        configs.into_values().collect()
    }

    fn sensitivity(&mut self, candidate: &Candidate) -> Result<Vec<Sensitivity>> {
        let graph = load_graph(&candidate.graph_directory.join("graph.json"))?;
        let Some(reference) = candidate.runs.iter().find(|r| r.assignments.is_some()) else {
            return Ok(vec![]);
        };
        let reference_labels =
            read_assignments(reference.assignments.as_ref().unwrap(), &graph.tokens)?;
        let mut results = vec![];
        for (direction, factor) in [("decrease", 0.8), ("increase", 1.2)] {
            for parameter in ["k", "resolution"] {
                if parameter == "k" && matches!(candidate.source, GraphSource::Archived(_)) {
                    continue;
                }
                let original = if parameter == "k" {
                    match &candidate.source {
                        GraphSource::Neighbors(g) => g.k as f64,
                        _ => unreachable!(),
                    }
                } else {
                    candidate.resolution
                };
                let changed = if parameter == "k" {
                    (original * factor).round().max(1.0)
                } else {
                    original * factor
                };
                let mut record = Sensitivity {
                    parameter: parameter.into(),
                    direction: direction.into(),
                    requested_factor: factor,
                    original,
                    changed,
                    edge_overlap: metrics::Metric::missing("Sensitivity run failed"),
                    agreement: None,
                    error: None,
                };
                let result = (|| -> Result<()> {
                    let mut source = candidate.source.clone();
                    if parameter == "k" {
                        if let GraphSource::Neighbors(g) = &mut source {
                            g.k = changed as usize;
                        }
                    }
                    let (modified, _) = self.graph(&source)?;
                    record.edge_overlap = metrics::edge_overlap(&graph, &modified);
                    let directory = candidate
                        .directory
                        .join("sensitivity")
                        .join(format!("{parameter}-{direction}"));
                    let r = if parameter == "resolution" {
                        changed
                    } else {
                        candidate.resolution
                    };
                    let run = self.run_seed(
                        &modified,
                        r,
                        candidate.theta,
                        reference.seed,
                        &directory,
                        "sensitivity",
                    )?;
                    let path = run
                        .assignments
                        .as_ref()
                        .with_context(|| format!("Sensitivity solver failed: {:?}", run.error))?;
                    record.agreement = Some(metrics::agreement(
                        &reference_labels,
                        &read_assignments(path, &modified.tokens)?,
                    )?);
                    Ok(())
                })();
                if let Err(e) = result {
                    record.error = Some(format!("{e:#}"));
                }
                self.attempt("sensitivity",serde_json::json!({"candidate":candidate.id,"parameter":parameter,"direction":direction,"original":original,"changed":changed}),record.error.clone())?;
                results.push(record);
            }
        }
        write_json(&candidate.directory.join("sensitivity.json"), &results)?;
        Ok(results)
    }

    fn count_matched_comparisons(&self) -> Result<()> {
        let relative = |a: usize, b: usize| a.abs_diff(b) as f64 / a.max(b).max(1) as f64;
        let graphs: Vec<_> = self.graphs.values().collect();
        let mut graph_pairs = vec![];
        for i in 0..graphs.len() {
            for j in i + 1..graphs.len() {
                if relative(graphs[i].metrics.edges, graphs[j].metrics.edges)
                    <= self.manifest.similar_edge_count_relative_tolerance
                {
                    let left = load_graph(&graphs[i].directory.join("graph.json"))?;
                    let right = load_graph(&graphs[j].directory.join("graph.json"))?;
                    graph_pairs.push(serde_json::json!({"left":graphs[i].key,"right":graphs[j].key,"left_edges":left.edges.len(),"right_edges":right.edges.len(),"edge_jaccard":metrics::edge_overlap(&left,&right)}));
                }
            }
        }
        let mut partitions = vec![];
        for i in 0..self.candidates.len() {
            for j in i + 1..self.candidates.len() {
                let (a, b) = (&self.candidates[i], &self.candidates[j]);
                let (Some(ap), Some(bp)) =
                    (&a.representative_assignments, &b.representative_assignments)
                else {
                    continue;
                };
                let al = read_assignments(ap, &self.input.tokens)?;
                let bl = read_assignments(bp, &self.input.tokens)?;
                let ac = leiden::groups(&al).len();
                let bc = leiden::groups(&bl).len();
                if relative(ac, bc) <= self.manifest.similar_community_count_relative_tolerance {
                    partitions.push(serde_json::json!({"left":a.id,"right":b.id,"left_communities":ac,"right_communities":bc,"agreement":metrics::agreement(&al,&bl)?}));
                }
            }
        }
        write_json(
            &self.output.join("count-matched-comparisons.json"),
            &serde_json::json!({"relative_difference":"abs(a-b)/max(a,b,1)","graphs":graph_pairs,"partitions":partitions,"note":"Counts are comparison controls, never optimization targets. CPM objectives are not compared across graphs or resolutions."}),
        )
    }

    fn execute(&mut self) -> Result<()> {
        if let Some(baseline) = self.manifest.baseline.clone() {
            let source = GraphSource::Archived(ArchiveGraph {
                path: baseline.graph.clone(),
                threshold: baseline.threshold,
                unshift: false,
            });
            let archived = (|| -> Result<()> {
                let (graph, _) = self.graph(&source)?;
                let labels =
                    read_assignments(&baseline.recommended_assignments, &self.input.tokens)?;
                let (metrics, words) = metrics::partition_metrics(&self.input, &graph, &labels)?;
                ensure!(
                    metrics.all_communities_connected,
                    "Archived recommended partition has disconnected communities"
                );
                self.baseline_degrees = Some(graph.degrees());
                self.baseline_labels = Some(labels.clone());
                let directory = self.output.join("archived-recommended");
                fs::create_dir_all(&directory)?;
                write_assignments(
                    &directory.join("communities.csv"),
                    &self.input.tokens,
                    &labels,
                )?;
                write_json(&directory.join("metrics.json"), &metrics)?;
                write_json(
                    &directory.join("provenance.json"),
                    &serde_json::json!({"baseline":baseline,"assignments_sha256":sha256(&baseline.recommended_assignments)?,"graph_sha256":sha256(&baseline.graph)?}),
                )?;
                write_json(
                    &directory.join("strata.json"),
                    &metrics::stratify(
                        &self.input.tokens,
                        &words,
                        &self.metadata,
                        self.baseline_degrees.as_deref(),
                    ),
                )?;
                write_json(
                    &directory.join("null-control.json"),
                    &metrics::null_control(&self.input, &labels, &self.manifest.null_seeds),
                )?;
                Ok(())
            })();
            self.attempt(
                "archived-recommended",
                &baseline,
                archived.err().map(|e| format!("{e:#}")),
            )?;
            self.screen_one(
                &source,
                None,
                Some((baseline.resolution, baseline.theta)),
                "baseline",
                true,
                true,
            )?;
            for threshold in self.manifest.archived_thresholds.clone() {
                let raw = GraphSource::Archived(ArchiveGraph {
                    path: baseline.graph.clone(),
                    threshold,
                    unshift: false,
                });
                for r in self.manifest.archived_resolutions.clone() {
                    self.screen_one(&raw, None, Some((r, 0.30)), "controls", false, true)?;
                }
                let unshift = GraphSource::Archived(ArchiveGraph {
                    path: baseline.graph.clone(),
                    threshold,
                    unshift: true,
                });
                self.screen_grid(&unshift, "controls-unshifted", true)?;
            }
        }
        for graph in self.manifest.graphs.clone() {
            self.screen_grid(&GraphSource::Neighbors(graph), "core", false)?;
        }
        if self.manifest.ablations {
            let retained = self.retained_configs();
            let mut variants = BTreeMap::new();
            for base in retained {
                let mut changed = base.clone();
                changed.symmetrization = Symmetrization::Mutual;
                variants.insert(serde_json::to_string(&changed)?, changed);
                for tau in [0.2, 0.4] {
                    let mut changed = base.clone();
                    changed.tau = tau;
                    variants.insert(serde_json::to_string(&changed)?, changed);
                }
                let mut changed = base.clone();
                changed.weight = WeightMode::CosineSnn;
                variants.insert(serde_json::to_string(&changed)?, changed);
                for representation in [
                    Representation::Center,
                    Representation::Abtt(1),
                    Representation::Abtt(3),
                ] {
                    let mut changed = base.clone();
                    changed.representation = representation;
                    variants.insert(serde_json::to_string(&changed)?, changed);
                }
            }
            for config in variants.into_values() {
                self.screen_grid(&GraphSource::Neighbors(config), "ablations", false)?;
            }
        }
        if self.manifest.combinations {
            let retained = self.retained_configs();
            let mut variants = BTreeMap::new();
            for base in &retained {
                let same_k: Vec<_> = retained.iter().filter(|g| g.k == base.k).collect();
                for topology in &same_k {
                    for threshold in &same_k {
                        for weights in &same_k {
                            for representation in [
                                Representation::Raw,
                                Representation::Center,
                                Representation::Abtt(1),
                                Representation::Abtt(3),
                            ] {
                                let mut config = base.clone();
                                config.symmetrization = topology.symmetrization;
                                config.tau = threshold.tau;
                                config.weight = weights.weight;
                                config.representation = representation;
                                variants.insert(serde_json::to_string(&config)?, config);
                            }
                        }
                    }
                }
            }
            for config in variants.into_values() {
                self.screen_grid(&GraphSource::Neighbors(config), "combinations", false)?;
            }
        }
        let scores: Vec<_> = self.candidates.iter().map(|c| c.score.clone()).collect();
        let mut selection = selection::select(&scores, self.manifest.selection.as_ref())?;
        if selection.nominee.is_some() && self.baseline_labels.is_none() {
            selection.nominee = None;
            selection.reason =
                "Archived recommended baseline could not be loaded; automatic nomination disabled"
                    .into();
        }
        if let Some(nominee) = &selection.nominee {
            let candidate = self.candidates.iter().find(|c| &c.id == nominee).unwrap();
            if self.expansion_limited.contains(&candidate.graph_key) {
                selection.nominee = None;
                selection.reason="Nominee lies in a graph search that reached its expansion budget; baseline retained".into();
            }
        }
        // Freeze before observing fresh confirmation seeds or perturbations.
        write_json(&self.output.join("development-selection.json"), &selection)?;
        write_json(
            &self.output.join("search-limits.json"),
            &serde_json::json!({"endpoint_limited_graphs":self.expansion_limited,"max_endpoint_expansions":self.manifest.max_endpoint_expansions}),
        )?;
        let finalists: Vec<_> = self
            .candidates
            .iter()
            .filter(|c| selection.pareto.contains(&c.id) || c.control || c.baseline)
            .cloned()
            .collect();
        for candidate in &finalists {
            let sensitivity = self.sensitivity(candidate)?;
            if let Some(c) = self.candidates.iter_mut().find(|c| c.id == candidate.id) {
                c.sensitivity = sensitivity;
                write_json(&c.directory.join("candidate.json"), c)?;
            }
        }
        self.save_tables()?;
        self.count_matched_comparisons()?;
        let mut confirmation = vec![];
        if self.manifest.confirmation {
            for candidate in finalists {
                let result = self.evaluate(
                    &candidate.source,
                    candidate.q,
                    Some((candidate.resolution, candidate.theta)),
                    "confirmation",
                    candidate.baseline,
                    candidate.control,
                    &self.manifest.confirmation_seeds.clone(),
                    &self.manifest.confirmation_perturbation_seeds.clone(),
                );
                match result {
                    Ok(mut c) => {
                        c.sensitivity = self.sensitivity(&c)?;
                        write_json(&c.directory.join("candidate.json"), &c)?;
                        confirmation.push(c);
                    }
                    Err(e) => {
                        self.attempt("confirmation", &candidate.id, Some(format!("{e:#}")))?
                    }
                }
            }
        }
        write_json(&self.output.join("confirmation.json"), &confirmation)?;
        let accepted = match (&selection.nominee, &self.manifest.selection) {
            (Some(id), Some(policy)) => {
                let nominee = confirmation.iter().find(|c| &c.id == id);
                let baseline = confirmation.iter().find(|c| c.baseline);
                match (nominee, baseline) {
                    (Some(a), Some(b)) => {
                        selection::confirmation_accepts(&a.score, &b.score, policy)
                    }
                    _ => false,
                }
            }
            _ => false,
        };
        let exported = if accepted {
            let c = confirmation
                .iter()
                .find(|c| Some(&c.id) == selection.nominee.as_ref())
                .unwrap();
            let labels = read_assignments(
                c.representative_assignments.as_ref().unwrap(),
                &self.input.tokens,
            )?;
            write_assignments(
                &self.output.join("communities.csv"),
                &self.input.tokens,
                &labels,
            )?;
            Some(c.id.clone())
        } else if let Some(labels) = &self.baseline_labels {
            write_assignments(
                &self.output.join("communities.csv"),
                &self.input.tokens,
                labels,
            )?;
            Some("archived-recommended".into())
        } else {
            None
        };
        write_json(
            &self.output.join("selection-report.json"),
            &serde_json::json!({"development":selection,"confirmation_accepted":accepted,
            "default":exported,"baseline_retained":!accepted,"external_evaluation":null,"external_evaluation_reason":"No external evaluation data available",
            "note":"Confirmation accepts or rejects the frozen nomination; it never chooses a different candidate or retunes settings. Cohesion reflects source geometry; stability is not semantic ground truth."}),
        )?;
        write_atomic(&self.output.join("REPORT.md"), |w| {
            writeln!(w,"# Graph v2 experiment\n\n{} candidate settings; {} recorded attempts.\n\n{}\n\nConfirmation accepted: **{}**. Exported default: `{}`.\n\nSee `candidates.csv`, `development-selection.json`, `selection-report.json`, and each candidate's metrics, repeat distributions, perturbations, sensitivity and strata. Missing reference metrics are null because no external evaluation data is available.\n\nCPM objectives are comparable only for the same graph at the same resolution.",self.candidates.len(),self.attempts.len(),selection.reason,accepted,exported.as_deref().unwrap_or("none"))?;
            Ok(())
        })?;
        ensure!(
            self.candidates.iter().any(|c| c.score.valid),
            "Every candidate failed; inspect attempts.json"
        );
        Ok(())
    }
}

pub fn experiment_command(manifest: &Manifest, output: &Path) -> Result<()> {
    manifest.validate()?;
    fresh_directory(output)?;
    let start = Instant::now();
    write_json(&output.join("manifest.json"), manifest)?;
    let result = (|| -> Result<()> {
        let input = Embeddings::load(&manifest.input)?;
        let input_hash = sha256(&manifest.input)?;
        let metadata = manifest
            .metadata
            .as_ref()
            .map(|p| metrics::load_metadata(p))
            .transpose()?
            .unwrap_or_default();
        write_json(
            &output.join("provenance.json"),
            &serde_json::json!({"version":VERSION,"backend":leiden::BACKEND,"input_sha256":input_hash,"code_hashes":code_hashes(),"executable_sha256":sha256(&std::env::current_exe()?)?,
            "metadata_sha256":manifest.metadata.as_ref().map(|p|sha256(p)).transpose()?,"command":std::env::args().collect::<Vec<_>>(),
            "metric_conventions":"Linear-interpolated quantiles; population dispersion; medians across solver/stress repeats; word-weighted source-space coherence; no external labels",
            "path_resolution":"Manifest paths are resolved relative to the invocation working directory"}),
        )?;
        let mut runner = Runner {
            manifest: manifest.clone(),
            input,
            input_hash,
            output: output.into(),
            metadata,
            baseline_degrees: None,
            baseline_labels: None,
            graphs: BTreeMap::new(),
            candidates: vec![],
            attempts: vec![],
            expansion_limited: BTreeSet::new(),
        };
        runner.execute()
    })();
    write_json(
        &output.join("experiment-status.json"),
        &serde_json::json!({"status":if result.is_ok(){"complete"}else{"failed"},"error":result.as_ref().err().map(|e|format!("{e:#}")),"cost":cost(start,output)?}),
    )?;
    result
}
