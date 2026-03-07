//! Glyph registry types from the Glyph Foundry spec v2.1 (Sections 17 & 19).
//!
//! Defines four registry types:
//! - [`OperatorRegistry`] (Section 19.1)
//! - [`MacroRegistry`] (Section 19.2)
//! - [`ObservableRegistry`] (Section 19.3)
//! - [`ObligationRegistry`] (Section 17.1)

use glyph_ir::NodeKind;
use glyph_q16::Q16;
use serde::{Deserialize, Serialize};

/// Schema version for all registry types.
pub const REGISTRY_SCHEMA_VERSION: &str = "1.0.0";

// ---------------------------------------------------------------------------
// ObligationClass enum (Section 17.1)
// ---------------------------------------------------------------------------

/// Classification of an obligation as hard (must hold) or soft (best-effort).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ObligationClass {
    Hard,
    Soft,
}

// ---------------------------------------------------------------------------
// OperatorRegistry (Section 19.1)
// ---------------------------------------------------------------------------

/// A single operator entry in the operator registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatorEntry {
    pub id: String,
    pub version: String,
    pub input_types: Vec<NodeKind>,
    pub output_type: NodeKind,
    pub constraints: Vec<String>,
    pub evidence_schema: serde_json::Value,
}

/// The operator registry (Section 19.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatorRegistry {
    pub schema_version: String,
    #[serde(rename = "type")]
    pub registry_type: String,
    pub entries: Vec<OperatorEntry>,
}

impl OperatorRegistry {
    /// Create an empty, valid operator registry.
    pub fn default_empty() -> Self {
        OperatorRegistry {
            schema_version: REGISTRY_SCHEMA_VERSION.to_string(),
            registry_type: "operator_registry".to_string(),
            entries: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// MacroRegistry (Section 19.2)
// ---------------------------------------------------------------------------

/// The expansion graph embedded in a macro entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MacroExpansion {
    pub nodes: Vec<serde_json::Value>,
    pub edges: Vec<serde_json::Value>,
}

/// A single macro entry in the macro registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MacroEntry {
    pub pattern: String,
    pub precedence: i64,
    pub expansion: MacroExpansion,
    pub bindings: serde_json::Value,
}

/// The macro registry (Section 19.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MacroRegistry {
    pub schema_version: String,
    #[serde(rename = "type")]
    pub registry_type: String,
    pub entries: Vec<MacroEntry>,
}

