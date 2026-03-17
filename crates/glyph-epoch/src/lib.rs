//! Epoch management for COL language evolution.
//! Implements the HDAG-based epoch system from the COL Master Specification v3.0.

use glyph_registry::compute_digest;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Epoch policy and lineage
// ---------------------------------------------------------------------------

/// Policy set bound to an epoch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpochPolicy {
    pub unicode_normalization: String,
    pub strict_mode: bool,
    pub seed_mode: String,
    pub seed: String,
}

impl Default for EpochPolicy {
    fn default() -> Self {
        EpochPolicy {
            unicode_normalization: "NFC".to_string(),
            strict_mode: false,
            seed_mode: "fixed".to_string(),
            seed: "0".to_string(),
        }
    }
}

/// Lineage tracking for HDAG (Hash DAG) epochs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpochLineage {
    pub parents: Vec<String>,
    pub depth: u64,
}

// ---------------------------------------------------------------------------
// Epoch object
// ---------------------------------------------------------------------------

/// An Epoch — immutable snapshot of all registries and policies at a point in evolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Epoch {
    pub epoch_id: String,
    pub parent_epoch: Option<String>,
    pub sign_registry_digest: String,
    pub macro_registry_digest: String,
    pub profile_registry_digest: String,
    pub obligation_registry_digest: String,
    pub gate_registry_digest: String,
    pub policy_set: EpochPolicy,
    pub lineage: EpochLineage,
    pub created_at: String,
    pub epoch_digest: String,
}

/// Registry digests used to construct an epoch.
#[derive(Debug, Clone)]
pub struct EpochRegistryDigests {
    pub sign: String,
    pub r#macro: String,
    pub profile: String,
    pub obligation: String,
    pub gate: String,
}

impl Epoch {
    /// Construct a new epoch from registry digests and policy.
    /// epoch_digest = SHA-256(self serialized with epoch_digest="")
    pub fn construct(
        registries: EpochRegistryDigests,
        policy: EpochPolicy,
        parent: Option<&Epoch>,
    ) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        let (parent_epoch, lineage) = match parent {
            Some(p) => (
                Some(p.epoch_id.clone()),
                EpochLineage {
                    parents: vec![p.epoch_id.clone()],
                    depth: p.lineage.depth + 1,
                },
            ),
            None => (
                None,
                EpochLineage {
                    parents: vec![],
                    depth: 0,
                },
            ),
        };

        let mut epoch = Epoch {
            epoch_id: String::new(),
            parent_epoch,
            sign_registry_digest: registries.sign,
            macro_registry_digest: registries.r#macro,
            profile_registry_digest: registries.profile,
            obligation_registry_digest: registries.obligation,
            gate_registry_digest: registries.gate,
            policy_set: policy,
            lineage,
            created_at: now,
            epoch_digest: String::new(),
        };

        // Compute epoch_digest with epoch_digest="" and epoch_id=""
        let digest = compute_digest(&epoch);
        epoch.epoch_digest = digest.clone();
        epoch.epoch_id = digest;

        epoch
    }

    /// Verify the epoch digest by recomputing it.
    pub fn verify_digest(&self) -> bool {
        let mut check = self.clone();
        check.epoch_digest = String::new();
        check.epoch_id = String::new();
        let recomputed = compute_digest(&check);
        recomputed == self.epoch_digest
    }

    /// Load an epoch from a JSON file.
    pub fn load(path: &std::path::Path) -> Result<Self, EpochError> {
        let data =
            std::fs::read_to_string(path).map_err(|e| EpochError::Io(e.to_string()))?;
        let epoch: Epoch =
            serde_json::from_str(&data).map_err(|e| EpochError::Parse(e.to_string()))?;
        Ok(epoch)
    }

    /// Save this epoch to a JSON file.
    pub fn save(&self, path: &std::path::Path) -> Result<(), EpochError> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| EpochError::Serialization(e.to_string()))?;
        std::fs::write(path, json).map_err(|e| EpochError::Io(e.to_string()))?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Epoch Store (filesystem-based)
// ---------------------------------------------------------------------------

/// Simple filesystem-backed epoch store.
pub struct EpochStore {
    base_dir: std::path::PathBuf,
}

impl EpochStore {
    /// Create or open an epoch store at the given directory.
    pub fn new(base_dir: &std::path::Path) -> Result<Self, EpochError> {
        std::fs::create_dir_all(base_dir).map_err(|e| EpochError::Io(e.to_string()))?;
        Ok(EpochStore {
            base_dir: base_dir.to_path_buf(),
        })
    }

