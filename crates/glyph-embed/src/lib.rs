//! H5 embedding space, axis functions, kernel, and fingerprint.
//! Implements Section 6 of the Glyph Foundry Specification v2.1.

use glyph_canon::digest_bytes;
use glyph_ir::{IrDocument, NodeKind};
use glyph_q16::Q16;
use std::collections::BTreeMap;

/// A point in the 5-dimensional embedding space H5 = Q16^5.
pub type H5Point = [Q16; 5];

/// Compute the H5 embedding for an IR document.
/// Axes: a1=structural_coupling, a2=functional_density,
///        a3=topological_complexity, a4=symmetry, a5=entropy.
pub fn compute_embedding(doc: &IrDocument) -> H5Point {
    [
        a1_structural_coupling(doc),
        a2_functional_density(doc),
        a3_topological_complexity(doc),
        a4_symmetry(doc),
        a5_entropy(doc),
    ]
}

/// a1: Structural coupling — ratio of cross-scope edges to total edges.
fn a1_structural_coupling(doc: &IrDocument) -> Q16 {
    if doc.edges.is_empty() {
        return Q16::from_raw(0);
    }
    // Build a map of node_id -> parent function name
    let mut node_scope: BTreeMap<&str, &str> = BTreeMap::new();
    for edge in &doc.edges {
        if edge.kind == glyph_ir::EdgeKind::Contains {
            // Find the src node's kind
            if let Some(src_node) = doc.nodes.iter().find(|n| n.id == edge.src) {
                if src_node.kind == NodeKind::Function {
                    node_scope.insert(&edge.dst, &src_node.name);
                }
            }
        }
    }

    let mut cross_count: i64 = 0;
    let total = doc.edges.len() as i64;
    for edge in &doc.edges {
        let src_scope = node_scope.get(edge.src.as_str());
        let dst_scope = node_scope.get(edge.dst.as_str());
        if src_scope != dst_scope && src_scope.is_some() && dst_scope.is_some() {
            cross_count += 1;
        }
    }

    // Q16: (cross_count << 16) / total
    if total == 0 {
        Q16::from_raw(0)
    } else {
        Q16::from_raw(((cross_count << 16) / total) as i32)
    }
}

/// a2: Functional density — function count / total node count.
fn a2_functional_density(doc: &IrDocument) -> Q16 {
    if doc.nodes.is_empty() {
        return Q16::from_raw(0);
    }
    let func_count = doc
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .count() as i64;
    let total = doc.nodes.len() as i64;
    Q16::from_raw(((func_count << 16) / total) as i32)
}

/// a3: Topological complexity — (If + BinaryOp count) / total nodes.
fn a3_topological_complexity(doc: &IrDocument) -> Q16 {
    if doc.nodes.is_empty() {
        return Q16::from_raw(0);
    }
    let complex_count = doc
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::If || n.kind == NodeKind::BinaryOp)
        .count() as i64;
    let total = doc.nodes.len() as i64;
    Q16::from_raw(((complex_count << 16) / total) as i32)
}

/// a4: Symmetry — uniformity of kind distribution (max_kind_count / total).
fn a4_symmetry(doc: &IrDocument) -> Q16 {
    if doc.nodes.is_empty() {
        return Q16::from_raw(0);
    }
    let mut counts: BTreeMap<&str, i64> = BTreeMap::new();
    for node in &doc.nodes {
        *counts.entry(node.kind.as_str()).or_insert(0) += 1;
    }
    let max_count = counts.values().copied().max().unwrap_or(0);
    let total = doc.nodes.len() as i64;
    // Symmetry = 1 - (max/total), so more uniform = higher symmetry
    let max_ratio = (max_count << 16) / total;
    Q16::from_raw((65536 - max_ratio) as i32)
}

