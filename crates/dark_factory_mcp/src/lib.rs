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

/// **How fast `blueprint_decide` answers, in the words the tool's description uses** (#456).
///
/// **The one place the latency is written**, so the figure DM-01 (#452) measures on the local model is
/// changed here and nowhere else: the server's tool description (`main.rs`) and the reference's copy of it
/// (`docs.rs`) both build their sentence from this. A macro, not a `const`, because an attribute's
/// description is assembled with `concat!`, which takes literals. Then regenerate `docs/mcp-tools.md`
/// (`cargo run -q -p dark_factory_mcp -- --emit-docs > docs/mcp-tools.md`).
#[macro_export]
macro_rules! decide_latency {
    () => {
        "in about a second"
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
