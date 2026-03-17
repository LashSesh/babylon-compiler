//! Bundle assembly, manifest creation, and atomic emission.
//! Implements Section 22 of the Glyph Foundry Specification v2.1.

use glyph_canon::{canonical_json, digest_bytes, digest_object};
use glyph_crystal::Crystal;
use glyph_ir::IrDocument;
use glyph_mef::MefChain;
use glyph_rd::RunDescriptor;
use glyph_tic::Tic;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The manifest schema (Section 22.2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub schema_version: String,
    pub created_at: String,
    pub rd_digest: String,
    pub source_digest: String,
    pub ir_digest: String,
    pub mef_digest: String,
    pub evidence_digest: String,
    pub trace_digest: String,
    pub tic_digest: String,
    pub crystal_digest: Option<String>,
    pub embedding_fingerprint: String,
    pub genome_id: String,
    pub registry_digests: RegistryDigests,
    pub artifact_digests: BTreeMap<String, String>,
    pub ir_schema_version: String,
    pub toolchain: ToolchainInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryDigests {
    pub operator: String,
    pub r#macro: String,
    pub obligation: String,
    pub observable: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub sign: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub profile: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub gate: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolchainInfo {
    pub name: String,
    pub version: String,
}

/// Trace record for a pipeline stage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceRecord {
    pub stage: String,
    pub tick: u64,
    pub duration_ns: u64,
    pub input_digest: String,
    pub output_digest: String,
}

/// Evidence record from gate evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceRecord {
    pub obligation_id: String,
    pub class: String,
    pub passed: bool,
    pub details: serde_json::Value,
}

/// Complete pipeline result ready for bundle emission.
pub struct PipelineResult {
    pub normalized_source: String,
    pub ir_document: IrDocument,
    pub rd: RunDescriptor,
    pub mef_chain: MefChain,
    pub tic: Tic,
    pub crystal: Option<Crystal>,
    pub trace: Vec<TraceRecord>,
    pub evidence: Vec<EvidenceRecord>,
    pub embedding_fingerprint: String,
    pub genome_id: String,
}

#[derive(Debug, thiserror::Error)]
pub enum BundleError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(String),
}

/// Emit a complete bundle to the output directory atomically.
/// On failure, the output directory must not contain a partial bundle.
pub fn emit_bundle(result: &PipelineResult, output_dir: &Path) -> Result<PathBuf, BundleError> {
    let temp_dir = output_dir.with_extension("tmp");
    if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir)?;
    }
    std::fs::create_dir_all(&temp_dir)?;
    std::fs::create_dir_all(temp_dir.join("artifacts"))?;

    // Write source.norm.txt
    std::fs::write(temp_dir.join("source.norm.txt"), &result.normalized_source)?;

    // Write ir.json (canonical)
    let ir_value = serde_json::to_value(&result.ir_document)
        .map_err(|e| BundleError::Serialization(e.to_string()))?;
    let ir_bytes = canonical_json(&ir_value);
    std::fs::write(temp_dir.join("ir.json"), &ir_bytes)?;

    // Write rd.json
    let rd_json = serde_json::to_string_pretty(&result.rd)
        .map_err(|e| BundleError::Serialization(e.to_string()))?;
    std::fs::write(temp_dir.join("rd.json"), &rd_json)?;

    // Write mef.json
    let mef_value = serde_json::to_value(&result.mef_chain)
        .map_err(|e| BundleError::Serialization(e.to_string()))?;
    let mef_bytes = canonical_json(&mef_value);
    std::fs::write(temp_dir.join("mef.json"), &mef_bytes)?;

    // Write tic.json
    let tic_value = serde_json::to_value(&result.tic)
        .map_err(|e| BundleError::Serialization(e.to_string()))?;
    let tic_bytes = canonical_json(&tic_value);
    std::fs::write(temp_dir.join("tic.json"), &tic_bytes)?;

    // Write trace.json
    let trace_value = serde_json::to_value(&result.trace)
        .map_err(|e| BundleError::Serialization(e.to_string()))?;
    let trace_bytes = canonical_json(&trace_value);
    std::fs::write(temp_dir.join("trace.json"), &trace_bytes)?;

    // Write evidence.json
    let evidence_value = serde_json::to_value(&result.evidence)
        .map_err(|e| BundleError::Serialization(e.to_string()))?;
    let evidence_bytes = canonical_json(&evidence_value);
    std::fs::write(temp_dir.join("evidence.json"), &evidence_bytes)?;

    // Write crystal.json (if present)
    let crystal_digest = if let Some(ref crystal) = result.crystal {
        let crystal_value = serde_json::to_value(crystal)
            .map_err(|e| BundleError::Serialization(e.to_string()))?;
        let crystal_bytes = canonical_json(&crystal_value);
        let digest = digest_bytes(&crystal_bytes);
        std::fs::write(temp_dir.join("crystal.json"), &crystal_bytes)?;
        Some(digest)
    } else {
        None
    };

    // Compute all digests for manifest
    let source_digest = digest_bytes(result.normalized_source.as_bytes());
    let ir_digest = digest_bytes(&ir_bytes);
    let rd_value = serde_json::to_value(&result.rd)
        .map_err(|e| BundleError::Serialization(e.to_string()))?;
    let rd_digest = digest_object(&rd_value);
    let mef_digest = digest_bytes(&mef_bytes);
    let tic_digest = digest_bytes(&tic_bytes);
    let trace_digest = digest_bytes(&trace_bytes);
    let evidence_digest = digest_bytes(&evidence_bytes);

    let now = chrono::Utc::now().to_rfc3339();
    let manifest = Manifest {
        schema_version: "2.1.0".to_string(),
        created_at: now,
        rd_digest,
        source_digest,
        ir_digest,
        mef_digest,
        evidence_digest,
        trace_digest,
        tic_digest,
        crystal_digest,
        embedding_fingerprint: result.embedding_fingerprint.clone(),
        genome_id: result.genome_id.clone(),
        registry_digests: RegistryDigests {
            operator: result.rd.registries.operator.clone(),
            r#macro: result.rd.registries.r#macro.clone(),
            obligation: result.rd.registries.obligation.clone(),
            observable: result.rd.registries.observable.clone(),
            sign: result.rd.registries.sign.clone(),
            profile: result.rd.registries.profile.clone(),
            gate: result.rd.registries.gate.clone(),
        },
        artifact_digests: BTreeMap::new(),
        ir_schema_version: "1.0.0".to_string(),
        toolchain: ToolchainInfo {
            name: result.rd.toolchain.name.clone(),
            version: result.rd.toolchain.version.clone(),
        },
    };

    let manifest_json = serde_json::to_string_pretty(&manifest)
        .map_err(|e| BundleError::Serialization(e.to_string()))?;
    std::fs::write(temp_dir.join("manifest.json"), &manifest_json)?;

    // Atomic rename
    if output_dir.exists() {
        std::fs::remove_dir_all(output_dir)?;
    }
    std::fs::rename(&temp_dir, output_dir)?;

    Ok(output_dir.to_path_buf())
}

/// Load a manifest from a bundle directory.
pub fn load_manifest(bundle_dir: &Path) -> Result<Manifest, BundleError> {
    let data = std::fs::read_to_string(bundle_dir.join("manifest.json"))?;
    serde_json::from_str(&data).map_err(|e| BundleError::Serialization(e.to_string()))
}