/// a5: Entropy — Shannon-like measure using Q16 integer approximation.
fn a5_entropy(doc: &IrDocument) -> Q16 {
    if doc.nodes.is_empty() {
        return Q16::from_raw(0);
    }
    let mut counts: BTreeMap<&str, i64> = BTreeMap::new();
    for node in &doc.nodes {
        *counts.entry(node.kind.as_str()).or_insert(0) += 1;
    }
    let total = doc.nodes.len() as i64;
    let _num_kinds = counts.len() as i64;

    // Normalized entropy approximation:
    // For each kind, contribution = -(count/total) * log2(count/total)
    // Approximate using integer arithmetic:
    // sum of count * (log2(total) - log2(count)) / total
    // Use integer log2 approximation
    let mut entropy_sum: i64 = 0;
    for &count in counts.values() {
        if count > 0 {
            let log_total = integer_log2(total as u64);
            let log_count = integer_log2(count as u64);
            // Contribution in Q16: count * (log_total - log_count) << 16 / total
            let diff = log_total.saturating_sub(log_count) as i64;
            entropy_sum += count * diff;
        }
    }

    // Normalize: entropy_sum << 16 / (total * max_possible_log)
    let max_log = integer_log2(total as u64).max(1) as i64;
    let normalized = if max_log > 0 && total > 0 {
        (entropy_sum << 16) / (total * max_log)
    } else {
        0
    };

    Q16::from_raw(normalized.clamp(0, 65536) as i32)
}

/// Integer approximation of log2 (returns value * 16 for more precision).
fn integer_log2(v: u64) -> u64 {
    if v <= 1 {
        return 0;
    }
    let bits = 63 - v.leading_zeros() as u64;
    bits * 16
}

/// Compute the Zero-Float Fingerprint (Definition 6.3):
/// ZFP(e) = SHA256(hex(a1) + ":" + hex(a2) + ... + ":" + hex(a5))
pub fn fingerprint(point: &H5Point) -> String {
    let hex_parts: Vec<String> = point.iter().map(|q| q.to_hex()).collect();
    let input = hex_parts.join(":");
    digest_bytes(input.as_bytes())
}

/// Compute the genome ID for an IR document.
/// genome_id = digest of the sorted observable measurements.
pub fn compute_genome_id(doc: &IrDocument) -> String {
    let embedding = compute_embedding(doc);
    let genome_data = serde_json::json!({
        "axes": embedding.iter().map(|q| q.raw()).collect::<Vec<_>>(),
        "node_count": doc.nodes.len(),
        "edge_count": doc.edges.len(),
    });
    let bytes = glyph_canon::canonical_json(&genome_data);
    digest_bytes(&bytes)
}

/// Resonance kernel (Definition 6.4):
/// K(ex, ey; sigma) = prod_i exp(-(xi - yi)^2 / (2 * sigma_i^2))
pub fn kernel(ex: &H5Point, ey: &H5Point, sigma: &[Q16; 5]) -> Q16 {
    let mut result = Q16::from_raw(65536); // 1.0 in Q16
    for i in 0..5 {
        let diff = ex[i].raw() as i64 - ey[i].raw() as i64;
        let diff_sq = diff * diff;
        let sigma_val = sigma[i].raw() as i64;
        if sigma_val == 0 {
            continue;
        }
        let two_sigma_sq = 2 * sigma_val * sigma_val;
        // exp(-diff_sq / two_sigma_sq) approximated in Q16
        let ratio = if two_sigma_sq > 0 {
            (diff_sq << 16) / two_sigma_sq
        } else {
            0
        };
        let exp_val = q16_exp_neg(Q16::from_raw(ratio.clamp(0, i32::MAX as i64) as i32));
        result = result.checked_mul(exp_val).unwrap_or(Q16::from_raw(0));
    }
    result
}

/// Kernel distance (Definition 6.5):
/// delta(ex, ey; sigma) = sqrt(2 - 2*K(ex, ey; sigma))
pub fn kernel_distance(ex: &H5Point, ey: &H5Point, sigma: &[Q16; 5]) -> Q16 {
    let k = kernel(ex, ey, sigma);
    // 2 - 2K in Q16: 2*65536 - 2*k.raw()
    let two_minus_2k = 2i64 * 65536 - 2i64 * k.raw() as i64;
    if two_minus_2k <= 0 {
        return Q16::from_raw(0);
    }
    q16_sqrt(Q16::from_raw(two_minus_2k.clamp(0, i32::MAX as i64) as i32))
}