    /// Store an epoch. Never overwrites existing epochs.
    pub fn put(&self, epoch: &Epoch) -> Result<(), EpochError> {
        let path = self.base_dir.join(format!("{}.json", epoch.epoch_id));
        if path.exists() {
            return Err(EpochError::AlreadyExists(epoch.epoch_id.clone()));
        }
        epoch.save(&path)
    }

    /// Retrieve an epoch by ID.
    pub fn get(&self, epoch_id: &str) -> Result<Epoch, EpochError> {
        let path = self.base_dir.join(format!("{}.json", epoch_id));
        Epoch::load(&path)
    }

    /// List all epoch IDs.
    pub fn list(&self) -> Result<Vec<String>, EpochError> {
        let mut ids = Vec::new();
        let entries =
            std::fs::read_dir(&self.base_dir).map_err(|e| EpochError::Io(e.to_string()))?;
        for entry in entries {
            let entry = entry.map_err(|e| EpochError::Io(e.to_string()))?;
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(id) = name.strip_suffix(".json") {
                ids.push(id.to_string());
            }
        }
        ids.sort();
        Ok(ids)
    }

    /// Verify HDAG acyclicity: no epoch can be its own ancestor.
    pub fn verify_acyclicity(&self, epoch_id: &str) -> Result<bool, EpochError> {
        let mut visited = std::collections::HashSet::new();
        let mut current = epoch_id.to_string();
        loop {
            if visited.contains(&current) {
                return Ok(false); // cycle detected
            }
            visited.insert(current.clone());
            let epoch = match self.get(&current) {
                Ok(e) => e,
                Err(_) => break,
            };
            match epoch.parent_epoch {
                Some(parent) => current = parent,
                None => break,
            }
        }
        Ok(true)
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum EpochError {
    #[error("I/O error: {0}")]
    Io(String),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Epoch already exists: {0}")]
    AlreadyExists(String),
    #[error("Epoch not found: {0}")]
    NotFound(String),
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn zero_digests() -> EpochRegistryDigests {
        let z = "0".repeat(64);
        EpochRegistryDigests {
            sign: z.clone(),
            r#macro: z.clone(),
            profile: z.clone(),
            obligation: z.clone(),
            gate: z,
        }
    }

    #[test]
    fn test_epoch_construct() {
        let epoch = Epoch::construct(zero_digests(), EpochPolicy::default(), None);
        assert_eq!(epoch.epoch_digest.len(), 64);
        assert_eq!(epoch.epoch_id, epoch.epoch_digest);
        assert!(epoch.parent_epoch.is_none());
        assert_eq!(epoch.lineage.depth, 0);
    }

    #[test]
    fn test_epoch_verify_digest() {
        let epoch = Epoch::construct(zero_digests(), EpochPolicy::default(), None);
        assert!(epoch.verify_digest());
    }

    #[test]
    fn test_epoch_child() {
        let parent = Epoch::construct(zero_digests(), EpochPolicy::default(), None);
        let child = Epoch::construct(zero_digests(), EpochPolicy::default(), Some(&parent));
        assert_eq!(child.parent_epoch, Some(parent.epoch_id.clone()));
        assert_eq!(child.lineage.depth, 1);
        assert!(child.verify_digest());
    }

    #[test]
    fn test_epoch_store_roundtrip() {
        let dir = std::env::temp_dir().join("glyph_epoch_test_rt");
        let _ = std::fs::remove_dir_all(&dir);
        let store = EpochStore::new(&dir).unwrap();

        let epoch = Epoch::construct(zero_digests(), EpochPolicy::default(), None);
        store.put(&epoch).unwrap();

        let loaded = store.get(&epoch.epoch_id).unwrap();
        assert_eq!(loaded.epoch_digest, epoch.epoch_digest);

        let ids = store.list().unwrap();
        assert!(ids.contains(&epoch.epoch_id));

        assert!(store.verify_acyclicity(&epoch.epoch_id).unwrap());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_epoch_store_no_overwrite() {
        let dir = std::env::temp_dir().join("glyph_epoch_test_noow");
        let _ = std::fs::remove_dir_all(&dir);
        let store = EpochStore::new(&dir).unwrap();

        let epoch = Epoch::construct(zero_digests(), EpochPolicy::default(), None);
        store.put(&epoch).unwrap();
        assert!(store.put(&epoch).is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
