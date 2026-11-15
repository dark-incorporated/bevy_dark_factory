# bevy_oracle

> ⚠️ **Vibe Coded.** Written by AI agents working from a human's direction, covered by tests rather than a line-by-line human review. Read the code before you rely on it for something you care about.

Typed questions to a decision model, for tooling that checks pictures and facts at authoring time: yes/no, choice and score questions about a piece of state and up to four images, answered with a probability for every option. A decision model (Cloudflare's Clef, and its small variant clef-flash) generates no text, so there is nothing to parse and no answer to talk it into.

**Status:** experimental

**Why:** a vision model asked to describe a screenshot gives prose that must be judged in turn; asked a typed question it gives a number a gate can compare with a bar, and a calibration can say how far to trust it. The crate never answers for a model that did not answer, and its tests never call a live one.

## Quick start

`bevy_oracle` is not on crates.io; take it from the repository:

```sh
cargo add bevy_oracle --git https://github.com/dark-incorporated/bevy_dark_factory
```

```rust,no_run
use bevy_oracle::{DecisionModel, Endpoints, Http, Question, Request, ask};

fn main() -> Result<(), bevy_oracle::Error> {
    let request = Request::new("The bedroom map, after the bed was removed.")
        .with_question("bed", Question::noul("Is there a bed in the room?"))
        .with_question(
            "mood",
            Question::choice(
                "Which word fits the room best?",
                [("cosy", "lived in and warm"), ("bare", "empty and unfinished")],
            ),
        );

    // One endpoint per run, chosen by the person's decision-model setting (Local or Remote).
    let config = Endpoints::from_env().config(DecisionModel::Local)?;
    let response = ask(&Http::new(config), &request)?;

    let p_bed = response.answers["bed"].p_yes();
    println!("P(bed) = {p_bed:?}");
    Ok(())
}
```

`Request::with_image` takes a URL or a data URL, and `bevy_oracle::data_url(png_bytes)` makes the latter. A request is checked against the API's limits before anything is sent: 1 to 64 questions, at most 4 images, and at least two options in a choice or two levels in a score.

## What is in it

- **Three question types**: `Question::noul` (yes or no; the answer is P(yes)), `Question::choice` (named options; the answer is each option's probability) and `Question::score` (levels on a scale; the answer is a probability-weighted level).
- **Transports**: `Http` for a live endpoint, `Replay` for a recorded exchange (it answers only the request it was recorded for, never a different one) and `Recorder`, which wraps a live transport and writes the exchange to a folder as `request.json` and `response.json`. The tests replay; a live call happens only in calibration runs and smoke checks.
- **One endpoint, no fallback.** A wrong endpoint or a refused request is an `Error` naming the endpoint, never a default answer. The endpoint is `GRIMOIRE_DECIDE_ENDPOINT` (default `http://127.0.0.1:8093/v1/systemone`, clef-flash on this LAN), with `GRIMOIRE_DECIDE_MODEL`, `GRIMOIRE_DECIDE_KEY` and `GRIMOIRE_DECIDE_TIMEOUT_SECS` beside it. Cloudflare sends screenshots off the local network, so it is used only when you set those variables.
- **Checks** (`Checks`, `Check`, `run_checks`): named questions with a threshold, from a built-in `checks.yaml` and a project's own. A check may fail a gate only once it has been calibrated against a labelled set (`calibrate`, `Calibration`), and it does not answer from less evidence than it needs.
- **`decide_check`**, a binary that runs the checks over a folder of captures. Without `--ask` nothing leaves the machine: it lists each check with the evidence it would send and its calibration.

```sh
cargo run -p bevy_oracle --bin decide_check -- --captures docs/changelog/media --project .
```

The crate is authoring time only: nothing here ships in a game, and it has no Bevy dependency despite the name.

## Learn more

- [Checking captures with the decision model](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/docs/decide/checking-captures.md) and [serving clef-flash](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/docs/decide/serving-clef-flash.md).
- The editor's MCP server exposes this to an agent as `grimoire_decide`: see [the MCP tool reference](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/docs/mcp-tools.md).
- [Changelog](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/CHANGELOG.md) of the repository this crate lives in.
- Part of Dark Inc.: the [family table](https://github.com/dark-incorporated) lists the other crates, and [the book page for this repository](https://dark-incorporated.github.io/crates/bevy_grimoire.html) says how they fit together.

## Who and licence

Maintained by [@Ladvien](https://github.com/Ladvien). Issues go to the [issue tracker](https://github.com/dark-incorporated/bevy_dark_factory/issues); see the organisation's [`CONTRIBUTING.md`](https://github.com/dark-incorporated/.github/blob/main/CONTRIBUTING.md).

MIT OR Apache-2.0, at your option: [`LICENSE-MIT`](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/LICENSE-MIT) and [`LICENSE-APACHE`](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/LICENSE-APACHE).
