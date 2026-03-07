//! Crystal construction, HyperCrystal composition.
//! Implements Section 15 of the Glyph Foundry Specification v2.1.

use glyph_canon::{canonical_json, digest_bytes};
use glyph_embed::H5Point;
use glyph_q16::Q16;
use glyph_tic::Tic;
use serde::{Deserialize, Serialize};

/// A Crystal — the immutable, certified knowledge unit (Section 15.1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Crystal {
    pub schema_version: String,
    pub crystal_id: String,
    pub core: CrystalCore,
    pub embedding: CrystalEmbedding,
    pub support_count: u64,
    pub tic_final: TicFinal,
    pub lineage: Lineage,
    pub config_hash: String,
    pub evidence_refs: Vec<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrystalCore {
    pub graph_id: String,
    pub signature_id: String,
    pub genome_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrystalEmbedding {
    pub axes: [Q16; 5],
    pub fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TicFinal {
    pub delta_last: Q16,
    pub kappa_last: Q16,
    pub condensing_window: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lineage {
    pub parent_ids: Vec<String>,
    pub merge_reason: String,
    pub commit_tick: u64,
}

impl Crystal {
    /// Construct a Crystal from pipeline results.
    /// crystal_id = digest(core)
    pub fn construct(
        graph_id: String,
        signature_id: String,
        genome_id: String,
        embedding: H5Point,
        tic: &Tic,
        config_hash: String,
        evidence_refs: Vec<String>,
        tick: u64,
    ) -> Self {
        let core = CrystalCore {
            graph_id,
            signature_id: signature_id.clone(),
            genome_id,
        };

        // crystal_id = digest(core)
        let core_value = serde_json::to_value(&core).unwrap();
        let core_bytes = canonical_json(&core_value);
        let crystal_id = digest_bytes(&core_bytes);

        let fp = glyph_embed::fingerprint(&embedding);

        let now = chrono::Utc::now().to_rfc3339();

        Crystal {
            schema_version: "2.1.0".to_string(),
            crystal_id,
            core,
            embedding: CrystalEmbedding {
                axes: embedding,
                fingerprint: fp,
            },
            support_count: tic.entries.len() as u64,
            tic_final: TicFinal {
                delta_last: tic.last_delta(),
                kappa_last: tic.last_kappa(),
                condensing_window: tic
                    .entries
                    .len()
                    .min(3) as u64,
            },
            lineage: Lineage {
                parent_ids: vec![],
                merge_reason: "initial_crystallization".to_string(),
                commit_tick: tick,
            },
            config_hash,
            evidence_refs,
            created_at: now,
        }
    }

    /// Compute the HyperCrystal embedding as weighted mean (Section 26).
    pub fn hyper_embedding(crystals: &[&Crystal]) -> H5Point {
        if crystals.is_empty() {
            return [Q16::from_raw(0); 5];
        }
        let count = crystals.len() as i64;
        let mut sum = [0i64; 5];
        for c in crystals {
            for i in 0..5 {
                sum[i] += c.embedding.axes[i].raw() as i64;
            }
        }
        let mut result = [Q16::from_raw(0); 5];
        for i in 0..5 {
            result[i] = Q16::from_raw((sum[i] / count) as i32);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crystal_construct() {
        let mut tic = Tic::new();
        for i in 0..5 {
            tic.append(i, Q16::from_raw((100 - i * 10) as i32), 5);
        }
        let embedding = [Q16::from_int(1); 5];
        let crystal = Crystal::construct(
            "graph_abc".to_string(),
            "sig_abc".to_string(),
            "genome_abc".to_string(),
            embedding,
            &tic,
            "config_hash".to_string(),
            vec![],
            5,
        );
        assert_eq!(crystal.schema_version, "2.1.0");
        assert_eq!(crystal.crystal_id.len(), 64);
        assert_eq!(crystal.support_count, 5);
    }

    #[test]
    fn test_crystal_id_deterministic() {
        let tic = Tic::new();
        let embedding = [Q16::from_int(1); 5];
        let c1 = Crystal::construct(
            "g".to_string(), "s".to_string(), "gn".to_string(),
            embedding, &tic, "ch".to_string(), vec![], 0,
        );
        let c2 = Crystal::construct(
            "g".to_string(), "s".to_string(), "gn".to_string(),
            embedding, &tic, "ch".to_string(), vec![], 0,
        );
        assert_eq!(c1.crystal_id, c2.crystal_id);
    }
}
