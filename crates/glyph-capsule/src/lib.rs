//! Evidence capsules for COL provenance tracking.
//! Implements Section 18 of the COL Master Specification v3.0.

use glyph_canon::digest_bytes;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Capsule types and policy
// ---------------------------------------------------------------------------

/// Classification of capsule types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapsuleType {
    ContextAnchor,
    Commitment,
    Boundary,
    Attachment,
}

/// Policy governing capsule creation and validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapsulePolicy {
    pub max_size: usize,
    pub admissible_types: Vec<CapsuleType>,
    pub provenance_required: bool,
    pub hash_binding: String,
}

impl Default for CapsulePolicy {
    fn default() -> Self {
        CapsulePolicy {
            max_size: 65536, // 64 KiB
            admissible_types: vec![
                CapsuleType::ContextAnchor,
                CapsuleType::Commitment,
                CapsuleType::Boundary,
                CapsuleType::Attachment,
            ],
            provenance_required: true,
            hash_binding: "sha256".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Capsule
// ---------------------------------------------------------------------------

/// An evidence capsule — a hash-bound, policy-constrained evidence container.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capsule {
    pub capsule_id: String,
    pub capsule_type: CapsuleType,
    pub size_limit: usize,
    pub content_digest: String,
    pub manifest_digest: String,
    pub policy_ref: String,
    pub provenance: String,
}

impl Capsule {
    /// Create a new capsule from content, applying policy constraints.
    pub fn create(
        content: &[u8],
        capsule_type: CapsuleType,
        policy: &CapsulePolicy,
        provenance: &str,
        manifest_digest: &str,
    ) -> Result<Self, CapsuleError> {
        // Check size
        if content.len() > policy.max_size {
            return Err(CapsuleError::SizeExceeded {
                actual: content.len(),
                limit: policy.max_size,
            });
        }

        // Check type admissibility
        if !policy.admissible_types.contains(&capsule_type) {
            return Err(CapsuleError::TypeNotAdmissible(capsule_type));
        }

        // Check provenance requirement
        if policy.provenance_required && provenance.is_empty() {
            return Err(CapsuleError::MissingProvenance);
        }

        let content_digest = digest_bytes(content);
        let capsule_id = digest_bytes(
            format!("{}:{}:{}", content_digest, manifest_digest, provenance).as_bytes(),
        );
        let policy_ref = digest_bytes(
            serde_json::to_string(policy)
                .unwrap_or_default()
                .as_bytes(),
        );

        Ok(Capsule {
            capsule_id,
            capsule_type,
            size_limit: policy.max_size,
            content_digest,
            manifest_digest: manifest_digest.to_string(),
            policy_ref,
            provenance: provenance.to_string(),
        })
    }

    /// Verify that content matches the capsule's content digest.
    pub fn verify(&self, content: &[u8]) -> bool {
        let actual = digest_bytes(content);
        actual == self.content_digest
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum CapsuleError {
    #[error("Content size {actual} exceeds limit {limit}")]
    SizeExceeded { actual: usize, limit: usize },
    #[error("Capsule type {0:?} not admissible under policy")]
    TypeNotAdmissible(CapsuleType),
    #[error("Provenance is required but was empty")]
    MissingProvenance,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_valid_capsule() {
        let policy = CapsulePolicy::default();
        let content = b"test evidence data";
        let capsule = Capsule::create(
            content,
            CapsuleType::Commitment,
            &policy,
            "epoch-0",
            "manifest-digest-abc",
        )
        .unwrap();
        assert_eq!(capsule.capsule_id.len(), 64);
        assert!(capsule.verify(content));
    }

    #[test]
    fn test_verify_wrong_content() {
        let policy = CapsulePolicy::default();
        let capsule = Capsule::create(
            b"original",
            CapsuleType::ContextAnchor,
            &policy,
            "prov",
            "mfst",
        )
        .unwrap();
        assert!(!capsule.verify(b"tampered"));
    }

    #[test]
    fn test_size_exceeded() {
        let policy = CapsulePolicy {
            max_size: 4,
            ..Default::default()
        };
        let result = Capsule::create(
            b"too long",
            CapsuleType::Commitment,
            &policy,
            "prov",
            "mfst",
        );
        assert!(matches!(result, Err(CapsuleError::SizeExceeded { .. })));
    }

    #[test]
    fn test_type_not_admissible() {
        let policy = CapsulePolicy {
            admissible_types: vec![CapsuleType::ContextAnchor],
            ..Default::default()
        };
        let result = Capsule::create(
            b"data",
            CapsuleType::Attachment,
            &policy,
            "prov",
            "mfst",
        );
        assert!(matches!(result, Err(CapsuleError::TypeNotAdmissible(_))));
    }

    #[test]
    fn test_missing_provenance() {
        let policy = CapsulePolicy {
            provenance_required: true,
            ..Default::default()
        };
        let result = Capsule::create(b"data", CapsuleType::Commitment, &policy, "", "mfst");
        assert!(matches!(result, Err(CapsuleError::MissingProvenance)));
    }

    #[test]
    fn test_provenance_not_required() {
        let policy = CapsulePolicy {
            provenance_required: false,
            ..Default::default()
        };
        let capsule =
            Capsule::create(b"data", CapsuleType::Commitment, &policy, "", "mfst").unwrap();
        assert!(capsule.provenance.is_empty());
    }
}
