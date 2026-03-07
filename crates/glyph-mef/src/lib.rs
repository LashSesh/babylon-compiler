//! Morphogenetic Evidence Framework (MEF) — append-only hash chain.
//! Implements Section 16 of the Glyph Foundry Specification v2.1.

use glyph_canon::{canonical_json, digest_bytes};
use serde::{Deserialize, Serialize};

/// Operator evidence record (Definition 16.2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorEvidence {
    pub step: u64,
    pub role: String,
    pub operator: String,
    pub before_digest: String,
    pub after_digest: String,
    pub evidence_digest: String,
}

/// A single MEF block (Section 16.1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MefBlock {
    pub block_index: u64,
    pub block_hash: String,
    pub prev_hash: String,
    pub transition_type: String,
    pub tick: u64,
    pub state_digest: String,
    pub operator_evidence: OperatorEvidence,
}

/// The MEF chain — an append-only hash chain (Definition 16.1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MefChain {
    pub blocks: Vec<MefBlock>,
}

/// Error from MEF chain verification.
#[derive(Debug, Clone)]
pub struct MefVerifyError {
    pub block_index: u64,
    pub expected: String,
    pub actual: String,
}

impl std::fmt::Display for MefVerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "MEF tamper at block {}: expected {}, got {}",
            self.block_index, self.expected, self.actual
        )
    }
}

impl std::error::Error for MefVerifyError {}

impl MefChain {
    /// Create a new empty MEF chain.
    pub fn new() -> Self {
        MefChain { blocks: Vec::new() }
    }

    /// Append a new block to the chain.
    pub fn append(
        &mut self,
        transition_type: String,
        tick: u64,
        state_digest: String,
        operator_evidence: OperatorEvidence,
    ) -> &MefBlock {
        let genesis_hash = "0".repeat(64);
        let prev_hash = self
            .blocks
            .last()
            .map(|b| b.block_hash.clone())
            .unwrap_or(genesis_hash);

        let block_index = self.blocks.len() as u64;

        // block_hash = SHA256(prev_hash || JCS(state))
        let state_obj = serde_json::json!({
            "block_index": block_index,
            "transition_type": &transition_type,
            "tick": tick,
            "state_digest": &state_digest,
        });
        let state_bytes = canonical_json(&state_obj);
        let mut hash_input = prev_hash.as_bytes().to_vec();
        hash_input.extend_from_slice(&state_bytes);
        let block_hash = digest_bytes(&hash_input);

        let block = MefBlock {
            block_index,
            block_hash,
            prev_hash,
            transition_type,
            tick,
            state_digest,
            operator_evidence,
        };

        self.blocks.push(block);
        self.blocks.last().unwrap()
    }

    /// Verify the chain integrity (Theorem 16.1).
    pub fn verify(&self) -> Result<(), MefVerifyError> {
        let genesis_hash = "0".repeat(64);
        let mut expected_prev = genesis_hash;

        for block in &self.blocks {
            if block.prev_hash != expected_prev {
                return Err(MefVerifyError {
                    block_index: block.block_index,
                    expected: expected_prev,
                    actual: block.prev_hash.clone(),
                });
            }

            // Recompute block hash
            let state_obj = serde_json::json!({
                "block_index": block.block_index,
                "transition_type": &block.transition_type,
                "tick": block.tick,
                "state_digest": &block.state_digest,
            });
            let state_bytes = canonical_json(&state_obj);
            let mut hash_input = block.prev_hash.as_bytes().to_vec();
            hash_input.extend_from_slice(&state_bytes);
            let expected_hash = digest_bytes(&hash_input);

            if block.block_hash != expected_hash {
                return Err(MefVerifyError {
                    block_index: block.block_index,
                    expected: expected_hash,
                    actual: block.block_hash.clone(),
                });
            }

            expected_prev = block.block_hash.clone();
        }

        Ok(())
    }

    pub fn len(&self) -> usize {
        self.blocks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }
}

impl Default for MefChain {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_evidence() -> OperatorEvidence {
        OperatorEvidence {
            step: 0,
            role: "test".to_string(),
            operator: "test_op".to_string(),
            before_digest: "a".repeat(64),
            after_digest: "b".repeat(64),
            evidence_digest: "c".repeat(64),
        }
    }

    #[test]
    fn test_chain_construction() {
        let mut chain = MefChain::new();
        chain.append("ingest".to_string(), 0, "state0".to_string(), test_evidence());
        chain.append("canon".to_string(), 1, "state1".to_string(), test_evidence());
        assert_eq!(chain.len(), 2);
        assert_eq!(chain.blocks[0].prev_hash, "0".repeat(64));
        assert_eq!(chain.blocks[1].prev_hash, chain.blocks[0].block_hash);
    }

    #[test]
    fn test_chain_verify_valid() {
        let mut chain = MefChain::new();
        chain.append("ingest".to_string(), 0, "s0".to_string(), test_evidence());
        chain.append("canon".to_string(), 1, "s1".to_string(), test_evidence());
        assert!(chain.verify().is_ok());
    }

    #[test]
    fn test_chain_verify_tamper() {
        let mut chain = MefChain::new();
        chain.append("ingest".to_string(), 0, "s0".to_string(), test_evidence());
        chain.append("canon".to_string(), 1, "s1".to_string(), test_evidence());
        // Tamper with block 0
        chain.blocks[0].state_digest = "tampered".to_string();
        assert!(chain.verify().is_err());
    }

    #[test]
    fn test_chain_deterministic() {
        let mut c1 = MefChain::new();
        let mut c2 = MefChain::new();
        c1.append("ingest".to_string(), 0, "s0".to_string(), test_evidence());
        c2.append("ingest".to_string(), 0, "s0".to_string(), test_evidence());
        assert_eq!(c1.blocks[0].block_hash, c2.blocks[0].block_hash);
    }
}
