//! **The bevy_blueprint MCP server**, as a library.
//!
//! The binary is the server; this is the half the tests read. **The tool declarations live here**
//! so that a test can check the reference against them without parsing source text — a test that
//! parses source text is a test of the source text, and it passes when the text is right and the
//! server is wrong.
//!
//! # What it is
//!
//! An MCP server for the [bevy_blueprint](https://github.com/dark-incorporated/bevy_dark_factory) world editor. It
//! speaks the Model Context Protocol over **stdio** and reaches the running editor over the Bevy
//! Remote Protocol, so an agent can drive the editor through typed operations instead of
//! synthesising keystrokes.

pub mod brp;
pub mod decide;
pub mod docs;
pub mod sequence;

/// **How fast `blueprint_decide` answers, in the words the tool's description uses** (#456, #457).
///
/// **The one place the latency is written**: the server's tool description (`main.rs`) and the reference's
/// copy of it (`docs.rs`) both build their sentence from this. A macro, not a `const`, because an attribute's
/// description is assembled with `concat!`, which takes literals. Then regenerate `docs/mcp-tools.md`
/// (`cargo run -q -p dark_factory_mcp -- --emit-docs > docs/mcp-tools.md`).
///
/// **What the figure covers, and what it does not.** `latency_ms` times `Transport::send` and nothing else
/// (`decide::Measured`): the decision model's round trips. It does **not** include the editor taking the
/// still, so it is not what an agent waits. Measured 2026-11-21 on big against the local `d3-flash`, five calls
/// per cell with a **different `state` every call** (llama-server caches prompts, so a repeated body measures
/// the cache):
///
/// | the model, one call | text only | with a 27 kB still |
/// |---|---|---|
/// | 1 question | 36 ms | 76 ms |
/// | 3 questions | 107 ms | 203 ms |
/// | 4 questions | 150 ms | 261 ms |
/// | 8 questions | 320 ms | 597 ms |
///
/// The repo's own three-question fixture (`crates/bevy_oracle/fixtures/request.json` with its 164 kB image)
/// re-measured at **613 ms** beside a running benchmark arm, against the 397 ms
/// `docs/decide/serving-d3-flash.md` recorded on an idle card: the difference is contention, and it is why the
/// sentence below is rounded rather than a promise.
///
/// **What an agent waits is longer and is unmeasured.** The editor's capture is the part nobody has timed, so
/// the description says the model's number and names the rest as unmeasured instead of blaming a component.
///
/// **A cold call is seconds**: 5.34 s for the first request after llama-swap has unloaded the model. That is
/// `ttl`, not the tool; the benchmark's keep-alive exists for it.
#[macro_export]
macro_rules! decide_latency {
    () => {
        "in a fraction of a second for the model itself (about 150 ms for four questions without a picture, 260 ms with one; what you wait also includes the still being taken, which is not counted here)"
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
