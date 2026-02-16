//! Module containing the parser for callgrind flamegraphs
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use log::debug;

use super::hashmap_parser::{CallgrindMap, HashMapParser, Id, SourcePath};
use super::model::Metrics;
use super::parser::{CallgrindParser, CallgrindProperties, Sentinel};
use crate::api::EventKind;
use crate::runner::metrics::Metric;

/// The `FlamegraphMap` based on a [`CallgrindMap`]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlamegraphMap {
    roots: Vec<Id>,
    costs: CallgrindMap,
    edges: HashMap<Id, HashMap<Id, Metrics>>,
}

/// The parser for flamegraphs
#[derive(Debug)]
pub struct FlamegraphParser {
    project_root: PathBuf,
    sentinel: Option<Sentinel>,
    min_cost: u64,
}

impl FlamegraphMap {
    /// Return true if this map is empty
    pub fn is_empty(&self) -> bool {
        self.costs.is_empty()
    }

    /// Calculate the cache summary for each entry in the map in-place
    pub fn make_summary(&mut self) -> Result<()> {
        let mut iter = self.costs.map.values_mut().peekable();
        if let Some(value) = iter.peek() {
            // If one cost can be summarized then all costs can be summarized.
            if value.metrics.can_summarize() {
                for value in iter {
                    value
                        .metrics
                        .make_summary()
                        .map_err(|error| anyhow!("Failed calculating summary events: {error}"))?;
                }
            }
        }

        Ok(())
    }

    /// Sum this map with another map
    pub fn add(&mut self, other: &Self) {
        for (other_id, other_value) in &other.costs {
            // The performance of HashMap::entry is worse than the following method because we have
            // a heavy id which needs to be cloned, although it is already present in the map.
            if let Some(value) = self.costs.map.get_mut(other_id) {
                value.metrics.add(&other_value.metrics);
            } else {
                self.costs.map.insert(other_id.clone(), other_value.clone());
            }
        }

        for (caller, callee_map) in &other.edges {
            if let Some(entry) = self.edges.get_mut(caller) {
                for (callee, metrics) in callee_map {
                    if let Some(m) = entry.get_mut(callee) {
                        m.add(metrics);
                    } else {
                        entry.insert(callee.clone(), metrics.clone());
                    }
                }
            } else {
                self.edges.insert(caller.clone(), callee_map.clone());
            }
        }

        // TODO: recompute roots after merge
    }

    /// Convert to stacks string format for this `EventType`
    ///
    /// # Errors
    ///
    /// If the event type was not present in the stacks
    pub fn to_stack_format(&self, event_kind: &EventKind) -> Result<Vec<String>> {
        if self.costs.map.is_empty() {
            return Ok(vec![]);
        }

        for (_id, value) in &self.costs {
            value.metrics.metric_by_kind(event_kind).ok_or_else(|| {
                anyhow!("Failed creating flamegraph stack: Missing event type '{event_kind}'")
            })?;
        }

        let mut stacks: Vec<String> = vec![];
        let mut visited: HashSet<&Id> = HashSet::new();
        for root in &self.roots {
            self.dfs_emit(root, "", event_kind, &mut stacks, None, &mut visited);
        }

        Ok(stacks)
    }

