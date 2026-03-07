//! IR schema types, serialization, and validation.
//! Implements the Canonical IR wire format (Section 12).

use glyph_q16::Q16;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// IR schema version.
pub const IR_SCHEMA_VERSION: &str = "1.0.0";

/// Node kinds (Section 12.2).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum NodeKind {
    Module,
    Function,
    Block,
    If,
    Return,
    Call,
    Assignment,
    Literal,
    BinaryOp,
    Identifier,
}

impl NodeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeKind::Module => "Module",
            NodeKind::Function => "Function",
            NodeKind::Block => "Block",
            NodeKind::If => "If",
            NodeKind::Return => "Return",
            NodeKind::Call => "Call",
            NodeKind::Assignment => "Assignment",
            NodeKind::Literal => "Literal",
            NodeKind::BinaryOp => "BinaryOp",
            NodeKind::Identifier => "Identifier",
        }
    }
}

impl std::fmt::Display for NodeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Edge kinds (Section 12.3).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    Contains,
    Next,
    Condition,
    ThenBranch,
    ElseBranch,
    Argument,
    Target,
    Value,
    Left,
    Right,
    CalleeRef,
}

impl EdgeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            EdgeKind::Contains => "contains",
            EdgeKind::Next => "next",
            EdgeKind::Condition => "condition",
            EdgeKind::ThenBranch => "then_branch",
            EdgeKind::ElseBranch => "else_branch",
            EdgeKind::Argument => "argument",
            EdgeKind::Target => "target",
            EdgeKind::Value => "value",
            EdgeKind::Left => "left",
            EdgeKind::Right => "right",
            EdgeKind::CalleeRef => "callee_ref",
        }
    }
}

impl std::fmt::Display for EdgeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A literal value in an IR node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LiteralValue {
    String(String),
    Int(i64),
    Bool(bool),
}

/// An IR node (Section 12.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IrNode {
    pub id: String,
    pub kind: NodeKind,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<LiteralValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub literal_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub op: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callee: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<BTreeMap<String, serde_json::Value>>,
}

impl IrNode {
    /// Create a new node with required fields only.
    pub fn new(id: impl Into<String>, kind: NodeKind, name: impl Into<String>) -> Self {
        IrNode {
            id: id.into(),
            kind,
            name: name.into(),
            params: None,
            value: None,
            literal_type: None,
            op: None,
            callee: None,
            properties: None,
        }
    }

    /// Sort key for canonical node ordering (Section 13.1):
    /// (kind, name, id) — lexicographic on UTF-8.
    pub fn sort_key(&self) -> (&str, &str, &str) {
        (self.kind.as_str(), &self.name, &self.id)
    }
}

/// An IR edge (Section 12.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IrEdge {
    pub src: String,
    pub dst: String,
    pub kind: EdgeKind,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub ordinal: i64,
}

fn is_zero(v: &i64) -> bool {
    *v == 0
}

impl IrEdge {
    pub fn new(src: impl Into<String>, dst: impl Into<String>, kind: EdgeKind) -> Self {
        IrEdge {
            src: src.into(),
            dst: dst.into(),
            kind,
            ordinal: 0,
        }
    }

    pub fn with_ordinal(mut self, ordinal: i64) -> Self {
        self.ordinal = ordinal;
        self
    }

    /// Sort key for canonical edge ordering (Section 13.2):
    /// (kind, src, dst, ordinal).
    pub fn sort_key(&self) -> (&str, &str, &str, i64) {
        (self.kind.as_str(), &self.src, &self.dst, self.ordinal)
    }
}

/// Embedding information attached to the IR document (v2.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddingInfo {
    pub axes: [Q16; 5],
    pub fingerprint: String,
    pub genome_id: String,
}

impl Default for EmbeddingInfo {
    fn default() -> Self {
        EmbeddingInfo {
            axes: [Q16::from_raw(0); 5],
            fingerprint: "0".repeat(64),
            genome_id: "0".repeat(64),
        }
    }
}

/// The IR document (Section 12.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IrDocument {
    pub schema_version: String,
    pub nodes: Vec<IrNode>,
    pub edges: Vec<IrEdge>,
    pub source_language: String,
    pub source_digest: String,
    pub embedding: EmbeddingInfo,
}

impl IrDocument {
    pub fn new(source_language: impl Into<String>, source_digest: impl Into<String>) -> Self {
        IrDocument {
            schema_version: IR_SCHEMA_VERSION.to_string(),
            nodes: Vec::new(),
            edges: Vec::new(),
            source_language: source_language.into(),
            source_digest: source_digest.into(),
            embedding: EmbeddingInfo::default(),
        }
    }

    /// Sort nodes and edges into canonical order.
    pub fn canonicalize(&mut self) {
        self.nodes.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
        self.edges.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
    }

    /// Compute the digest of the canonical IR document.
    pub fn digest(&self) -> String {
        let value = serde_json::to_value(self).expect("IR serialization must succeed");
        glyph_canon::digest_object(&value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_creation() {
        let node = IrNode::new("n_abc", NodeKind::Module, "main");
        assert_eq!(node.kind, NodeKind::Module);
        assert_eq!(node.name, "main");
    }

    #[test]
    fn test_edge_creation() {
        let edge = IrEdge::new("n_a", "n_b", EdgeKind::Contains);
        assert_eq!(edge.kind, EdgeKind::Contains);
        assert_eq!(edge.ordinal, 0);
    }

    #[test]
    fn test_document_canonicalize() {
        let mut doc = IrDocument::new("sanskroot", "abc123");
        doc.nodes.push(IrNode::new("n_z", NodeKind::Function, "z"));
        doc.nodes.push(IrNode::new("n_a", NodeKind::Block, "a"));
        doc.canonicalize();
        // Block < Function lexicographically
        assert_eq!(doc.nodes[0].kind, NodeKind::Block);
        assert_eq!(doc.nodes[1].kind, NodeKind::Function);
    }

    #[test]
    fn test_document_digest_deterministic() {
        let mut doc = IrDocument::new("sanskroot", "abc123");
        doc.nodes.push(IrNode::new("n_a", NodeKind::Module, "root"));
        doc.canonicalize();
        let d1 = doc.digest();
        let d2 = doc.digest();
        assert_eq!(d1, d2);
        assert_eq!(d1.len(), 64);
    }

    #[test]
    fn test_node_serialization() {
        let node = IrNode::new("n_test", NodeKind::Literal, "42");
        let json = serde_json::to_string(&node).unwrap();
        assert!(json.contains("\"kind\":\"Literal\""));
    }
}
