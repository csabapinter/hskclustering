use anyhow::{ensure, Context, Result};
use quick_xml::{
    events::{BytesStart, Event},
    Reader, Writer,
};
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use crate::output::{ensure_distinct_paths, write_atomic};

#[derive(Debug, Default)]
pub struct FilterStats {
    pub nodes: u64,
    pub edges_read: u64,
    pub edges_kept: u64,
}

struct PendingEdge {
    xml: Writer<Vec<u8>>,
    weight: Option<f64>,
    reading_weight: bool,
    weight_text: String,
}

impl PendingEdge {
    fn new() -> Self {
        Self {
            xml: Writer::new(Vec::new()),
            weight: None,
            reading_weight: false,
            weight_text: String::new(),
        }
    }
}

/// Filter a weighted GraphML file using O(one edge) memory. All nodes, graph
/// metadata and attributes of retained edges are copied through unchanged.
/// Edge weights must be explicit data values (GraphML defaults are unsupported).
pub fn filter_graphml(input: &Path, output: &Path, threshold: f64) -> Result<FilterStats> {
    super::validate_threshold(threshold)?;
    ensure_distinct_paths(input, output)?;
    let reader = BufReader::new(File::open(input)?);
    write_atomic(output, |writer| filter_stream(reader, writer, threshold))
        .with_context(|| format!("Failed to filter {}", input.display()))
}

fn attribute(element: &BytesStart<'_>, name: &str) -> Result<Option<String>> {
    element
        .try_get_attribute(name)?
        .map(|attribute| {
            Ok(attribute
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)?
                .into_owned())
        })
        .transpose()
}

