//! **Whose hands pressed this frame's input** — the person at the keyboard, or an agent over BRP.
//!
//! # Why this exists (DL-02, #470)
//!
//! The turn ledger records what a person made of an agent's work as a **gold** label, and the series is explicit that only a person's own action may
//! be gold. It could not tell whose hands pressed: an answer reached `present::answer_presentation` as a modal answer (`Confirm::answer`) or a
//! `PresentationReply`, and both are set from key and pointer input with no source attached. So an agent that put its own result in front of the
//! person and then pressed **Keep** through `bevy_fairy/input`'s injected cursor had its own act recorded as the person's verdict, training a
//! student on a judgement nobody made.
//!
//! # Two writers, both exact choke points
//!
//! Only two paths in this editor can put an agent's act into the input the interface answers, and each says so here:
//!
//! - **`surface::inject_clicks`** is the bridge that turns an agent's cursor press into picking's `PointerInput`, and it writes nothing unless
//!   `bevy_debugger_bevy::DebugCursor` is out — the crate's own rule for who owns the pointer (*"Precedence, not fallback"*). A person's click never
//!   passes through it.
//! - **`command_run::Chords`** carries the [`Source`] of whoever queued a chord, so the frame `grimoire/run_command` presses a key in is the agent's
//!   and a palette row's, a menu row's or an empty-state prompt's is the person's.
//!
//! # Why a mark lasts two frames
//!
//! **A click is not one frame.** The press is forwarded in `PreUpdate`; `bevy_picking` turns it into a pointer event, and the widget's `Activate`
//! — which is what sets the answer — can arrive in the frame after. A mark read only in the frame it was made would therefore call the agent's own
//! click the person's, which is precisely the bug this file exists to stop. So a mark is honoured in the frame it was made **and the one after**.
//!
//! The cost is the other way round and deliberately so: a person answering within one frame of an agent's click is read as the agent, so a gold
//! label is **missed**, never invented. A missing label is a smaller loss than a fabricated one, and no test can tell a person's reflex from a
//! scripted press that close together.
//!
//! # The limit, stated
//!
//! **An injected key with no injected cursor** cannot be told from a person's at this `bevy_fairy` pin (0.3.1): the crate writes the same
//! `KeyboardInput` message for both and its queue exposes no count of what it applied. An agent's documented way to drive this editor is the
//! cursor — `tools/demo/record.py` and every debug driver aim one before acting — so the remaining hole is a key-only injection, recorded on #470.

use bevy::input::InputSystems;
use bevy::prelude::*;

use crate::ops::Source;

/// **Whose hands are on this frame's input.** Read by whatever turns input into a record of the person.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct Hands {
    /// The frame an agent last put something into the input, by [`Hands::frame`]'s count.
    agent_at: Option<u64>,
    /// Frames seen, so a mark can expire without a clock and without a second resource.
    frame: u64,
}

impl Hands {
    /// Whether the person at the keyboard did this, which is the only source a gold label may come from.
    pub fn are_the_persons(&self) -> bool {
        self.source() == Source::Human
    }

    /// Whose hands this frame's input was: the agent's for the frame of its mark and the one after, the person's otherwise.
    pub fn source(&self) -> Source {
        match self.agent_at {
            Some(at) if self.frame.saturating_sub(at) <= 1 => Source::Agent,
            _ => Source::Human,
        }
    }

    /// An agent drove this frame: its pointer forwarded a press, or its chord went down.
    pub fn say_an_agent(&mut self) {
        self.agent_at = Some(self.frame);
    }
}

/// **A frame begins**, counted here so [`Hands`] needs no clock and no second writer of its count.
///
/// Ahead of [`InputSystems`] and of [`crate::command_run::press_waiting`], so both agent paths mark *after* the count moves and their mark is read
/// against the frame it happened in.
pub fn count_the_frame(mut hands: ResMut<Hands>) {
    hands.frame += 1;
}

/// Registers [`Hands`] and its frame counter.
pub struct HandsPlugin;

impl Plugin for HandsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Hands>().add_systems(
            PreUpdate,
            count_the_frame
                .before(InputSystems)
                .before(crate::command_run::press_waiting),
        );
    }
}
