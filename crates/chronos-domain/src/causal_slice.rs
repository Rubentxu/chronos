//! Conservative backward causal slice over evidence nodes/edges.
//!
//! Given a sink (e.g. a violating state write), walk backward through declared
//! causal/dataflow edges to the evidence that produced it. Only nodes connected
//! backward through declared edges are included (unrelated work is excluded),
//! and any included node whose evidence is not observed is reported in
//! `missing` — never silently dropped ("missing evidence must remain visible").

use std::collections::{HashMap, HashSet, VecDeque};

/// Stable identifier for an evidence node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EvidenceNodeId(pub u64);

/// A unit of evidence with a declared observed/unobserved status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvidenceNode {
    pub id: EvidenceNodeId,
    pub observed: bool,
}

/// A causal/dataflow edge from a predecessor/producer to a dependent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CausalEdge {
    pub from: EvidenceNodeId,
    pub to: EvidenceNodeId,
}

/// Result of a backward causal slice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CausalSlice {
    /// Nodes reachable backward from the sink, in deterministic BFS order.
    pub included: Vec<EvidenceNodeId>,
    /// Included nodes whose evidence is not observed (gaps), never dropped.
    pub missing: Vec<EvidenceNodeId>,
}

/// Compute the conservative backward causal slice from `sink` over `edges`.
///
/// Nodes are considered observed per `observed`; a node absent from the map is
/// treated as unobserved (an evidence gap surfaced in `missing`).
pub fn slice_from(
    edges: &[CausalEdge],
    observed: &HashMap<EvidenceNodeId, bool>,
    sink: EvidenceNodeId,
) -> CausalSlice {
    // Reverse adjacency: dependent `to` -> its predecessors `from`.
    let mut preds: HashMap<EvidenceNodeId, Vec<EvidenceNodeId>> = HashMap::new();
    for e in edges {
        preds.entry(e.to).or_default().push(e.from);
    }

    let mut included = Vec::new();
    let mut seen: HashSet<EvidenceNodeId> = HashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back(sink);

    while let Some(node) = queue.pop_front() {
        if !seen.insert(node) {
            continue;
        }
        included.push(node);
        let mut ps = preds.get(&node).cloned().unwrap_or_default();
        ps.sort_unstable(); // deterministic tie-break (ascending id)
        for p in ps {
            queue.push_back(p);
        }
    }

    let missing = included
        .iter()
        .copied()
        .filter(|id| observed.get(id) != Some(&true))
        .collect();

    CausalSlice { included, missing }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn node(id: u64) -> EvidenceNodeId {
        EvidenceNodeId(id)
    }

    #[test]
    fn slice_contains_producer_and_excludes_unrelated() {
        let total = node(1);
        let discount = node(2);
        let tax = node(3);
        let unrelated = node(99);
        let edges = [
            CausalEdge {
                from: discount,
                to: total,
            },
            CausalEdge {
                from: tax,
                to: total,
            },
            CausalEdge {
                from: node(4),
                to: unrelated,
            },
        ];
        let observed = HashMap::from([
            (total, true),
            (discount, true),
            (tax, true),
            (unrelated, true),
            (node(4), true),
        ]);
        let slice = slice_from(&edges, &observed, total);
        assert_eq!(slice.included, vec![total, discount, tax]);
        assert!(slice.missing.is_empty());
    }

    #[test]
    fn missing_evidence_stays_visible() {
        let total = node(1);
        let discount = node(2);
        let edges = [CausalEdge {
            from: discount,
            to: total,
        }];
        let observed = HashMap::from([(total, true)]); // discount unobserved/absent
        let slice = slice_from(&edges, &observed, total);
        assert_eq!(slice.included, vec![total, discount]);
        assert_eq!(slice.missing, vec![discount]);
    }

    #[test]
    fn no_incoming_sink_includes_only_sink() {
        let total = node(1);
        let slice = slice_from(&[], &HashMap::from([(total, true)]), total);
        assert_eq!(slice.included, vec![total]);
        assert!(slice.missing.is_empty());
        // unobserved sink surfaces its own gap
        let slice2 = slice_from(&[], &HashMap::new(), total);
        assert_eq!(slice2.missing, vec![total]);
    }
}
