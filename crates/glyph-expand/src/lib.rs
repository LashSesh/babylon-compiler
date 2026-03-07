//! Macro expansion engine.
//! Implements Section 20 of the Glyph Foundry Specification v2.1.

use glyph_ir::IrDocument;
use glyph_registry::MacroRegistry;
use serde::{Deserialize, Serialize};

/// Record of a macro expansion that occurred.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpansionRecord {
    pub pattern: String,
    pub nodes_added: usize,
    pub edges_added: usize,
}

/// Expand macros in an IR document according to the macro registry.
///
/// Requirements:
/// - Deterministic given (document, registry) [Req 20.1]
/// - Precedence: lowest value wins, ties → lexicographic [Req 20.2]
/// - Re-canonicalize after expansion [Req 20.3]
pub fn expand(doc: &mut IrDocument, registry: &MacroRegistry) -> Vec<ExpansionRecord> {
    let mut records = Vec::new();

    if registry.entries.is_empty() {
        return records;
    }

    // Sort entries by (precedence, pattern) for deterministic ordering
    let mut sorted_entries = registry.entries.clone();
    sorted_entries.sort_by(|a, b| {
        a.precedence
            .cmp(&b.precedence)
            .then_with(|| a.pattern.cmp(&b.pattern))
    });

    for entry in &sorted_entries {
        // Pattern matching against IR subgraphs
        // For now, simple string-based pattern matching on node names
        let mut nodes_added = 0;
        let mut edges_added = 0;

        // Check if any node name matches the pattern
        let matches: Vec<usize> = doc
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.name == entry.pattern)
            .map(|(i, _)| i)
            .collect();

        if !matches.is_empty() {
            // Apply expansion: add nodes and edges from the template
            for node_val in &entry.expansion.nodes {
                if let Ok(node) = serde_json::from_value::<glyph_ir::IrNode>(node_val.clone()) {
                    doc.nodes.push(node);
                    nodes_added += 1;
                }
            }
            for edge_val in &entry.expansion.edges {
                if let Ok(edge) = serde_json::from_value::<glyph_ir::IrEdge>(edge_val.clone()) {
                    doc.edges.push(edge);
                    edges_added += 1;
                }
            }

            if nodes_added > 0 || edges_added > 0 {
                records.push(ExpansionRecord {
                    pattern: entry.pattern.clone(),
                    nodes_added,
                    edges_added,
                });
            }
        }
    }

    // Re-canonicalize after expansion (Requirement 20.3)
    if !records.is_empty() {
        doc.canonicalize();
    }

    records
}

#[cfg(test)]
mod tests {
    use super::*;
    use glyph_ir::*;
    use glyph_registry::{MacroEntry, MacroExpansion};

    #[test]
    fn test_expand_empty_registry() {
        let mut doc = IrDocument::new("test", "digest");
        doc.nodes.push(IrNode::new("n_a", NodeKind::Module, "root"));
        let registry = MacroRegistry::default_empty();
        let records = expand(&mut doc, &registry);
        assert!(records.is_empty());
        assert_eq!(doc.nodes.len(), 1);
    }

    #[test]
    fn test_expand_no_match() {
        let mut doc = IrDocument::new("test", "digest");
        doc.nodes.push(IrNode::new("n_a", NodeKind::Module, "root"));

        let mut registry = MacroRegistry::default_empty();
        registry.entries.push(MacroEntry {
            pattern: "nonexistent".to_string(),
            precedence: 1,
            expansion: MacroExpansion { nodes: vec![], edges: vec![] },
            bindings: serde_json::json!({}),
        });

        let records = expand(&mut doc, &registry);
        assert!(records.is_empty());
        assert_eq!(doc.nodes.len(), 1);
    }

    #[test]
    fn test_expand_with_match() {
        let mut doc = IrDocument::new("test", "digest");
        doc.nodes.push(IrNode::new("n_a", NodeKind::Module, "root"));

        let exp_node = serde_json::to_value(&IrNode::new("n_exp", NodeKind::Block, "expanded")).unwrap();

        let mut registry = MacroRegistry::default_empty();
        registry.entries.push(MacroEntry {
            pattern: "root".to_string(),
            precedence: 1,
            expansion: MacroExpansion { nodes: vec![exp_node], edges: vec![] },
            bindings: serde_json::json!({}),
        });

        let records = expand(&mut doc, &registry);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].pattern, "root");
        assert_eq!(records[0].nodes_added, 1);
        assert_eq!(doc.nodes.len(), 2);
    }

    #[test]
    fn test_expand_precedence_order() {
        let mut doc = IrDocument::new("test", "digest");
        doc.nodes.push(IrNode::new("n_a", NodeKind::Module, "target"));

        let node_a = serde_json::to_value(&IrNode::new("n_x", NodeKind::Literal, "from_a")).unwrap();
        let node_b = serde_json::to_value(&IrNode::new("n_y", NodeKind::Literal, "from_b")).unwrap();

        let mut registry = MacroRegistry::default_empty();
        // Higher precedence number (lower priority)
        registry.entries.push(MacroEntry {
            pattern: "target".to_string(),
            precedence: 10,
            expansion: MacroExpansion { nodes: vec![node_b], edges: vec![] },
            bindings: serde_json::json!({}),
        });
        // Lower precedence number (higher priority, processed first)
        registry.entries.push(MacroEntry {
            pattern: "target".to_string(),
            precedence: 1,
            expansion: MacroExpansion { nodes: vec![node_a], edges: vec![] },
            bindings: serde_json::json!({}),
        });

        let records = expand(&mut doc, &registry);
        assert_eq!(records.len(), 2);
    }

    #[test]
    fn test_expand_recanonicalize() {
        let mut doc = IrDocument::new("test", "digest");
        doc.nodes.push(IrNode::new("n_z", NodeKind::Module, "target"));

        let node = serde_json::to_value(&IrNode::new("n_a", NodeKind::Block, "added")).unwrap();

        let mut registry = MacroRegistry::default_empty();
        registry.entries.push(MacroEntry {
            pattern: "target".to_string(),
            precedence: 1,
            expansion: MacroExpansion { nodes: vec![node], edges: vec![] },
            bindings: serde_json::json!({}),
        });

        expand(&mut doc, &registry);
        // After re-canonicalization, Block < Module
        assert_eq!(doc.nodes[0].kind, NodeKind::Block);
        assert_eq!(doc.nodes[1].kind, NodeKind::Module);
    }
}
