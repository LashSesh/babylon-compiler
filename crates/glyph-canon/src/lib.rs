//! Canonicalization, deterministic ordering, content-hash ID allocation,
//! and canonical JSON serialization (RFC 8785 / JCS).

use sha2::{Digest as _, Sha256};
use std::collections::BTreeMap;

/// Compute SHA-256 digest of raw bytes, returning 64-char lowercase hex.
pub fn digest_bytes(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    hex_encode(&result)
}

/// Compute the canonical digest of a serde-serializable object:
/// dig(o) = SHA256(JCS(o))
pub fn digest_object(value: &serde_json::Value) -> String {
    let canonical = canonical_json(value);
    digest_bytes(&canonical)
}

/// Encode bytes as lowercase hex string.
pub fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

/// Canonical JSON serialization per RFC 8785 (JCS) and Appendix A of the spec:
/// - Object keys sorted lexicographically by UTF-8 bytes
/// - No whitespace between tokens
/// - No floating-point literals (integers only for numbers)
/// - Minimal escape sequences
pub fn canonical_json(value: &serde_json::Value) -> Vec<u8> {
    let mut buf = Vec::new();
    write_canonical_json(value, &mut buf);
    buf
}

/// Serialize a serde-serializable object to canonical JSON bytes.
pub fn canonical_json_from_serialize<T: serde::Serialize>(obj: &T) -> Vec<u8> {
    let value = serde_json::to_value(obj).expect("serialization to Value must succeed");
    canonical_json(&value)
}

fn write_canonical_json(value: &serde_json::Value, buf: &mut Vec<u8>) {
    match value {
        serde_json::Value::Null => buf.extend_from_slice(b"null"),
        serde_json::Value::Bool(b) => {
            if *b {
                buf.extend_from_slice(b"true");
            } else {
                buf.extend_from_slice(b"false");
            }
        }
        serde_json::Value::Number(n) => {
            // Must be integer — no floating point allowed
            if let Some(i) = n.as_i64() {
                buf.extend_from_slice(i.to_string().as_bytes());
            } else if let Some(u) = n.as_u64() {
                buf.extend_from_slice(u.to_string().as_bytes());
            } else {
                panic!("Floating-point numbers are forbidden in canonical JSON");
            }
        }
        serde_json::Value::String(s) => {
            write_canonical_string(s, buf);
        }
        serde_json::Value::Array(arr) => {
            buf.push(b'[');
            for (i, item) in arr.iter().enumerate() {
                if i > 0 {
                    buf.push(b',');
                }
                write_canonical_json(item, buf);
            }
            buf.push(b']');
        }
        serde_json::Value::Object(obj) => {
            // Sort keys lexicographically by UTF-8 bytes
            let sorted: BTreeMap<_, _> = obj.iter().collect();
            buf.push(b'{');
            for (i, (key, val)) in sorted.iter().enumerate() {
                if i > 0 {
                    buf.push(b',');
                }
                write_canonical_string(key, buf);
                buf.push(b':');
                write_canonical_json(val, buf);
            }
            buf.push(b'}');
        }
    }
}

fn write_canonical_string(s: &str, buf: &mut Vec<u8>) {
    buf.push(b'"');
    for ch in s.chars() {
        match ch {
            '"' => buf.extend_from_slice(b"\\\""),
            '\\' => buf.extend_from_slice(b"\\\\"),
            '\n' => buf.extend_from_slice(b"\\n"),
            '\r' => buf.extend_from_slice(b"\\r"),
            '\t' => buf.extend_from_slice(b"\\t"),
            c if (c as u32) < 0x20 => {
                // Control characters below U+0020: use \uXXXX
                buf.extend_from_slice(format!("\\u{:04x}", c as u32).as_bytes());
            }
            c => {
                let mut utf8_buf = [0u8; 4];
                buf.extend_from_slice(c.encode_utf8(&mut utf8_buf).as_bytes());
            }
        }
    }
    buf.push(b'"');
}

/// Generate a content-hash node ID per Requirement 13.1:
/// id(v) = "n_" || sha256(kind || ":" || name || ":" || scope_path)[0:12]
pub fn content_hash_node_id(kind: &str, name: &str, scope_path: &str) -> String {
    let input = format!("{}:{}:{}", kind, name, scope_path);
    let hash = digest_bytes(input.as_bytes());
    format!("n_{}", &hash[..12])
}

/// Resolve duplicate IDs by appending _dup1, _dup2, etc.
pub fn resolve_duplicate_ids(ids: &mut Vec<String>) {
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for id in ids.iter_mut() {
        let count = seen.entry(id.clone()).or_insert(0);
        if *count > 0 {
            *id = format!("{}_dup{}", id, count);
        }
        *count += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_canonical_json_object_sorted_keys() {
        let val = json!({"z": 1, "a": 2, "m": 3});
        let result = String::from_utf8(canonical_json(&val)).unwrap();
        assert_eq!(result, r#"{"a":2,"m":3,"z":1}"#);
    }

    #[test]
    fn test_canonical_json_no_whitespace() {
        let val = json!({"key": [1, 2, 3]});
        let result = String::from_utf8(canonical_json(&val)).unwrap();
        assert_eq!(result, r#"{"key":[1,2,3]}"#);
    }

    #[test]
    fn test_canonical_json_string_escaping() {
        let val = json!({"msg": "hello\nworld"});
        let result = String::from_utf8(canonical_json(&val)).unwrap();
        assert_eq!(result, r#"{"msg":"hello\nworld"}"#);
    }

    #[test]
    fn test_canonical_json_unicode_string() {
        let val = json!({"name": "नमस्ते"});
        let result = String::from_utf8(canonical_json(&val)).unwrap();
        assert!(result.contains("नमस्ते"));
    }

    #[test]
    fn test_digest_bytes() {
        let d = digest_bytes(b"hello");
        assert_eq!(d.len(), 64);
        assert_eq!(
            d,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn test_content_hash_node_id() {
        let id = content_hash_node_id("Function", "main", "/");
        assert!(id.starts_with("n_"));
        assert_eq!(id.len(), 14); // "n_" + 12 hex chars
    }

    #[test]
    fn test_resolve_duplicate_ids() {
        let mut ids = vec!["n_abc".to_string(), "n_abc".to_string(), "n_abc".to_string()];
        resolve_duplicate_ids(&mut ids);
        assert_eq!(ids, vec!["n_abc", "n_abc_dup1", "n_abc_dup2"]);
    }

    #[test]
    #[should_panic(expected = "Floating-point")]
    fn test_canonical_json_rejects_float() {
        let val = json!(1.5);
        canonical_json(&val);
    }

    #[test]
    fn test_canonical_json_null() {
        assert_eq!(
            String::from_utf8(canonical_json(&json!(null))).unwrap(),
            "null"
        );
    }

    #[test]
    fn test_canonical_json_bool() {
        assert_eq!(
            String::from_utf8(canonical_json(&json!(true))).unwrap(),
            "true"
        );
    }
}
