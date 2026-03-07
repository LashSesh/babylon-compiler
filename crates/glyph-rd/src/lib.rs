//! Run Descriptor (RD) parsing, validation, and digest computation.
//! Implements Section 9 of the Glyph Foundry Specification v2.1.

use glyph_q16::Q16;
use serde::{Deserialize, Serialize};

/// The Run Descriptor — configuration object binding policies, versions, and seeds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunDescriptor {
    pub schema_version: String,
    pub run_id: String,
    pub unicode_policy: UnicodePolicy,
    pub canon_policy: CanonPolicy,
    pub seed_policy: SeedPolicy,
    pub embedding_policy: EmbeddingPolicy,
    pub convergence_policy: ConvergencePolicy,
    pub registries: RegistryRefs,
    pub toolchain: Toolchain,
    #[serde(default = "default_digest_algorithm")]
    pub digest_algorithm: String,
}

fn default_digest_algorithm() -> String {
    "sha256".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnicodePolicy {
    pub normalization_form: String,
    #[serde(default)]
    pub forbidden_ranges: Vec<ForbiddenRange>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confusable_policy: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confusable_map: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForbiddenRange {
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonPolicy {
    pub id_scheme: String,
    pub node_order: String,
    pub edge_order: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeedPolicy {
    pub mode: String,
    pub seed: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingPolicy {
    pub sigma: [Q16; 5],
    pub spectral_n: usize,
    pub axis_weights: Vec<Vec<Q16>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvergencePolicy {
    pub s_min: usize,
    pub w: usize,
    pub rho_max: Q16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryRefs {
    pub operator: String,
    pub r#macro: String,
    pub obligation: String,
    pub observable: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Toolchain {
    pub name: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
}

impl RunDescriptor {
    /// Create a default Run Descriptor (for `glyph init-rd`).
    pub fn default_rd() -> Self {
        let placeholder_hash = "0".repeat(64);
        let mut rd = RunDescriptor {
            schema_version: "2.1.0".to_string(),
            run_id: String::new(),
            unicode_policy: UnicodePolicy {
                normalization_form: "NFC".to_string(),
                forbidden_ranges: vec![],
                confusable_policy: None,
                confusable_map: None,
            },
            canon_policy: CanonPolicy {
                id_scheme: "content_hash".to_string(),
                node_order: "lexicographic_utf8".to_string(),
                edge_order: "lexicographic_utf8".to_string(),
            },
            seed_policy: SeedPolicy {
                mode: "fixed".to_string(),
                seed: "0".to_string(),
            },
            embedding_policy: EmbeddingPolicy {
                sigma: [Q16::from_raw(65536); 5],
                spectral_n: 13,
                axis_weights: vec![vec![], vec![], vec![], vec![], vec![]],
            },
            convergence_policy: ConvergencePolicy {
                s_min: 5,
                w: 3,
                rho_max: Q16::from_raw(64880),
            },
            registries: RegistryRefs {
                operator: placeholder_hash.clone(),
                r#macro: placeholder_hash.clone(),
                obligation: placeholder_hash.clone(),
                observable: placeholder_hash,
            },
            toolchain: Toolchain {
                name: "glyph".to_string(),
                version: "0.2.0".to_string(),
                platform: None,
            },
            digest_algorithm: "sha256".to_string(),
        };
        rd.run_id = rd.compute_run_id();
        rd
    }

    /// Compute the run_id per Requirement 9.1:
    /// Serialize RD with run_id="" as canonical JSON, SHA-256 hash.
    pub fn compute_run_id(&self) -> String {
        let mut rd_for_hash = self.clone();
        rd_for_hash.run_id = String::new();
        let value =
            serde_json::to_value(&rd_for_hash).expect("RD serialization must succeed");
        glyph_canon::digest_object(&value)
    }

    /// Load and validate a Run Descriptor from a JSON file.
    pub fn load(path: &std::path::Path) -> Result<Self, RdError> {
        let data = std::fs::read_to_string(path)
            .map_err(|e| RdError::Io(e.to_string()))?;
        let rd: RunDescriptor =
            serde_json::from_str(&data).map_err(|e| RdError::Parse(e.to_string()))?;
        Ok(rd)
    }

    /// Serialize the RD to pretty JSON (for file output).
    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).expect("RD serialization must succeed")
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RdError {
    #[error("I/O error: {0}")]
    Io(String),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Validation error: {0}")]
    Validation(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_rd() {
        let rd = RunDescriptor::default_rd();
        assert_eq!(rd.schema_version, "2.1.0");
        assert_eq!(rd.unicode_policy.normalization_form, "NFC");
        assert!(!rd.run_id.is_empty());
        assert_eq!(rd.run_id.len(), 64);
    }

    #[test]
    fn test_run_id_deterministic() {
        let rd1 = RunDescriptor::default_rd();
        let rd2 = RunDescriptor::default_rd();
        assert_eq!(rd1.run_id, rd2.run_id);
    }

    #[test]
    fn test_rd_serialization_roundtrip() {
        let rd = RunDescriptor::default_rd();
        let json = serde_json::to_string(&rd).unwrap();
        let rd2: RunDescriptor = serde_json::from_str(&json).unwrap();
        assert_eq!(rd.schema_version, rd2.schema_version);
        assert_eq!(rd.run_id, rd2.run_id);
    }
}
