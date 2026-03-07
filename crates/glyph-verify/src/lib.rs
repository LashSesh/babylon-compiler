//! Digest verification, replay, and divergence reporting.
//! Implements Section 22.3 of the Glyph Foundry Specification v2.1.

use glyph_bundle::{load_manifest, Manifest};
use glyph_canon::digest_bytes;
use glyph_mef::MefChain;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum VerifyError {
    #[error("Missing file: {0}")]
    MissingFile(String),
    #[error("Digest mismatch in {file}: expected {expected}, actual {actual}")]
    DigestMismatch {
        file: String,
        field: String,
        expected: String,
        actual: String,
    },
    #[error("MEF chain tamper detected at block {0}")]
    MefTamper(u64),
    #[error("Replay mismatch: {0}")]
    ReplayMismatch(String),
    #[error("I/O error: {0}")]
    Io(String),
}

/// Result of bundle verification.
#[derive(Debug)]
pub struct VerifyResult {
    pub passed: bool,
    pub errors: Vec<VerifyError>,
}

/// Verify all digests in a bundle directory against the manifest.
pub fn verify_bundle(bundle_dir: &Path) -> VerifyResult {
    let mut errors = Vec::new();

    // Load manifest
    let manifest = match load_manifest(bundle_dir) {
        Ok(m) => m,
        Err(e) => {
            return VerifyResult {
                passed: false,
                errors: vec![VerifyError::MissingFile(format!(
                    "manifest.json: {}",
                    e
                ))],
            };
        }
    };

    // Verify each file digest
    let checks: Vec<(&str, &str)> = vec![
        ("source.norm.txt", &manifest.source_digest),
        ("ir.json", &manifest.ir_digest),
        ("mef.json", &manifest.mef_digest),
        ("trace.json", &manifest.trace_digest),
        ("tic.json", &manifest.tic_digest),
        ("evidence.json", &manifest.evidence_digest),
    ];

    for (file, expected_digest) in checks {
        let path = bundle_dir.join(file);
        match std::fs::read(&path) {
            Ok(data) => {
                let actual = digest_bytes(&data);
                if actual != *expected_digest {
                    errors.push(VerifyError::DigestMismatch {
                        file: file.to_string(),
                        field: format!("{}_digest", file.split('.').next().unwrap_or(file)),
                        expected: expected_digest.to_string(),
                        actual,
                    });
                }
            }
            Err(_) => {
                errors.push(VerifyError::MissingFile(file.to_string()));
            }
        }
    }

    // Verify crystal.json if digest is present
    if let Some(ref expected) = manifest.crystal_digest {
        let path = bundle_dir.join("crystal.json");
        match std::fs::read(&path) {
            Ok(data) => {
                let actual = digest_bytes(&data);
                if actual != *expected {
                    errors.push(VerifyError::DigestMismatch {
                        file: "crystal.json".to_string(),
                        field: "crystal_digest".to_string(),
                        expected: expected.clone(),
                        actual,
                    });
                }
            }
            Err(_) => {
                errors.push(VerifyError::MissingFile("crystal.json".to_string()));
            }
        }
    }

    // Verify MEF chain integrity
    let mef_path = bundle_dir.join("mef.json");
    if let Ok(data) = std::fs::read_to_string(&mef_path) {
        if let Ok(chain) = serde_json::from_str::<MefChain>(&data) {
            if let Err(e) = chain.verify() {
                errors.push(VerifyError::MefTamper(e.block_index));
            }
        }
    }

    VerifyResult {
        passed: errors.is_empty(),
        errors,
    }
}
