use anyhow::{ensure, Result};
use graphrs::{
    algorithms::community::leiden::{self, QualityFunction},
    Graph,
};
use std::collections::HashSet;
use std::path::Path;

use crate::output::write_atomic;

pub struct LeidenConfig {
    pub quality: QualityFunction,
    pub resolution: f64,
    pub theta: f64,
    pub gamma: f64,
}

impl Default for LeidenConfig {
    fn default() -> Self {
        Self {
            quality: QualityFunction::CPM,
            resolution: 0.05,
            theta: 0.3,
            gamma: 0.05,
        }
    }
}

impl LeidenConfig {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.resolution.is_finite() && self.resolution >= 0.0,
            "Resolution must be finite and non-negative"
        );
        ensure!(
            self.theta.is_finite() && self.theta > 0.0,
            "Theta must be finite and strictly positive"
        );
        ensure!(
            self.gamma.is_finite() && self.gamma >= 0.0,
            "Gamma must be finite and non-negative"
        );
        Ok(())
    }
}

/// Validate the graph, run Leiden and verify that its partition covers every node
/// exactly once. Ordering is canonical; the upstream algorithm is still random.
pub fn detect_communities(
    graph: &Graph<String, ()>,
    config: LeidenConfig,
) -> Result<Vec<Vec<String>>> {
    config.validate()?;
    ensure!(!graph.specs.directed, "Leiden requires an undirected graph");
    let mut positive_weight = false;
    for edge in graph.get_all_edges() {
        ensure!(
            edge.weight.is_finite() && edge.weight >= 0.0,
            "Leiden requires finite non-negative weights; invalid edge {:?} -> {:?}",
            edge.u,
            edge.v
        );
        positive_weight |= edge.weight > 0.0;
    }
    let communities = if positive_weight {
        leiden::leiden(
            graph,
            true,
            config.quality,
            Some(config.resolution),
            Some(config.theta),
            Some(config.gamma),
        )
        .map_err(|error| anyhow::anyhow!("Leiden community detection failed: {error}"))?
    } else {
        // Avoid undefined modularity on a graph with zero total edge weight.
        graph
            .get_all_nodes()
            .iter()
            .map(|node| HashSet::from([node.name.clone()]))
            .collect()
    };
    canonicalize_partition(graph, communities)
}

fn canonicalize_partition(
    graph: &Graph<String, ()>,
    communities: Vec<HashSet<String>>,
) -> Result<Vec<Vec<String>>> {
    let mut remaining: HashSet<_> = graph
        .get_all_nodes()
        .iter()
        .map(|node| node.name.clone())
        .collect();
    let mut result = Vec::with_capacity(communities.len());
    for community in communities {
        ensure!(!community.is_empty(), "Leiden returned an empty community");
        let mut members: Vec<_> = community.into_iter().collect();
        for token in &members {
            ensure!(
                remaining.remove(token),
                "Leiden returned an unknown or duplicate token {token:?}"
            );
        }
        members.sort_unstable();
        result.push(members);
    }
    ensure!(
        remaining.is_empty(),
        "Leiden omitted {} nodes",
        remaining.len()
    );
    result.sort_unstable_by(|a, b| a[0].cmp(&b[0]));
    Ok(result)
}

pub fn write_assignments(path: &Path, communities: &[Vec<String>]) -> Result<usize> {
    let mut assignments: Vec<_> = communities
        .iter()
        .enumerate()
        .flat_map(|(id, members)| members.iter().map(move |token| (token.as_str(), id)))
        .collect();
    assignments.sort_unstable_by(|a, b| a.0.cmp(b.0));
    write_atomic(path, |output| {
        let mut writer = csv::Writer::from_writer(output);
        writer.write_record(["token", "community"])?;
        for &(token, community) in &assignments {
            writer.serialize((token, community))?;
        }
        writer.flush()?;
        Ok(assignments.len())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphrs::{Edge, GraphSpecs, Node};

    #[test]
    fn leiden_covers_disconnected_pairs_and_isolate() -> Result<()> {
        let graph = Graph::new_from_nodes_and_edges(
            ["a", "b", "c", "d", "isolate"]
                .map(|name| Node::from_name(name.to_string()))
                .to_vec(),
            vec![
                Edge::with_weight("a".into(), "b".into(), 0.8),
                Edge::with_weight("c".into(), "d".into(), 0.8),
            ],
            GraphSpecs::undirected(),
        )
        .unwrap();
        let communities = detect_communities(&graph, LeidenConfig::default())?;
        assert_eq!(
            communities,
            [vec!["a", "b"], vec!["c", "d"], vec!["isolate"]]
        );
        Ok(())
    }

    #[test]
    fn csv_round_trips_commas_quotes_and_unicode() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("communities.csv");
        let communities = vec![vec!["中,文".into(), "带\"引号".into(), "a".into()]];
        assert_eq!(write_assignments(&path, &communities)?, 3);
        let records = csv::Reader::from_path(path)?
            .records()
            .collect::<std::result::Result<Vec<_>, _>>()?;
        assert_eq!(&records[0][0], "a");
        assert_eq!(&records[1][0], "中,文");
        assert_eq!(&records[2][0], "带\"引号");
        Ok(())
    }

    #[test]
    fn invalid_settings_fail_before_algorithm() {
        for theta in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(LeidenConfig {
                theta,
                ..Default::default()
            }
            .validate()
            .is_err());
        }
    }
}
