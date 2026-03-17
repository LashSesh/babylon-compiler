# Babylon Compiler
### A deterministic compiler that transforms source code into **Crystals** - cryptographically certified knowledge artifacts. Built in Rust.
**Ref.: [Glyph Foundry Framework](glyph_foundry_spec.pdf)**

Babylon compiles source code through an 8-stage pipeline — parsing, canonicalization, embedding, convergence tracking, obligation gating, and tamper-evident chain recording — to produce fully verifiable, reproducible bundles. Every computation uses fixed-point arithmetic (Q16) and canonical JSON (RFC 8785) to guarantee bitwise determinism across platforms.

## Pipeline

```
Source ─→ S0 Ingest ─→ S1 Parse & Canonicalize ─→ S2 Expand ─→ S3 Embed
                                                                    │
  S7 Emit ←── S6 MEF ←── S5 Gate ←── S4 Crystallize ←──────────────┘
    │
    └─→ Bundle (manifest + all artifacts, digests, traces)
```

| Stage | Name | Purpose |
|-------|------|---------|
| S0 | **Ingest** | Read source bytes, normalize Unicode (NFC/NFD), reject forbidden ranges |
| S1 | **Parse & Canonicalize** | Parse with language frontend, lower to IR, canonicalize |
| S2 | **Expand** | Apply macro expansions from the macro registry |
| S3 | **Embed** | Compute 5-axis H5 embedding, fingerprint, and genome ID |
| S4 | **Crystallize** | Construct a Crystal when TIC reaches convergence |
| S5 | **Gate** | Evaluate obligations, run Proof-of-Resonance FSM |
| S6 | **MEF** | Record operator evidence in an append-only hash chain |
| S7 | **Emit** | Assemble bundle with manifest, artifacts, and digests |

## Crates

| Crate | Description |
|-------|-------------|
| `glyph-q16` | Q16 fixed-point arithmetic (signed 32-bit, 1/65536 resolution) |
| `glyph-canon` | Canonical JSON serialization (RFC 8785 / JCS) and SHA-256 hashing |
| `glyph-ir` | Intermediate representation — nodes, edges, documents |
| `glyph-rd` | Run Descriptor configuration and policy binding |
| `glyph-registry` | Operator, macro, obligation, and observable registries |
| `glyph-frontends` | Source language frontends (Sanskroot, HanLan) |
| `glyph-embed` | H5 embedding space — 5-axis structural analysis |
| `glyph-tic` | Temporal Information Certificate — convergence tracking |
| `glyph-crystal` | Crystal construction and hyper-embedding |
| `glyph-mef` | Morphogenetic Evidence Framework — tamper-evident hash chain |
| `glyph-gate` | Obligation evaluation and Proof-of-Resonance state machine |
| `glyph-expand` | Macro expansion engine with precedence resolution |
| `glyph-bundle` | Bundle assembly and atomic emission |
| `glyph-verify` | Bundle verification — digest validation and MEF chain integrity |
| `glyph-cli` | Command-line interface |

## Usage

```bash
# Build
cargo build --release

# Initialize a Run Descriptor
glyph-cli init-rd -o run.json

# Compile a source file through the full pipeline
glyph-cli run --rd run.json --source input.sk --out-dir ./output

# Verify a bundle
glyph-cli verify --bundle ./output

# Dump the IR for inspection
glyph-cli dump-ir --rd run.json --source input.sk

# Compute embedding only
glyph-cli embed --rd run.json --source input.sk
```

## Sanskroot

The default frontend parses **Sanskroot**, a language using Devanagari script:

```
कार्य मुख्य() {
    मान x = 42;
    यदि (x) {
        दर्शय("सत्य");
    } अन्यथा {
        दर्शय("असत्य");
    }
    निवृत्ति x;
}
```

| Keyword | Meaning |
|---------|---------|
| `कार्य` | function |
| `मान` | let / assign |
| `यदि` | if |
| `अन्यथा` | else |
| `दर्शय` | print |
| `निवृत्ति` | return |
| `सत्य` / `असत्य` | true / false |

## HanLan (CN++)

A second frontend using Chinese characters. File extension: `.hnln`, frontend ID: `hanlan`.

```
函 主() {
    令 x = 42;
    若 (x > 0) {
        示("正");
    } 否则 {
        示("非正");
    }
    返 x;
}
```

| Keyword | Meaning |
|---------|---------|
| `函` | function |
| `令` | let / assign |
| `若` | if |
| `否则` | else |
| `示` | print |
| `返` | return |
| `真` / `假` | true / false |

HanLan also accepts Sanskroot keywords as Tier-1 aliases for mixed-script compatibility.

```bash
glyph-cli run --frontend hanlan --rd run.json -i input.hnln -o ./output
```

## Design Principles

- **Determinism** — No floating-point in identity-bearing computations. All math uses Q16 fixed-point.
- **Canonicalization** — Every artifact is serialized to RFC 8785 canonical JSON before hashing.
- **Content-addressed identity** — Node IDs, graph IDs, crystal IDs are SHA-256 digests of their canonical content.
- **Tamper evidence** — The MEF chain links blocks by hash; verification recomputes the full chain.
- **Extensible frontends** — New source languages plug in by implementing the frontend trait and lowering to the shared IR.

## Building from Source

Requires Rust 1.70+.

```bash
cargo build
cargo test
```

## License

See [LICENSE](LICENSE) for details.
