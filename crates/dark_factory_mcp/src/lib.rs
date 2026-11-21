//! **The bevy_blueprint MCP server**, as a library.
//!
//! The binary is the server; this is the half the tests read. **The tool declarations live here**
//! so that a test can check the reference against them without parsing source text — a test that
//! parses source text is a test of the source text, and it passes when the text is right and the
//! server is wrong.
//!
//! # What it is
//!
//! An MCP server for the [bevy_blueprint](https://github.com/bevy-coven/bevy_blueprint) world editor. It
//! speaks the Model Context Protocol over **stdio** and reaches the running editor over the Bevy
//! Remote Protocol, so an agent can drive the editor through typed operations instead of
//! synthesising keystrokes.

pub mod brp;
pub mod decide;
pub mod docs;
pub mod sequence;

/// **How fast `blueprint_decide` answers, in the words the tool's description uses** (#456, #457).
///
/// **The one place the latency is written**, so the figure measured on the local model is changed here and
/// nowhere else: the server's tool description (`main.rs`) and the reference's copy of it (`docs.rs`) both
/// build their sentence from this. A macro, not a `const`, because an attribute's description is assembled
/// with `concat!`, which takes literals. Then regenerate `docs/mcp-tools.md`
/// (`cargo run -q -p dark_factory_mcp -- --emit-docs > docs/mcp-tools.md`).
///
/// **Three figures, because there are three things to buy, and which one you get is the design question.**
/// Measured on big against the local `d3-flash`, four `noul` questions in every case:
///
/// | the call | median | n |
/// |---|---|---|
/// | the model alone, text only, no picture | **151 ms** | 25 |
/// | the model alone, one 27 kB still attached | **279 ms** | 25 |
/// | `blueprint_decide` as an agent actually calls it (`view: true`, so the editor also takes the still) | **1495 ms** | 5 |
///
/// **The number that matters is the third**, and it is the one an agent waits for: a trial's editor takes the
/// still, encodes it, and the server reads the checks, before the model is asked anything. Measured inside a
/// live benchmark run on 2026-11-20, every call with `view: true`: median 1495 ms, max 1848. The 2026-11-15
/// benchmark found a median of **543 ms** for the same thing, so the editor's capture is the expensive half
/// and it varies with what is on screen — which is why the figure below is the rounded "about a second and a
/// half" and not a millisecond count that would read as a promise the capture cannot keep.
///
/// **A cold call is seconds, not milliseconds**: the first request after llama-swap has unloaded the model
/// pays its load (5.3 s measured). That is `ttl`, not the tool; the benchmark's keep-alive exists for it.
#[macro_export]
macro_rules! decide_latency {
    () => {
        "in about a second and a half (the model itself answers in 150-280 ms; the editor taking the still is the rest)"
    };
}

/// **The version, for the server's own handshake.**
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// **A stable number for a picture's contents** — FNV-1a, 64-bit, as lowercase hex.
///
/// Sixteen hex digits, so it reads as one token in an answer and two answers differ at a glance when
/// the pixels moved. Chosen over a cryptographic hash because the question is equality, not
/// tampering, and over a hand-rolled checksum because FNV-1a is a published construction with no
/// tuning to get wrong (Fowler, Noll & Vo, *FNV*, 1991). No dependency is taken for it.
///
/// **Here and not in the binary**, so the test that pins its properties calls *this* function
/// rather than a restatement of it. A test of a copy passes when the copy is right and the server is
/// wrong — the failure mode this crate's `lib.rs` exists to avoid (see its module note).
///
/// Published vectors, which `the_fingerprint_matches_the_published_vectors` checks: `""` is
/// `cbf29ce484222325`, `"a"` is `af63dc4c8601ec8c`, `"foobar"` is `85944171f73967e8`, and
/// `"hello"` is `a430d84680aabd0b`.
pub fn fingerprint(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}
