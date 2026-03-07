//! CLI binary for the Glyph Foundry.
//! Implements Section 23 of the Glyph Foundry Specification v2.1.

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process;
use std::time::Instant;

use glyph_bundle::{emit_bundle, EvidenceRecord, PipelineResult, TraceRecord};
use glyph_canon::{canonical_json, digest_bytes, digest_object};
use glyph_embed::{compute_embedding, compute_genome_id, fingerprint};
use glyph_expand::expand;
use glyph_gate::evaluate_obligations;
use glyph_ir::IrDocument;
use glyph_mef::{MefChain, OperatorEvidence};
use glyph_rd::RunDescriptor;
use glyph_registry::{MacroRegistry, ObligationRegistry};
use glyph_tic::Tic;
use glyph_verify::verify_bundle;

#[derive(Parser)]
#[command(name = "glyph", version = "0.2.0", about = "Glyph Foundry CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Emit a default Run Descriptor.
    InitRd,
    /// Execute the full 8-stage pipeline and emit a bundle.
    Run {
        /// Source file (REQUIRED).
        #[arg(short = 'i', long = "input")]
        input: PathBuf,
        /// Run Descriptor file (REQUIRED).
        #[arg(long)]
        rd: PathBuf,
        /// Bundle output directory.
        #[arg(short = 'o', long = "output-dir", default_value = "./bundle")]
        output_dir: PathBuf,
        /// Enable macro expansion (S2).
        #[arg(long, default_value_t = true)]
        expand: bool,
        /// Enable crystallization (S4).
        #[arg(long, default_value_t = true)]
        crystallize: bool,
        /// Frontend ID.
        #[arg(long, default_value = "sanskroot")]
        frontend: String,
    },
    /// Verify bundle integrity.
    Verify {
        /// Bundle directory.
        bundle_dir: PathBuf,
    },
    /// Parse and canonicalize, emit IR to stdout.
    DumpIr {
        /// Source file.
        #[arg(short = 'i', long = "input")]
        input: PathBuf,
        /// Frontend ID.
        #[arg(long, default_value = "sanskroot")]
        frontend: String,
    },
    /// Compute and print H5 embedding and fingerprint.
    Embed {
        /// Source file.
        #[arg(short = 'i', long = "input")]
        input: PathBuf,
        /// Frontend ID.
        #[arg(long, default_value = "sanskroot")]
        frontend: String,
    },
}

fn main() {
    let cli = Cli::parse();
    let exit_code = match cli.command {
        Commands::InitRd => cmd_init_rd(),
        Commands::Run {
            input,
            rd,
            output_dir,
            expand: do_expand,
            crystallize,
            frontend,
        } => cmd_run(input, rd, output_dir, do_expand, crystallize, frontend),
        Commands::Verify { bundle_dir } => cmd_verify(bundle_dir),
        Commands::DumpIr { input, frontend } => cmd_dump_ir(input, frontend),
        Commands::Embed { input, frontend } => cmd_embed(input, frontend),
    };
    process::exit(exit_code);
}

fn cmd_init_rd() -> i32 {
    let rd = RunDescriptor::default_rd();
    println!("{}", rd.to_json_pretty());
    0
}

/// S0: Ingest raw source bytes, normalize Unicode.
fn s0_ingest(
    source_bytes: &[u8],
    rd: &RunDescriptor,
) -> Result<(String, String), (i32, String)> {
    // Strip BOM if present
    let text = String::from_utf8_lossy(source_bytes);
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(&text).to_string();

    // Apply Unicode normalization (NFC by default)
    let normalized = match rd.unicode_policy.normalization_form.as_str() {
        "NFC" => {
            use unicode_normalization::UnicodeNormalization;
            text.nfc().collect::<String>()
        }
        "NFD" => {
            use unicode_normalization::UnicodeNormalization;
            text.nfd().collect::<String>()
        }
        _ => text,
    };

    // Check forbidden ranges
    for range in &rd.unicode_policy.forbidden_ranges {
        for (byte_offset, ch) in normalized.char_indices() {
            let cp = ch as u32;
            if cp >= range.start && cp <= range.end {
                return Err((
                    4,
                    format!(
                        "Forbidden character U+{:04X} at byte offset {}",
                        cp, byte_offset
                    ),
                ));
            }
        }
    }

    let d_src = digest_bytes(normalized.as_bytes());
    Ok((normalized, d_src))
}

