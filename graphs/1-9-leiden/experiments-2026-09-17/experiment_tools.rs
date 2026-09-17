// Standalone experiment helper. Links the repository's existing release libraries.
// Application source and its CLI defaults are not changed.
use hskclustering::{embeddings::{load_embeddings, prepare_embeddings}, graph_io::build_threshold_graph};
use linfa::{prelude::{Fit, Transformer}, DatasetBase};
use linfa_reduction::Pca;
use ndarray::Array2;
use std::{collections::{BTreeMap, HashMap}, fs::{self, File}, io::{BufWriter, Write}, path::Path};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn hash_word(s: &str) -> u64 {
    s.bytes().fold(14695981039346656037u64, |h, b| (h ^ b as u64).wrapping_mul(1099511628211))
}

fn pca(input: &str, components: usize, out: &Path, target_edges: usize, full_fit: bool) -> Result<()> {
    fs::create_dir_all(out)?;
    let (tokens, vectors) = load_embeddings(input)?;
    let n = vectors.len(); let d = vectors[0].len();
    let data = Array2::from_shape_vec((n,d), vectors.iter().flatten().map(|v| *v as f64).collect())?;
    let mean = data.mean_axis(ndarray::Axis(0)).unwrap();
    let total_ss: f64 = data.rows().into_iter().map(|r| r.iter().zip(&mean).map(|(x,m)| (x-m).powi(2)).sum::<f64>()).sum();
    eprintln!("Fitting centered, unwhitened PCA with {components} components");
    let dataset = DatasetBase::from(data);
    let fitted_components=if full_fit {d} else {components};
    let model = Pca::params(fitted_components).whiten(false).fit(&dataset)?;
    let all_projected = model.transform(dataset).records;
    let projected = all_projected.slice(ndarray::s![..,..components]);
    let retained = projected.iter().map(|v| v*v).sum::<f64>() / total_ss;
    let mut basis = csv::Writer::from_path(out.join("pca-components.csv"))?;
    for row in model.components().rows().into_iter().take(components) { basis.serialize(row.to_vec())?; }
    basis.flush()?;
    let mut mean_csv = csv::Writer::from_path(out.join("pca-mean.csv"))?;
    mean_csv.serialize(mean.to_vec())?; mean_csv.flush()?;
    let vectors: Vec<Vec<f32>> = projected.rows().into_iter().map(|row| {
        let mut v: Vec<f32> = row.iter().map(|v| *v as f32).collect();
        let norm = v.iter().map(|v| (*v as f64).powi(2)).sum::<f64>().sqrt();
        for x in &mut v { *x = (*x as f64 / norm) as f32; }
        v
    }).collect();
    let mut embeddings = BufWriter::new(File::create(out.join("embeddings.txt"))?);
    writeln!(embeddings,"{n} {components}")?;
    for (token, vector) in tokens.iter().zip(&vectors) {
        write!(embeddings,"{token}")?;
        for value in vector { write!(embeddings," {value}")?; }
        writeln!(embeddings)?;
    }
    embeddings.flush()?;
    let mut histogram = vec![0usize;1001];
    for i in 0..n {
        for j in i+1..n {
            let weight = (hskclustering::embeddings::cosine_normalized(&vectors[i],&vectors[j]) as f64 + 1.0)/2.0;
            histogram[(weight*1000.0).floor().min(1000.0) as usize] += 1;
        }
    }
    let mut accumulated = 0usize; let mut best_bin = 1000usize; let mut best_diff = usize::MAX;
    let mut counts = vec![0usize;1001];
    for bin in (0..=1000).rev() {
        accumulated += histogram[bin]; counts[bin]=accumulated;
        let diff=accumulated.abs_diff(target_edges);
        if diff < best_diff { best_diff=diff; best_bin=bin; }
    }
    let threshold = best_bin as f64/1000.0;
    let tag=out.file_name().unwrap().to_str().unwrap();
    let graph = out.parent().unwrap().join("graphs").join(format!("{tag}-t{threshold:.3}.graphml"));
    let edges = build_threshold_graph(&tokens,&vectors,&graph,threshold)?;
    let info = format!("{{\"components\":{components},\"fitted_components\":{fitted_components},\"whiten\":false,\"fit_on\":\"all raw SGNS rows before L2 normalization\",\"retained_centered_variance\":{retained},\"target_edges\":{target_edges},\"threshold\":{threshold},\"edges\":{edges},\"mean_degree\":{},\"edges_at_0_700\":{},\"edges_at_0_725\":{},\"edges_at_0_750\":{}}}\n",2.0*edges as f64/n as f64,counts[700],counts[725],counts[750]);
    fs::write(out.join("pca.json"),&info)?;
    println!("{info}");
    Ok(())
}