fn filter_stream(input: impl BufRead, output: impl Write, threshold: f64) -> Result<FilterStats> {
    let mut reader = Reader::from_reader(input);
    let mut writer = Writer::new(output);
    let mut buffer = Vec::new();
    let mut weight_keys = HashSet::new();
    let mut pending: Option<PendingEdge> = None;
    let mut stats = FilterStats::default();
    let mut depth = 0usize;
    let mut root_seen = false;
    let mut graph_seen = false;
    loop {
        let event = reader
            .read_event_into(&mut buffer)
            .context("Invalid GraphML XML")?;
        match &event {
            Event::Start(element) | Event::Empty(element) => {
                ensure!(
                    !pending.as_ref().is_some_and(|edge| edge.reading_weight),
                    "Edge weight must contain numeric text only"
                );
                if depth == 0 {
                    ensure!(
                        !root_seen && element.name().as_ref() == "graphml",
                        "Expected a single <graphml> root"
                    );
                    root_seen = true;
                }
                match element.name().as_ref() {
                    "graph" => {
                        ensure!(!graph_seen, "Only one non-nested graph is supported");
                        graph_seen = true;
                    }
                    "key" if attribute(element, "attr.name")?.as_deref() == Some("weight") => {
                        let scope = attribute(element, "for")?;
                        if matches!(scope.as_deref(), None | Some("edge" | "all")) {
                            weight_keys.insert(
                                attribute(element, "id")?.context("Weight key missing id")?,
                            );
                        }
                    }
                    "node" => stats.nodes += 1,
                    "edge" => {
                        ensure!(pending.is_none(), "Nested GraphML edges are unsupported");
                        ensure!(
                            matches!(event, Event::Start(_)),
                            "Edge is missing an explicit weight"
                        );
                        ensure!(
                            attribute(element, "source")?.is_some()
                                && attribute(element, "target")?.is_some(),
                            "Edge is missing source or target"
                        );
                        pending = Some(PendingEdge::new());
                    }
                    "data" => {
                        if let Some(edge) = &mut pending {
                            if attribute(element, "key")?
                                .is_some_and(|key| weight_keys.contains(&key))
                            {
                                ensure!(matches!(event, Event::Start(_)), "Empty edge weight");
                                ensure!(
                                    edge.weight.is_none() && !edge.reading_weight,
                                    "Duplicate edge weight"
                                );
                                edge.reading_weight = true;
                            }
                        }
                    }
                    _ => {}
                }
                if matches!(event, Event::Start(_)) {
                    depth += 1;
                }
            }
            Event::Text(text) => {
                if let Some(edge) = pending.as_mut().filter(|edge| edge.reading_weight) {
                    edge.weight_text.push_str(text);
                }
            }
            Event::CData(text) => {
                if let Some(edge) = pending.as_mut().filter(|edge| edge.reading_weight) {
                    edge.weight_text.push_str(text);
                }
            }
            Event::GeneralRef(_) if pending.as_ref().is_some_and(|edge| edge.reading_weight) => {
                anyhow::bail!("Entity references in edge weights are unsupported");
            }
            Event::End(element) => {
                depth = depth.checked_sub(1).context("Unexpected XML closing tag")?;
                if element.name().as_ref() == "data" {
                    if let Some(edge) = pending.as_mut().filter(|edge| edge.reading_weight) {
                        let weight: f64 = edge
                            .weight_text
                            .trim()
                            .parse()
                            .context("Invalid edge weight")?;
                        ensure!(weight.is_finite(), "Non-finite edge weight");
                        edge.weight = Some(weight);
                        edge.reading_weight = false;
                    }
                }
            }
            Event::Eof => {
                ensure!(
                    depth == 0 && root_seen && graph_seen && pending.is_none(),
                    "Incomplete GraphML document"
                );
                break;
            }
            _ => {}
        }
        if let Some(edge) = &mut pending {
            edge.xml.write_event(event.borrow())?;
            if matches!(&event, Event::End(end) if end.name().as_ref() == "edge") {
                let edge = pending.take().context("Missing pending edge")?;
                let weight = edge.weight.context("Edge is missing an explicit weight")?;
                stats.edges_read += 1;
                if weight >= threshold {
                    writer.get_mut().write_all(&edge.xml.into_inner())?;
                    stats.edges_kept += 1;
                }
            }
        } else {
            writer.write_event(event.borrow())?;
        }
        buffer.clear();
    }
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph_io::{build_graph_and_write_graphml, build_threshold_graph, read_graph};

    #[test]
    fn streaming_filter_matches_threshold_during_build() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let full = dir.path().join("full.graphml");
        let filtered = dir.path().join("filtered.graphml");
        let direct = dir.path().join("direct.graphml");
        let tokens = vec!["一".into(), "二".into(), "三".into()];
        let vectors = vec![vec![1.0, 0.0], vec![1.0, 0.0], vec![-1.0, 0.0]];
        build_graph_and_write_graphml(&tokens, &vectors, &full)?;
        build_threshold_graph(&tokens, &vectors, &direct, 0.7)?;
        let stats = filter_graphml(&full, &filtered, 0.7)?;
        assert_eq!((stats.nodes, stats.edges_read, stats.edges_kept), (3, 3, 1));
        let graph = read_graph(&filtered)?;
        assert_eq!(graph.number_of_nodes(), 3);
        let edge = graph.get_all_edges()[0];
        let reference = read_graph(&direct)?;
        let expected = reference.get_all_edges()[0];
        assert_eq!(
            (&edge.u, &edge.v, edge.weight),
            (&expected.u, &expected.v, expected.weight)
        );
        Ok(())
    }

    #[test]
    fn malformed_xml_and_missing_weights_fail() {
        for xml in [
            "",
            "<graphml><graph>",
            "<graphml><graph><edge source=\"a\" target=\"b\"/></graph></graphml>",
        ] {
            assert!(filter_stream(xml.as_bytes(), Vec::new(), 0.7).is_err());
        }
    }

    #[test]
    fn threshold_is_inclusive_and_metadata_is_preserved() -> Result<()> {
        let xml = r#"<graphml><key id="w" for="edge" attr.name="weight" attr.type="double"/><graph edgedefault="undirected"><node id="a"><data key="label">A&amp;B</data></node><node id="b"/><edge source="a" target="b" id="e1"><data key="w">0.7</data><data key="label">keep me</data></edge></graph></graphml>"#;
        let mut output = Vec::new();
        let stats = filter_stream(xml.as_bytes(), &mut output, 0.7)?;
        assert_eq!((stats.nodes, stats.edges_kept), (2, 1));
        assert_eq!(String::from_utf8(output)?, xml);
        for weight in ["NaN", "inf", "invalid", "<nested>0.7</nested>"] {
            let invalid = xml.replace(">0.7<", &format!(">{weight}<"));
            assert!(filter_stream(invalid.as_bytes(), Vec::new(), 0.7).is_err());
        }
        Ok(())
    }
}