/// S1: Parse and canonicalize into IR.
fn s1_canon(
    normalized: &str,
    source_digest: &str,
    frontend: &str,
) -> Result<IrDocument, (i32, String)> {
    match frontend {
        "sanskroot" => {
            let tokens = glyph_frontends::sanskroot::lexer::lex(normalized)
                .map_err(|e| (3, format!("Lex error: {}", e)))?;
            let program = glyph_frontends::sanskroot::parser::parse(&tokens)
                .map_err(|e| (3, format!("Parse error: {}", e)))?;
            let doc = glyph_frontends::sanskroot::lower::lower(&program, source_digest);
            Ok(doc)
        }
        _ => Err((1, format!("Unknown frontend: {}", frontend))),
    }
}

fn cmd_run(
    input: PathBuf,
    rd_path: PathBuf,
    output_dir: PathBuf,
    do_expand: bool,
    crystallize: bool,
    frontend: String,
) -> i32 {
    let mut tick: u64 = 0;
    let mut mef_chain = MefChain::new();
    let mut trace = Vec::new();
    let mut tic = Tic::new();

    // Load RD
    let rd = match RunDescriptor::load(&rd_path) {
        Ok(rd) => rd,
        Err(e) => {
            eprintln!("Error loading RD: {}", e);
            return 1;
        }
    };

    // Load source
    let source_bytes = match std::fs::read(&input) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Error reading source file: {}", e);
            return 1;
        }
    };

    // S0: Ingest
    let t0 = Instant::now();
    let (normalized, d_src) = match s0_ingest(&source_bytes, &rd) {
        Ok(r) => r,
        Err((code, msg)) => {
            eprintln!("S0 Ingest error: {}", msg);
            return code;
        }
    };
    let before_digest = digest_bytes(b"");
    let after_digest = digest_bytes(normalized.as_bytes());
    mef_chain.append(
        "ingest".to_string(),
        tick,
        after_digest.clone(),
        OperatorEvidence {
            step: 0,
            role: "ingest".to_string(),
            operator: "s0_ingest".to_string(),
            before_digest: before_digest.clone(),
            after_digest: after_digest.clone(),
            evidence_digest: d_src.clone(),
        },
    );
    trace.push(TraceRecord {
        stage: "S0_ingest".to_string(),
        tick,
        duration_ns: t0.elapsed().as_nanos() as u64,
        input_digest: before_digest,
        output_digest: after_digest,
    });
    tick += 1;

    // S1: Canon
    let t1 = Instant::now();
    let mut ir_doc = match s1_canon(&normalized, &d_src, &frontend) {
        Ok(doc) => doc,
        Err((code, msg)) => {
            eprintln!("S1 Canon error: {}", msg);
            // Clean up on failure (no partial bundle)
            return code;
        }
    };
    let ir_digest = ir_doc.digest();
    mef_chain.append(
        "canon".to_string(),
        tick,
        ir_digest.clone(),
        OperatorEvidence {
            step: 1,
            role: "canon".to_string(),
            operator: "s1_canon".to_string(),
            before_digest: d_src.clone(),
            after_digest: ir_digest.clone(),
            evidence_digest: ir_digest.clone(),
        },
    );
    trace.push(TraceRecord {
        stage: "S1_canon".to_string(),
        tick,
        duration_ns: t1.elapsed().as_nanos() as u64,
        input_digest: d_src,
        output_digest: ir_digest.clone(),
    });
    tick += 1;

    // S2: Expand (macro expansion)
    let t2 = Instant::now();
    if do_expand {
        let macro_registry = MacroRegistry::default_empty();
        let _records = expand(&mut ir_doc, &macro_registry);
    }
    let ir_digest_post_expand = ir_doc.digest();
    mef_chain.append(
        "expand".to_string(),
        tick,
        ir_digest_post_expand.clone(),
        OperatorEvidence {
            step: 2,
            role: "expand".to_string(),
            operator: "s2_expand".to_string(),
            before_digest: ir_digest.clone(),
            after_digest: ir_digest_post_expand.clone(),
            evidence_digest: ir_digest_post_expand.clone(),
        },
    );
    trace.push(TraceRecord {
        stage: "S2_expand".to_string(),
        tick,
        duration_ns: t2.elapsed().as_nanos() as u64,
        input_digest: ir_digest,
        output_digest: ir_digest_post_expand.clone(),
    });
    tick += 1;

    // S3: Sign (compute H5 embedding)
    let t3 = Instant::now();
    let embedding = compute_embedding(&ir_doc);
    let fp = fingerprint(&embedding);
    let genome_id = compute_genome_id(&ir_doc);
    ir_doc.embedding.axes = embedding;
    ir_doc.embedding.fingerprint = fp.clone();
    ir_doc.embedding.genome_id = genome_id.clone();
    let ir_digest_signed = ir_doc.digest();
    mef_chain.append(
        "sign".to_string(),
        tick,
        ir_digest_signed.clone(),
        OperatorEvidence {
            step: 3,
            role: "sign".to_string(),
            operator: "s3_sign".to_string(),
            before_digest: ir_digest_post_expand,
            after_digest: ir_digest_signed.clone(),
            evidence_digest: fp.clone(),
        },
    );
    trace.push(TraceRecord {
        stage: "S3_sign".to_string(),
        tick,
        duration_ns: t3.elapsed().as_nanos() as u64,
        input_digest: ir_digest_signed.clone(),
        output_digest: fp.clone(),
    });
    tick += 1;

    // S4: Condense (crystallization)
    let t4 = Instant::now();
    let crystal = if crystallize {
        // For a single run, we append one TIC observation
        tic.append(tick, glyph_q16::ZERO, rd.convergence_policy.s_min);

        if tic.crystallization_ready(rd.convergence_policy.s_min, rd.convergence_policy.w) {
            let crystal = glyph_crystal::Crystal::construct(
                ir_digest_signed.clone(),
                fp.clone(),
                genome_id.clone(),
                embedding,
                &tic,
                rd.compute_run_id(),
                vec![],
                tick,
            );
            mef_chain.append(
                "condense".to_string(),
                tick,
                crystal.crystal_id.clone(),
                OperatorEvidence {
                    step: 4,
                    role: "condense".to_string(),
                    operator: "s4_condense".to_string(),
                    before_digest: ir_digest_signed.clone(),
                    after_digest: crystal.crystal_id.clone(),
                    evidence_digest: crystal.crystal_id.clone(),
                },
            );
            Some(crystal)
        } else {
            None
        }
    } else {
        None
    };
    trace.push(TraceRecord {
        stage: "S4_condense".to_string(),
        tick,
        duration_ns: t4.elapsed().as_nanos() as u64,
        input_digest: ir_digest_signed.clone(),
        output_digest: crystal
            .as_ref()
            .map(|c| c.crystal_id.clone())
            .unwrap_or_else(|| "none".to_string()),
    });
    tick += 1;

    // S5: Gate (obligation evaluation)
    let t5 = Instant::now();
    let obligation_registry = ObligationRegistry::default_empty();
    let gate_result = evaluate_obligations(&ir_doc, &embedding, &obligation_registry);
    if !gate_result.passed {
        eprintln!("Hard obligation failure");
        return 2;
    }
    let evidence: Vec<EvidenceRecord> = gate_result
        .results
        .iter()
        .map(|r| EvidenceRecord {
            obligation_id: r.id.clone(),
            class: r.class.clone(),
            passed: r.passed,
            details: r.evidence.clone(),
        })
        .collect();
    let evidence_value = serde_json::to_value(&evidence).unwrap_or_default();
    let evidence_digest = digest_object(&evidence_value);
    mef_chain.append(
        "gate".to_string(),
        tick,
        evidence_digest.clone(),
        OperatorEvidence {
            step: 5,
            role: "gate".to_string(),
            operator: "s5_gate".to_string(),
            before_digest: ir_digest_signed.clone(),
            after_digest: evidence_digest.clone(),
            evidence_digest: evidence_digest.clone(),
        },
    );
    trace.push(TraceRecord {
        stage: "S5_gate".to_string(),
        tick,
        duration_ns: t5.elapsed().as_nanos() as u64,
        input_digest: ir_digest_signed.clone(),
        output_digest: evidence_digest,
    });
    tick += 1;

    // S6: Emit bundle
    let t6 = Instant::now();
    let pipeline_result = PipelineResult {
        normalized_source: normalized,
        ir_document: ir_doc,
        rd,
        mef_chain,
        tic,
        crystal,
        trace: trace.clone(),
        evidence,
        embedding_fingerprint: fp,
        genome_id,
    };
    match emit_bundle(&pipeline_result, &output_dir) {
        Ok(path) => {
            eprintln!("Bundle emitted to: {}", path.display());
        }
        Err(e) => {
            eprintln!("S6 Emit error: {}", e);
            // Ensure no partial bundle
            let _ = std::fs::remove_dir_all(&output_dir);
            return 1;
        }
    }

    eprintln!("Pipeline completed successfully.");
    0
}

