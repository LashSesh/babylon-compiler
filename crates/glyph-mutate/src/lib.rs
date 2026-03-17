//! Mutation engine for COL language evolution.
//! Implements Section 14 of the COL Master Specification v3.0.

use glyph_canon::digest_bytes;
use glyph_epoch::Epoch;
use glyph_gate::EvaluationMetrics;
use glyph_q16::Q16;
use glyph_registry::{MacroRegistry, SignRegistry, TripolarState};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// ---------------------------------------------------------------------------
// Mutation types
// ---------------------------------------------------------------------------

/// Classification of mutation types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationType {
    NewGlyphBinding,
    ModifiedMacroExpansion,
    ChangedGateThreshold,
    NewProfileWeighting,
}

/// A mutation candidate proposed by the engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationCandidate {
    pub candidate_id: String,
    pub mutation_type: MutationType,
    pub description: String,
    pub proposed_registry_delta: serde_json::Value,
}

/// Result of evaluating a candidate through the tripolar gate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateResult {
    pub candidate_id: String,
    pub mutation_type: MutationType,
    pub description: String,
    pub metrics: EvaluationMetrics,
    pub tripolar_state: TripolarState,
}

/// Complete mutation manifest for an evolution cycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationManifest {
    pub source_epoch: String,
    pub candidates: Vec<CandidateResult>,
    pub result_epoch: Option<String>,
    pub manifest_digest: String,
}

// ---------------------------------------------------------------------------
// Candidate generation
// ---------------------------------------------------------------------------

/// Generate mutation candidates deterministically from seed and epoch.
pub fn generate_candidates(
    epoch: &Epoch,
    sign_registry: &SignRegistry,
    _macro_registry: &MacroRegistry,
    seed: &str,
    max_candidates: usize,
) -> Vec<MutationCandidate> {
    let mut candidates = Vec::new();

    for i in 0..max_candidates {
        let mut hasher = Sha256::new();
        hasher.update(seed.as_bytes());
        hasher.update(epoch.epoch_digest.as_bytes());
        hasher.update(i.to_le_bytes());
        let hash = hasher.finalize();
        let candidate_id = hex::encode(&hash[..]);

        // Deterministically select mutation type based on hash byte
        let mutation_type = match hash[0] % 4 {
            0 => MutationType::NewGlyphBinding,
            1 => MutationType::ModifiedMacroExpansion,
            2 => MutationType::ChangedGateThreshold,
            _ => MutationType::NewProfileWeighting,
        };

        let description = match mutation_type {
            MutationType::NewGlyphBinding => {
                let sign_idx = (hash[1] as usize) % sign_registry.entries.len().max(1);
                format!("New glyph binding at sign index {}", sign_idx)
            }
            MutationType::ModifiedMacroExpansion => {
                format!("Modified macro expansion pattern {}", i)
            }
            MutationType::ChangedGateThreshold => {
                let delta = (hash[2] as i32) - 128;
                format!("Gate threshold adjustment by {}", delta)
            }
            MutationType::NewProfileWeighting => {
                format!("New profile weighting variant {}", i)
            }
        };

        candidates.push(MutationCandidate {
            candidate_id,
            mutation_type,
            description,
            proposed_registry_delta: serde_json::json!({
                "mutation_index": i,
                "seed_byte": hash[0],
            }),
        });
    }

    candidates
}

/// Compute evaluation metrics for a candidate.
/// In a full implementation this would run the candidate through test vectors.
/// For now, deterministic metrics derived from candidate hash.
pub fn evaluate_candidate(candidate: &MutationCandidate) -> EvaluationMetrics {
    let hash_bytes = digest_bytes(candidate.candidate_id.as_bytes());
    let bytes: Vec<u8> = (0..7)
        .map(|i| {
            u8::from_str_radix(&hash_bytes[i * 2..i * 2 + 2], 16).unwrap_or(0)
        })
        .collect();

    // Generate metrics deterministically from candidate hash
    let scale = |b: u8| -> Q16 {
        Q16::from_raw(((b as i32) * 65536) / 255)
    };

    EvaluationMetrics {
        coherence: scale(bytes[0]),
        entropy: scale(bytes[1]),
        drift: scale(bytes[2]),
        path_invariance_gap: scale(bytes[3]),
        compression: scale(bytes[4]),
        resonance_fitness: scale(bytes[5]),
        stability: scale(bytes[6]),
    }
}

/// Build a mutation manifest from evaluated candidates.
pub fn build_manifest(
    source_epoch: &str,
    results: Vec<CandidateResult>,
    result_epoch: Option<&str>,
) -> MutationManifest {
    let mut manifest = MutationManifest {
        source_epoch: source_epoch.to_string(),
        candidates: results,
        result_epoch: result_epoch.map(|s| s.to_string()),
        manifest_digest: String::new(),
    };

    // Compute manifest digest with digest=""
    let json = serde_json::to_string(&manifest).unwrap_or_default();
    manifest.manifest_digest = digest_bytes(json.as_bytes());

    manifest
}

// ---------------------------------------------------------------------------
// hex encoding helper (inline to avoid extra dependency)
// ---------------------------------------------------------------------------

mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use glyph_epoch::{Epoch, EpochPolicy, EpochRegistryDigests};

    fn test_epoch() -> Epoch {
        let z = "0".repeat(64);
        Epoch::construct(
            EpochRegistryDigests {
                sign: z.clone(),
                r#macro: z.clone(),
                profile: z.clone(),
                obligation: z.clone(),
                gate: z,
            },
            EpochPolicy::default(),
            None,
        )
    }

    #[test]
    fn test_generate_candidates_deterministic() {
        let epoch = test_epoch();
        let sign_reg = SignRegistry::default_epoch0();
        let macro_reg = MacroRegistry::default_empty();

        let c1 = generate_candidates(&epoch, &sign_reg, &macro_reg, "seed1", 5);
        let c2 = generate_candidates(&epoch, &sign_reg, &macro_reg, "seed1", 5);

        assert_eq!(c1.len(), 5);
        for (a, b) in c1.iter().zip(c2.iter()) {
            assert_eq!(a.candidate_id, b.candidate_id);
            assert_eq!(a.mutation_type, b.mutation_type);
        }
    }

    #[test]
    fn test_different_seeds_different_candidates() {
        let epoch = test_epoch();
        let sign_reg = SignRegistry::default_epoch0();
        let macro_reg = MacroRegistry::default_empty();

        let c1 = generate_candidates(&epoch, &sign_reg, &macro_reg, "seed1", 3);
        let c2 = generate_candidates(&epoch, &sign_reg, &macro_reg, "seed2", 3);

        assert_ne!(c1[0].candidate_id, c2[0].candidate_id);
    }

    #[test]
    fn test_evaluate_candidate_deterministic() {
        let epoch = test_epoch();
        let sign_reg = SignRegistry::default_epoch0();
        let macro_reg = MacroRegistry::default_empty();
        let candidates = generate_candidates(&epoch, &sign_reg, &macro_reg, "seed1", 1);

        let m1 = evaluate_candidate(&candidates[0]);
        let m2 = evaluate_candidate(&candidates[0]);
        assert_eq!(m1.coherence.raw(), m2.coherence.raw());
        assert_eq!(m1.stability.raw(), m2.stability.raw());
    }

    #[test]
    fn test_build_manifest() {
        let manifest = build_manifest("epoch-0", vec![], None);
        assert_eq!(manifest.source_epoch, "epoch-0");
        assert_eq!(manifest.manifest_digest.len(), 64);
    }
}
