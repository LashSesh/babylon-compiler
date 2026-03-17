//! Constraint lattice, obligation evaluation, Proof-of-Resonance, and tripolar gate logic.
//! Implements Section 17 of the Glyph Foundry Specification v2.1
//! and COL Master Specification v3.0 (Section 13).

use glyph_embed::H5Point;
use glyph_ir::IrDocument;
use glyph_q16::Q16;
use glyph_registry::{GateRegistry, ObligationRegistry, TripolarState};
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

// ---------------------------------------------------------------------------
// Tripolar Gate Logic (COL Section 13)
// ---------------------------------------------------------------------------

/// Evaluation metrics for tripolar gate decisions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationMetrics {
    pub coherence: Q16,
    pub entropy: Q16,
    pub drift: Q16,
    pub path_invariance_gap: Q16,
    pub compression: Q16,
    pub resonance_fitness: Q16,
    pub stability: Q16,
}

/// Result of evaluating a single gate in tripolar mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateObject {
    pub candidate_id: String,
    pub gate_id: String,
    pub metrics: EvaluationMetrics,
    pub tripolar_state: TripolarState,
    pub evidence_digest: String,
}

/// Evaluate metrics against a gate registry using tripolar logic.
///
/// For each gate entry, each threshold is evaluated:
/// - metric < reject_below → Reject (0)
/// - metric > accept_above → Accept (1)
/// - otherwise → Latent (LD)
///
/// Combined gate state: any Reject → overall Reject, all Accept → overall Accept, else Latent.
pub fn evaluate_tripolar(
    candidate_id: &str,
    metrics: &EvaluationMetrics,
    registry: &GateRegistry,
) -> Vec<GateObject> {
    let metrics_map = [
        ("coherence", metrics.coherence),
        ("entropy", metrics.entropy),
        ("drift", metrics.drift),
        ("path_invariance_gap", metrics.path_invariance_gap),
        ("compression", metrics.compression),
        ("resonance_fitness", metrics.resonance_fitness),
        ("stability", metrics.stability),
    ];
    let lookup = |name: &str| -> Q16 {
        metrics_map
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, v)| *v)
            .unwrap_or(Q16::from_raw(0))
    };

    let mut results = Vec::new();

    for entry in &registry.entries {
        let mut has_reject = false;
        let mut all_accept = true;

        for threshold in &entry.thresholds {
            let val = lookup(&threshold.metric);
            if val.raw() < threshold.reject_below.raw() {
                has_reject = true;
                all_accept = false;
            } else if val.raw() > threshold.accept_above.raw() {
                // Accept — no change to has_reject
            } else {
                // Latent
                all_accept = false;
            }
        }

        let state = if has_reject {
            TripolarState::Reject
        } else if all_accept {
            TripolarState::Accept
        } else {
            TripolarState::Latent
        };

        let evidence = serde_json::to_string(metrics).unwrap_or_default();
        let evidence_digest = glyph_canon::digest_bytes(evidence.as_bytes());

        results.push(GateObject {
            candidate_id: candidate_id.to_string(),
            gate_id: entry.gate_id.clone(),
            metrics: metrics.clone(),
            tripolar_state: state,
            evidence_digest,
        });
    }

    results
}

/// Compute semantic density: IR nodes / surface tokens.
pub fn semantic_density(surface_tokens: usize, ir_nodes: usize) -> Q16 {
    if surface_tokens == 0 {
        return Q16::from_raw(0);
    }
    // Q16 has 16 fractional bits, so multiply by 65536 first
    let ratio_raw = ((ir_nodes as i64) * 65536) / (surface_tokens as i64);
    Q16::from_raw(ratio_raw as i32)
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

    // -- Tripolar gate tests -----------------------------------------------

    fn test_metrics(coherence_raw: i32, entropy_raw: i32) -> EvaluationMetrics {
        EvaluationMetrics {
            coherence: Q16::from_raw(coherence_raw),
            entropy: Q16::from_raw(entropy_raw),
            drift: Q16::from_raw(0),
            path_invariance_gap: Q16::from_raw(0),
            compression: Q16::from_raw(0),
            resonance_fitness: Q16::from_raw(0),
            stability: Q16::from_raw(65536), // 1.0
        }
    }

    fn test_gate_registry() -> GateRegistry {
        use glyph_registry::{GateEntry, GateThreshold};
        let mut reg = GateRegistry::default_empty();
        reg.entries.push(GateEntry {
            gate_id: "gate-coherence".to_string(),
            version: "1.0.0".to_string(),
            input_metrics: vec!["coherence".to_string()],
            thresholds: vec![GateThreshold {
                metric: "coherence".to_string(),
                reject_below: Q16::from_raw(16384),   // 0.25
                accept_above: Q16::from_raw(49152),    // 0.75
            }],
            evidence_template: serde_json::json!({}),
        });
        reg
    }

    #[test]
    fn test_tripolar_accept() {
        let metrics = test_metrics(65536, 0); // coherence = 1.0
        let reg = test_gate_registry();
        let results = evaluate_tripolar("cand-1", &metrics, &reg);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].tripolar_state, TripolarState::Accept);
    }

    #[test]
    fn test_tripolar_reject() {
        let metrics = test_metrics(0, 0); // coherence = 0
        let reg = test_gate_registry();
        let results = evaluate_tripolar("cand-2", &metrics, &reg);
        assert_eq!(results[0].tripolar_state, TripolarState::Reject);
    }

    #[test]
    fn test_tripolar_latent() {
        let metrics = test_metrics(32768, 0); // coherence = 0.5 (between 0.25 and 0.75)
        let reg = test_gate_registry();
        let results = evaluate_tripolar("cand-3", &metrics, &reg);
        assert_eq!(results[0].tripolar_state, TripolarState::Latent);
    }

    #[test]
    fn test_tripolar_deterministic() {
        let metrics = test_metrics(32768, 0);
        let reg = test_gate_registry();
        let r1 = evaluate_tripolar("c", &metrics, &reg);
        let r2 = evaluate_tripolar("c", &metrics, &reg);
        assert_eq!(r1[0].tripolar_state, r2[0].tripolar_state);
        assert_eq!(r1[0].evidence_digest, r2[0].evidence_digest);
    }

    #[test]
    fn test_semantic_density() {
        // 10 surface tokens, 15 IR nodes → D = 1.5
        let d = semantic_density(10, 15);
        assert!(d.raw() > 65536); // > 1.0
        // 5 tokens, 3 nodes → D = 0.6
        let d2 = semantic_density(5, 3);
        assert!(d2.raw() < 65536); // < 1.0
    }
}