fn audit_pca(input:&str,basis_file:&Path,out:&Path)->Result<()> {
    let (_,vectors)=load_embeddings(input)?;
    let n=vectors.len();let d=vectors[0].len();
    let data=Array2::from_shape_vec((n,d),vectors.iter().flatten().map(|v|*v as f64).collect())?;
    let mean=data.mean_axis(ndarray::Axis(0)).unwrap();
    let centered=data-&mean;
    let cov=centered.t().dot(&centered);
    let mut reader=csv::ReaderBuilder::new().has_headers(false).from_path(basis_file)?;
    let mut basis=Vec::new();
    for row in reader.records() {for val in row?.iter(){basis.push(val.parse::<f64>()?);}}
    let k=basis.len()/d;let basis=Array2::from_shape_vec((k,d),basis)?;
    let cv=cov.dot(&basis.t());
    let pc_cov=basis.dot(&cv);let mut residual=0.0;
    for i in 0..d {for j in 0..k {residual+=(cv[[i,j]]-pc_cov[[j,j]]*basis[[j,i]]).powi(2);}}
    let relative=(residual/cv.iter().map(|x|x*x).sum::<f64>()).sqrt();
    let gram=basis.dot(&basis.t());let mut orth=0.0f64;
    for i in 0..k {for j in 0..k {orth=orth.max((gram[[i,j]]-if i==j {1.0}else{0.0}).abs());}}
    let info=format!("{{\"components\":{k},\"relative_eigen_residual\":{relative},\"max_orthonormality_error\":{orth},\"eigen_residual_pass_1e_6\":{}}}\n",relative<1e-6);
    fs::write(out,&info)?;println!("{} {info}",basis_file.display());Ok(())
}