impl MacroRegistry {
    /// Create an empty, valid macro registry.
    pub fn default_empty() -> Self {
        MacroRegistry {
            schema_version: REGISTRY_SCHEMA_VERSION.to_string(),
            registry_type: "macro_registry".to_string(),
            entries: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// ObservableRegistry (Section 19.3)
// ---------------------------------------------------------------------------

/// A single observable entry in the observable registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservableEntry {
    pub id: String,
    pub version: String,
    pub arity: i64,
    pub deterministic: bool,
    pub computation_ref: String,
}

/// The observable registry (Section 19.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservableRegistry {
    pub schema_version: String,
    #[serde(rename = "type")]
    pub registry_type: String,
    pub entries: Vec<ObservableEntry>,
}

impl ObservableRegistry {
    /// Create an empty, valid observable registry.
    pub fn default_empty() -> Self {
        ObservableRegistry {
            schema_version: REGISTRY_SCHEMA_VERSION.to_string(),
            registry_type: "observable_registry".to_string(),
            entries: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// ObligationRegistry (Section 17.1)
// ---------------------------------------------------------------------------

/// A single obligation entry in the obligation registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObligationEntry {
    pub id: String,
    pub version: String,
    pub class: ObligationClass,
    pub description: String,
    pub predicate: String,
    pub threshold: Q16,
    pub constraint_function: String,
    pub axis_index: i64,
}

/// The obligation registry (Section 17.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObligationRegistry {
    pub schema_version: String,
    #[serde(rename = "type")]
    pub registry_type: String,
    pub entries: Vec<ObligationEntry>,
}

impl ObligationRegistry {
    /// Create an empty, valid obligation registry.
    pub fn default_empty() -> Self {
        ObligationRegistry {
            schema_version: REGISTRY_SCHEMA_VERSION.to_string(),
            registry_type: "obligation_registry".to_string(),
            entries: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Digest computation
// ---------------------------------------------------------------------------

/// Compute the canonical digest of any registry using `glyph_canon::digest_object`.
///
/// The registry is first serialized to a `serde_json::Value`, then passed
/// through JCS canonicalization and SHA-256 hashing.
pub fn compute_digest<T: Serialize>(registry: &T) -> String {
    let value = serde_json::to_value(registry).expect("registry serialization must succeed");
    glyph_canon::digest_object(&value)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- OperatorRegistry ---------------------------------------------------

    #[test]
    fn test_operator_registry_default_empty() {
        let reg = OperatorRegistry::default_empty();
        assert_eq!(reg.schema_version, "1.0.0");
        assert_eq!(reg.registry_type, "operator_registry");
        assert!(reg.entries.is_empty());
    }

    #[test]
    fn test_operator_registry_roundtrip() {
        let mut reg = OperatorRegistry::default_empty();
        reg.entries.push(OperatorEntry {
            id: "op_add".to_string(),
            version: "1.0.0".to_string(),
            input_types: vec![NodeKind::Literal, NodeKind::Literal],
            output_type: NodeKind::BinaryOp,
            constraints: vec!["numeric_only".to_string()],
            evidence_schema: serde_json::json!({"type": "object"}),
        });

        let json = serde_json::to_string(&reg).unwrap();
        let decoded: OperatorRegistry = serde_json::from_str(&json).unwrap();
        assert_eq!(reg, decoded);
    }

    #[test]
    fn test_operator_registry_type_field_serializes_as_type() {
        let reg = OperatorRegistry::default_empty();
        let value = serde_json::to_value(&reg).unwrap();
        assert_eq!(value["type"], "operator_registry");
        // Ensure there is no "registry_type" key in the JSON output.
        assert!(value.get("registry_type").is_none());
    }

    // -- MacroRegistry ------------------------------------------------------

    #[test]
    fn test_macro_registry_default_empty() {
        let reg = MacroRegistry::default_empty();
        assert_eq!(reg.schema_version, "1.0.0");
        assert_eq!(reg.registry_type, "macro_registry");
        assert!(reg.entries.is_empty());
    }

    #[test]
    fn test_macro_registry_roundtrip() {
        let mut reg = MacroRegistry::default_empty();
        reg.entries.push(MacroEntry {
            pattern: "if_let".to_string(),
            precedence: 10,
            expansion: MacroExpansion {
                nodes: vec![serde_json::json!({"id": "n1"})],
                edges: vec![serde_json::json!({"src": "n1", "dst": "n2"})],
            },
            bindings: serde_json::json!({"x": "Identifier"}),
        });

        let json = serde_json::to_string(&reg).unwrap();
        let decoded: MacroRegistry = serde_json::from_str(&json).unwrap();
        assert_eq!(reg, decoded);
    }

    // -- ObservableRegistry -------------------------------------------------

    #[test]
    fn test_observable_registry_default_empty() {
        let reg = ObservableRegistry::default_empty();
        assert_eq!(reg.schema_version, "1.0.0");
        assert_eq!(reg.registry_type, "observable_registry");
        assert!(reg.entries.is_empty());
    }

    #[test]
    fn test_observable_registry_roundtrip() {
        let mut reg = ObservableRegistry::default_empty();
        reg.entries.push(ObservableEntry {
            id: "obs_cyclomatic".to_string(),
            version: "1.2.0".to_string(),
            arity: 1,
            deterministic: true,
            computation_ref: "compute_cyclomatic".to_string(),
        });

        let json = serde_json::to_string(&reg).unwrap();
        let decoded: ObservableRegistry = serde_json::from_str(&json).unwrap();
        assert_eq!(reg, decoded);
    }

    // -- ObligationRegistry -------------------------------------------------

    #[test]
    fn test_obligation_registry_default_empty() {
        let reg = ObligationRegistry::default_empty();
        assert_eq!(reg.schema_version, "1.0.0");
        assert_eq!(reg.registry_type, "obligation_registry");
        assert!(reg.entries.is_empty());
    }

    #[test]
    fn test_obligation_registry_roundtrip() {
        let mut reg = ObligationRegistry::default_empty();
        reg.entries.push(ObligationEntry {
            id: "obl_coverage".to_string(),
            version: "1.0.0".to_string(),
            class: ObligationClass::Hard,
            description: "Minimum test coverage".to_string(),
            predicate: "coverage >= threshold".to_string(),
            threshold: Q16::from_raw(52428), // ~0.8 in Q16
            constraint_function: "check_coverage".to_string(),
            axis_index: 0,
        });

        let json = serde_json::to_string(&reg).unwrap();
        let decoded: ObligationRegistry = serde_json::from_str(&json).unwrap();
        assert_eq!(reg, decoded);
    }

    #[test]
    fn test_obligation_class_serialization() {
        let hard = serde_json::to_string(&ObligationClass::Hard).unwrap();
        let soft = serde_json::to_string(&ObligationClass::Soft).unwrap();
        assert_eq!(hard, "\"hard\"");
        assert_eq!(soft, "\"soft\"");

        let decoded: ObligationClass = serde_json::from_str("\"hard\"").unwrap();
        assert_eq!(decoded, ObligationClass::Hard);
        let decoded: ObligationClass = serde_json::from_str("\"soft\"").unwrap();
        assert_eq!(decoded, ObligationClass::Soft);
    }

    // -- compute_digest -----------------------------------------------------

    #[test]
    fn test_compute_digest_deterministic() {
        let reg = OperatorRegistry::default_empty();
        let d1 = compute_digest(&reg);
        let d2 = compute_digest(&reg);
        assert_eq!(d1, d2);
        assert_eq!(d1.len(), 64); // SHA-256 hex string
    }

    #[test]
    fn test_compute_digest_differs_for_different_registries() {
        let op_reg = OperatorRegistry::default_empty();
        let macro_reg = MacroRegistry::default_empty();
        assert_ne!(compute_digest(&op_reg), compute_digest(&macro_reg));
    }

    #[test]
    fn test_compute_digest_changes_with_entries() {
        let empty = ObservableRegistry::default_empty();
        let mut with_entry = ObservableRegistry::default_empty();
        with_entry.entries.push(ObservableEntry {
            id: "obs_1".to_string(),
            version: "1.0.0".to_string(),
            arity: 2,
            deterministic: false,
            computation_ref: "compute_obs_1".to_string(),
        });
        assert_ne!(compute_digest(&empty), compute_digest(&with_entry));
    }

    #[test]
    fn test_all_registries_digest() {
        // Ensure compute_digest works for every registry type.
        let d1 = compute_digest(&OperatorRegistry::default_empty());
        let d2 = compute_digest(&MacroRegistry::default_empty());
        let d3 = compute_digest(&ObservableRegistry::default_empty());
        let d4 = compute_digest(&ObligationRegistry::default_empty());
        // All should be 64-char hex strings.
        for d in [&d1, &d2, &d3, &d4] {
            assert_eq!(d.len(), 64);
            assert!(d.chars().all(|c| c.is_ascii_hexdigit()));
        }
        // All four should be distinct.
        let mut set = std::collections::HashSet::new();
        set.insert(d1);
        set.insert(d2);
        set.insert(d3);
        set.insert(d4);
        assert_eq!(set.len(), 4);
    }
}