    fn dfs_emit<'a>(
        &'a self,
        id: &'a Id,
        parent_stack: &str,
        event_kind: &EventKind,
        stacks: &mut Vec<String>,
        edge_cost: Option<Metric>,
        visited: &mut HashSet<&'a Id>,
    ) {
        // Global visited: each function is emitted at most once, claimed by
        // the first DFS path that reaches it. This prevents infinite traversal
        // of cycles and avoids double-counting shared callees.
        if !visited.insert(id) {
            return;
        }

        // For root nodes (no incoming edge), use global inclusive cost from costs.map.
        // For non-root nodes, use the per-callsite edge cost from the parent.
        let inclusive = edge_cost.unwrap_or_else(|| {
            self.costs
                .map
                .get(id)
                .and_then(|v| v.metrics.metric_by_kind(event_kind))
                .unwrap_or(Metric::Int(0))
        });

        let mut source = String::new();
        if let Some(file) = &id.file {
            match file {
                SourcePath::Unknown => write!(source, "{}", id.func).unwrap(),
                SourcePath::Rust(obj_path)
                | SourcePath::Relative(obj_path)
                | SourcePath::Absolute(obj_path) => {
                    write!(source, "{}:{}", obj_path.display(), id.func).unwrap();
                }
            }
        } else {
            write!(source, "{}", id.func).unwrap();
        }
        if let Some(obj_path) = &id.obj {
            match obj_path {
                SourcePath::Unknown => {}
                SourcePath::Rust(p)
                | SourcePath::Relative(p)
                | SourcePath::Absolute(p) => {
                    write!(source, " [{}]", p.display()).unwrap();
                }
            }
        }

        let current_stack = if parent_stack.is_empty() {
            source
        } else {
            format!("{parent_stack};{source}")
        };

        // Collect children not yet visited, with their edge costs.
        let mut children: Vec<(&Id, Metric)> = self
            .edges
            .get(id)
            .map(|m| {
                m.iter()
                    .filter_map(|(callee, edge_metrics)| {
                        if visited.contains(callee) {
                            return None;
                        }
                        edge_metrics.metric_by_kind(event_kind).map(|c| (callee, c))
                    })
                    .collect()
            })
            .unwrap_or_default();
        children.sort_by(|(id_a, cost_a), (id_b, cost_b)| {
            cost_b.cmp(cost_a).then_with(|| id_a.cmp(id_b))
        });

        // Self-cost: inclusive minus children's edge costs.
        // Clamped to 0 because callgrind's phantom edges can make
        // outgoing edge costs exceed the incoming edge cost.
        let children_cost: Metric = children
            .iter()
            .map(|(_, c)| *c)
            .fold(Metric::Int(0), |acc, c| acc + c);

        let self_cost = if inclusive > children_cost {
            inclusive - children_cost
        } else {
            Metric::Int(0)
        };
        stacks.push(format!("{} {}", current_stack, self_cost));

        for (child_id, child_edge_cost) in &children {
            self.dfs_emit(
                child_id,
                &current_stack,
                event_kind,
                stacks,
                Some(*child_edge_cost),
                visited,
            );
        }
    }
}

impl FlamegraphParser {
    /// Create a new `FlamegraphParser`
    pub fn new<P>(sentinel: Option<&Sentinel>, project_root: P, min_cost: u64) -> Self
    where
        P: Into<PathBuf>,
    {
        Self {
            sentinel: sentinel.cloned(),
            project_root: project_root.into(),
            min_cost,
        }
    }
}

impl CallgrindParser for FlamegraphParser {
    type Output = FlamegraphMap;

    fn parse_single(&self, path: &Path) -> Result<(CallgrindProperties, Self::Output)> {
        debug!("Parsing flamegraph from file '{}'", path.display());

        let parser = HashMapParser {
            project_root: self.project_root.clone(),
            sentinel: self.sentinel.clone(),
        };

        let min_cost_metric = Metric::Int(self.min_cost);
        let mut callees = HashSet::new();
        let mut edges: HashMap<Id, HashMap<Id, Metrics>> = HashMap::new();

        let (props, costs) = parser.parse_with_edges(path, |caller_id, callee_id, metrics| {
            if self.min_cost > 0 {
                if let Some(ir_cost) = metrics.metric_by_kind(&EventKind::Ir) {
                    if ir_cost < min_cost_metric {
                        return;
                    }
                }
            }

            if let Some(callee_map) = edges.get_mut(caller_id) {
                if let Some(m) = callee_map.get_mut(callee_id) {
                    m.add(metrics);
                } else {
                    callee_map.insert(callee_id.clone(), metrics.clone());
                }
            } else {
                let mut callee_map = HashMap::new();
                callee_map.insert(callee_id.clone(), metrics.clone());
                edges.insert(caller_id.clone(), callee_map);
            }
            callees.insert(callee_id.clone());
        })?;

        let roots = if let Some(ref key) = costs.sentinel_key {
            vec![key.clone()]
        } else {
            let mut r: Vec<Id> = costs
                .map
                .keys()
                .filter(|id| !callees.contains(id) && edges.contains_key(id))
                .cloned()
                .collect();
            r.sort();
            r
        };

        Ok((
            props,
            FlamegraphMap {
                roots,
                costs,
                edges,
            },
        ))
    }
}