fn analyse(input: &str, assignments: &Path, out: &Path) -> Result<()> {
    fs::create_dir_all(out)?;
    let (tokens,vectors) = prepare_embeddings(input,None)?;
    let n=tokens.len(); let d=vectors[0].len();
    let index: HashMap<&str,usize> = tokens.iter().enumerate().map(|(i,s)|(s.as_str(),i)).collect();
    let mut grouped: BTreeMap<usize,Vec<usize>>=BTreeMap::new();
    let mut seen=vec![false;n];
    let mut reader=csv::Reader::from_path(assignments)?;
    for row in reader.records() {
        let row=row?; let i=*index.get(&row[0]).ok_or("unknown assignment token")?;
        if seen[i] { return Err("duplicate assignment".into()); }
        seen[i]=true; grouped.entry(row[1].parse()?).or_default().push(i);
    }
    if seen.iter().any(|x| !*x) { return Err("missing assignment".into()); }
    let ids:Vec<usize>=grouped.keys().copied().collect();
    let groups:Vec<Vec<usize>>=grouped.into_values().collect(); let k=groups.len();
    let mut membership=vec![0usize;n]; let mut means=Array2::<f32>::zeros((k,d));
    let mut within=vec![0.0;k];
    for (g, members) in groups.iter().enumerate() {
        let mut sum=vec![0.0f64;d]; let mut squared_norm_sum=0.0;
        for &i in members {
            membership[i]=g;
            for j in 0..d { sum[j]+=vectors[i][j] as f64; squared_norm_sum+=(vectors[i][j] as f64).powi(2); }
        }
        for j in 0..d { means[[g,j]]=(sum[j]/members.len() as f64) as f32; }
        if members.len()>1 { within[g]=(sum.iter().map(|v|v*v).sum::<f64>()-squared_norm_sum)/(members.len()*(members.len()-1)) as f64; }
    }
    let data=Array2::from_shape_vec((n,d),vectors.iter().flatten().copied().collect())?;
    // Dotting unit vectors against each cluster's unnormalized mean gives exact
    // average cosine similarity; the self term is removed for the own cluster.
    let similarities=data.dot(&means.t());
    let mut own=vec![0.0;n]; let mut margins=vec![0.0;n]; let mut silhouettes=vec![0.0;n]; let mut rivals=vec![0;n];
    for i in 0..n {
        let g=membership[i]; let size=groups[g].len();
        let self_dot=vectors[i].iter().map(|v|(*v as f64).powi(2)).sum::<f64>();
        own[i]=if size>1 {(similarities[[i,g]] as f64*size as f64-self_dot)/(size-1) as f64} else {0.0};
        let mut best=f64::NEG_INFINITY;
        for h in 0..k { if h!=g && similarities[[i,h]] as f64>best { best=similarities[[i,h]] as f64; rivals[i]=h; } }
        margins[i]=own[i]-best;
        if size>1 && k>1 { let a=1.0-own[i];let b=1.0-best;silhouettes[i]=(b-a)/a.max(b); }
    }
    // Cross-check the centroid identity against direct pairwise calculations.
    for members in groups.iter().filter(|m|m.len()>1).take(20) {
        let i=members[0];
        let direct=members.iter().filter(|&&j|j!=i).map(|&j| vectors[i].iter().zip(&vectors[j]).map(|(a,b)|*a as f64**b as f64).sum::<f64>()).sum::<f64>()/(members.len()-1) as f64;
        assert!((direct-own[i]).abs()<1e-5);
    }
    let mut words=csv::Writer::from_path(out.join("words.csv"))?;
    words.write_record(["token","community","size","mean_cosine_to_own_others","nearest_other_community","cosine_margin","cosine_silhouette"])?;
    for i in 0..n { words.serialize((&tokens[i],ids[membership[i]],groups[membership[i]].len(),own[i],ids[rivals[i]],margins[i],silhouettes[i]))?; }
    words.flush()?;
    let mut clusters=csv::Writer::from_path(out.join("clusters.csv"))?;
    clusters.write_record(["community","size","mean_pair_cosine","mean_cosine_silhouette","negative_silhouette_fraction","representatives","boundary_words","deterministic_sample","all_members"])?;
    for (g,members) in groups.iter().enumerate() {
        let size=members.len();let s=members.iter().map(|&i|silhouettes[i]).sum::<f64>()/size as f64;
        let negative=members.iter().filter(|&&i|silhouettes[i]<0.0).count() as f64/size as f64;
        let mut central=members.clone();central.sort_by(|&a,&b|own[b].total_cmp(&own[a]));
        let mut boundary=members.clone();boundary.sort_by(|&a,&b|margins[a].total_cmp(&margins[b]));
        let mut sample=members.clone();sample.sort_by_key(|&i|hash_word(&tokens[i]));
        let join=|items:&[usize],take:usize|items.iter().take(take).map(|&i|tokens[i].as_str()).collect::<Vec<_>>().join(" ");
        clusters.serialize((ids[g],size,if size>1 {Some(within[g])} else {None},s,negative,join(&central,12),join(&boundary,8),join(&sample,12),join(&central,size)))?;
    }
    clusters.flush()?;
    let mut sizes:Vec<usize>=groups.iter().map(|g|g.len()).collect();sizes.sort_unstable();
    let singletons=sizes.iter().filter(|&&s|s==1).count();
    let nonsingleton_cohesion=groups.iter().enumerate().filter(|(_,m)|m.len()>1).map(|(g,m)|within[g]*m.len() as f64).sum::<f64>()/(n-singletons) as f64;
    let silhouette=silhouettes.iter().sum::<f64>()/n as f64;
    let negative_fraction=silhouettes.iter().filter(|&&s|s<0.0).count() as f64/n as f64;
    let median=(sizes[(k-1)/2]+sizes[k/2]) as f64/2.0;
    let info=format!("{{\"n\":{n},\"communities\":{k},\"median_size\":{median},\"largest\":{},\"singletons\":{singletons},\"words_in_pairs\":{},\"words_in_3_to_30\":{},\"words_in_over_100\":{},\"mean_pair_cosine_word_weighted_nonsingletons\":{nonsingleton_cohesion},\"mean_cosine_silhouette_original_300d\":{silhouette},\"negative_silhouette_fraction\":{negative_fraction}}}\n",sizes[k-1],sizes.iter().filter(|&&s|s==2).sum::<usize>(),sizes.iter().filter(|&&s|(3..=30).contains(&s)).sum::<usize>(),sizes.iter().filter(|&&s|s>100).sum::<usize>());
    fs::write(out.join("metrics.json"),&info)?;
    println!("{} {info}",assignments.display());
    Ok(())
}

fn main() -> Result<()> {
    let args:Vec<String>=std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("pca")=>pca(&args[2],args[3].parse()?,Path::new(&args[4]),args[5].parse()?,args.get(6).map(String::as_str)==Some("full")),
        Some("audit-pca")=>audit_pca(&args[2],Path::new(&args[3]),Path::new(&args[4])),
        Some("analyse")=>analyse(&args[2],Path::new(&args[3]),Path::new(&args[4])),
        _=>Err("Use: pca EMBEDDINGS COMPONENTS OUT TARGET_EDGES | analyse EMBEDDINGS ASSIGNMENTS OUT".into())
    }
}