/// Approximate exp(-x) for x >= 0 using Taylor series in Q16.
/// exp(-x) ≈ 1 - x + x²/2 - x³/6 + x⁴/24
fn q16_exp_neg(x: Q16) -> Q16 {
    if x.raw() <= 0 {
        return Q16::from_raw(65536); // exp(0) = 1.0
    }
    if x.raw() > 4 * 65536 {
        return Q16::from_raw(0); // exp(-4) ≈ 0.018, close enough to 0
    }

    let one = 65536i64;
    let xr = x.raw() as i64;

    // Taylor: 1 - x + x²/2 - x³/6 + x⁴/24
    let x2 = (xr * xr) >> 16;
    let x3 = (x2 * xr) >> 16;
    let x4 = (x3 * xr) >> 16;

    let result = one - xr + (x2 >> 1) - (x3 / 6) + (x4 / 24);
    Q16::from_raw(result.clamp(0, 65536) as i32)
}

/// Integer square root in Q16 using Newton's method.
fn q16_sqrt(x: Q16) -> Q16 {
    if x.raw() <= 0 {
        return Q16::from_raw(0);
    }
    // sqrt in Q16: result = sqrt(raw << 16)
    let val = (x.raw() as i64) << 16;
    let mut guess = val;
    for _ in 0..30 {
        if guess == 0 {
            break;
        }
        let new_guess = (guess + val / guess) / 2;
        if (new_guess - guess).abs() <= 1 {
            break;
        }
        guess = new_guess;
    }
    Q16::from_raw(guess.clamp(0, i32::MAX as i64) as i32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use glyph_ir::*;

    fn make_test_doc() -> IrDocument {
        let mut doc = IrDocument::new("sanskroot", "test_digest");
        doc.nodes.push(IrNode::new("n_mod", NodeKind::Module, "root"));
        doc.nodes
            .push(IrNode::new("n_fn", NodeKind::Function, "main"));
        doc.nodes
            .push(IrNode::new("n_blk", NodeKind::Block, "block0"));
        doc.nodes
            .push(IrNode::new("n_call", NodeKind::Call, "print"));
        doc.nodes.push(IrNode::new("n_lit", NodeKind::Literal, "hello"));
        doc.edges
            .push(IrEdge::new("n_mod", "n_fn", EdgeKind::Contains));
        doc.edges
            .push(IrEdge::new("n_fn", "n_blk", EdgeKind::Contains));
        doc.edges
            .push(IrEdge::new("n_blk", "n_call", EdgeKind::Contains));
        doc.edges
            .push(IrEdge::new("n_call", "n_lit", EdgeKind::Argument));
        doc
    }

    #[test]
    fn test_compute_embedding() {
        let doc = make_test_doc();
        let emb = compute_embedding(&doc);
        // All axes should be valid Q16 values
        for q in &emb {
            assert!(q.raw() >= 0);
        }
    }

    #[test]
    fn test_fingerprint_deterministic() {
        let doc = make_test_doc();
        let emb = compute_embedding(&doc);
        let fp1 = fingerprint(&emb);
        let fp2 = fingerprint(&emb);
        assert_eq!(fp1, fp2);
        assert_eq!(fp1.len(), 64);
    }

    #[test]
    fn test_kernel_self() {
        let sigma = [Q16::from_raw(65536); 5];
        let point = [Q16::from_int(1); 5];
        let k = kernel(&point, &point, &sigma);
        // K(e, e) should be close to 1.0 = 65536
        assert_eq!(k.raw(), 65536);
    }

    #[test]
    fn test_kernel_distance_self() {
        let sigma = [Q16::from_raw(65536); 5];
        let point = [Q16::from_int(1); 5];
        let d = kernel_distance(&point, &point, &sigma);
        assert_eq!(d.raw(), 0);
    }

    #[test]
    fn test_genome_id_deterministic() {
        let doc = make_test_doc();
        let g1 = compute_genome_id(&doc);
        let g2 = compute_genome_id(&doc);
        assert_eq!(g1, g2);
    }
}