fn cmd_verify(bundle_dir: PathBuf) -> i32 {
    let result = verify_bundle(&bundle_dir);
    if result.passed {
        eprintln!("Verification passed: all digests match.");
        0
    } else {
        for err in &result.errors {
            eprintln!("  {}", err);
        }
        match result.errors.first() {
            Some(glyph_verify::VerifyError::MissingFile(_)) => 12,
            Some(glyph_verify::VerifyError::DigestMismatch { .. }) => 10,
            Some(glyph_verify::VerifyError::MefTamper(_)) => 13,
            Some(glyph_verify::VerifyError::ReplayMismatch(_)) => 11,
            _ => 10,
        }
    }
}

fn cmd_dump_ir(input: PathBuf, frontend: String) -> i32 {
    let source_bytes = match std::fs::read(&input) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Error reading source: {}", e);
            return 1;
        }
    };
    let text = String::from_utf8_lossy(&source_bytes);
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(&text);
    use unicode_normalization::UnicodeNormalization;
    let normalized: String = text.nfc().collect();
    let d_src = digest_bytes(normalized.as_bytes());

    let ir_doc = match s1_canon(&normalized, &d_src, &frontend) {
        Ok(doc) => doc,
        Err((code, msg)) => {
            eprintln!("{}", msg);
            return code;
        }
    };

    let value = serde_json::to_value(&ir_doc).unwrap();
    let canonical = canonical_json(&value);
    let output = String::from_utf8_lossy(&canonical);
    println!("{}", output);
    0
}

fn cmd_embed(input: PathBuf, frontend: String) -> i32 {
    let source_bytes = match std::fs::read(&input) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Error reading source: {}", e);
            return 1;
        }
    };
    let text = String::from_utf8_lossy(&source_bytes);
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(&text);
    use unicode_normalization::UnicodeNormalization;
    let normalized: String = text.nfc().collect();
    let d_src = digest_bytes(normalized.as_bytes());

    let ir_doc = match s1_canon(&normalized, &d_src, &frontend) {
        Ok(doc) => doc,
        Err((code, msg)) => {
            eprintln!("{}", msg);
            return code;
        }
    };

    let emb = compute_embedding(&ir_doc);
    let fp = fingerprint(&emb);

    // Output: 5 Q16 values tab-separated, then fingerprint on next line
    println!(
        "{}\t{}\t{}\t{}\t{}",
        emb[0].raw(),
        emb[1].raw(),
        emb[2].raw(),
        emb[3].raw(),
        emb[4].raw()
    );
    println!("{}", fp);
    0
}
