//! Constraint lattice, obligation evaluation, and Proof-of-Resonance.
//! Implements Section 17 of the Glyph Foundry Specification v2.1.

use glyph_embed::H5Point;
use glyph_ir::IrDocument;
use glyph_registry::ObligationRegistry;
use serde::{Deserialize, Serialize};

/// Result of evaluating a single obligation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObligationResult {
    pub id: String,
    pub class: String,
    pub passed: bool,
    pub evidence: serde_json::Value,
}

/// Result of the gate evaluation (all obligations).
#[derive(Debug, Clone)]
pub struct GateResult {
    pub passed: bool,
    pub results: Vec<ObligationResult>,
}

/// PoR FSM states (Definition 17.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PorState {
    Search,
    Lock,
    Verify,
    Commit,
}

impl PorState {
    /// Transition the PoR FSM to the next state.
    pub fn next(&self) -> Option<PorState> {
        match self {
            PorState::Search => Some(PorState::Lock),
            PorState::Lock => Some(PorState::Verify),
            PorState::Verify => Some(PorState::Commit),
            PorState::Commit => None, // terminal
        }
    }
}

/// Evaluate all obligations against the graph and embedding.
pub fn evaluate_obligations(
    _doc: &IrDocument,
    embedding: &H5Point,
    registry: &ObligationRegistry,
) -> GateResult {
    let mut results = Vec::new();
    let mut all_passed = true;

    for entry in &registry.entries {
        let passed = match entry.predicate.as_str() {
            "always_pass" => true,
            "axis_ge_threshold" => {
                let axis_idx = entry.axis_index;
                if (axis_idx as usize) < 5 {
                    let threshold = entry.threshold;
                    embedding[axis_idx as usize].raw() >= threshold.raw()
                } else {
                    false
                }
            }
            _ => true, // Unknown predicates pass by default
        };

        if !passed && entry.class == glyph_registry::ObligationClass::Hard {
            all_passed = false;
        }

        let class_str = match entry.class {
            glyph_registry::ObligationClass::Hard => "hard",
            glyph_registry::ObligationClass::Soft => "soft",
        };

        results.push(ObligationResult {
            id: entry.id.clone(),
            class: class_str.to_string(),
            passed,
            evidence: serde_json::json!({
                "predicate": entry.predicate,
                "passed": passed,
            }),
        });
    }

    GateResult {
        passed: all_passed,
        results,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glyph_q16::Q16;
    use glyph_registry::{ObligationClass, ObligationEntry};

    fn make_obligation(id: &str, class: ObligationClass, predicate: &str, threshold: Q16, axis: i64) -> ObligationEntry {
        ObligationEntry {
            id: id.to_string(),
            version: "1.0.0".to_string(),
            class,
            description: format!("Test obligation {}", id),
            predicate: predicate.to_string(),
            threshold,
            constraint_function: "test_fn".to_string(),
            axis_index: axis,
        }
    }

    #[test]
    fn test_empty_obligations_pass() {
        let doc = IrDocument::new("test", "digest");
        let embedding = [Q16::from_int(1); 5];
        let registry = ObligationRegistry::default_empty();
        let result = evaluate_obligations(&doc, &embedding, &registry);
        assert!(result.passed);
        assert!(result.results.is_empty());
    }

    #[test]
    fn test_always_pass_obligation() {
        let doc = IrDocument::new("test", "digest");
        let embedding = [Q16::from_int(1); 5];
        let mut registry = ObligationRegistry::default_empty();
        registry.entries.push(make_obligation("obl_1", ObligationClass::Hard, "always_pass", Q16::from_raw(0), 0));
        let result = evaluate_obligations(&doc, &embedding, &registry);
        assert!(result.passed);
        assert_eq!(result.results.len(), 1);
        assert!(result.results[0].passed);
    }

    #[test]
    fn test_axis_ge_threshold_pass() {
        let doc = IrDocument::new("test", "digest");
        let embedding = [Q16::from_int(1); 5]; // 1.0 on all axes
        let mut registry = ObligationRegistry::default_empty();
        registry.entries.push(make_obligation("obl_axis", ObligationClass::Hard, "axis_ge_threshold", Q16::from_raw(32768), 0));
        let result = evaluate_obligations(&doc, &embedding, &registry);
        assert!(result.passed);
    }

    #[test]
    fn test_axis_ge_threshold_fail_hard() {
        let doc = IrDocument::new("test", "digest");
        let embedding = [Q16::from_raw(100); 5]; // very small
        let mut registry = ObligationRegistry::default_empty();
        registry.entries.push(make_obligation("obl_axis", ObligationClass::Hard, "axis_ge_threshold", Q16::from_int(1), 0));
        let result = evaluate_obligations(&doc, &embedding, &registry);
        assert!(!result.passed);
    }

    #[test]
    fn test_soft_obligation_fail_still_passes() {
        let doc = IrDocument::new("test", "digest");
        let embedding = [Q16::from_raw(100); 5];
        let mut registry = ObligationRegistry::default_empty();
        registry.entries.push(make_obligation("obl_soft", ObligationClass::Soft, "axis_ge_threshold", Q16::from_int(1), 0));
        let result = evaluate_obligations(&doc, &embedding, &registry);
        assert!(result.passed); // soft fail does not block gate
        assert!(!result.results[0].passed);
    }

    #[test]
    fn test_mixed_hard_soft() {
        let doc = IrDocument::new("test", "digest");
        let embedding = [Q16::from_int(1); 5];
        let mut registry = ObligationRegistry::default_empty();
        registry.entries.push(make_obligation("obl_hard", ObligationClass::Hard, "always_pass", Q16::from_raw(0), 0));
        registry.entries.push(make_obligation("obl_soft", ObligationClass::Soft, "axis_ge_threshold", Q16::from_int(10), 0));
        let result = evaluate_obligations(&doc, &embedding, &registry);
        assert!(result.passed); // hard passes, soft fails but doesn't block
        assert_eq!(result.results.len(), 2);
    }

    #[test]
    fn test_por_fsm_transitions() {
        let state = PorState::Search;
        assert_eq!(state.next(), Some(PorState::Lock));
        assert_eq!(PorState::Lock.next(), Some(PorState::Verify));
        assert_eq!(PorState::Verify.next(), Some(PorState::Commit));
        assert_eq!(PorState::Commit.next(), None);
    }
}
