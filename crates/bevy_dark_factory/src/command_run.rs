//! **Run a registered command by id, through the key that runs it** (UI-90, issue #256).
//!
//! `blueprint/run_command { id }` is the agent's door onto [`crate::commands`]. The id names an
//! [`crate::keys::Action`]; this module presses that action's chord, one frame, as the keyboard
//! would, and lets the dispatcher that every key goes through answer. Nothing here knows what
//! `edit.undo` does, so there is no second dispatcher to drift from the first: a command that the
//! key would refuse in this stance is refused here with the registry's own reason
//! ([`crate::commands::Command::enabled`]), and one that fires fires through the same system, with the
//! same side effects (the undo stack, the receipt, the status line), as the key.
//!
//! **Why a key and not a function call.** The verbs live in systems that take a dozen resources each
//! (`editor.rs`), because they are written against a frame's input. Rebuilding that context in a
//! second place is how two front ends come to disagree; pressing the chord reuses the one place it is
//! built. The cost is two frames: the chord goes down in one frame's `PreUpdate` and up in the next, so
//! the answer says `ran` when the press was queued, and a caller reads `blueprint/state` for the result.
//!
//! **A chord is held one frame, then let go.** The modifier and the key go down together and come up
//! together, so the key census's `needs_mod` and `needs_shift` see exactly the keys a person holds.

use bevy::input::ButtonState;
use bevy::input::InputSystems;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::prelude::*;

use bevy_gooey::{Modifiers, Shortcut};

use crate::commands::Command;
use crate::keys::{self, Binding};
use crate::ops::Source;

/// The chords waiting to go down, and the keys that are down for this frame's input.
#[derive(Resource, Default, Debug)]
pub struct Chords {
    /// Chords asked for and not yet pressed, oldest first. One goes down per two frames, **each with the [`Source`] that asked for it**.
    ///
    /// **The source is carried, not guessed** (DL-02, #470): a chord `blueprint/run_command` queued is an agent's and one a palette row, a menu row
    /// or an empty-state prompt queued is the person's, and the frame it goes down in is labelled from here.
    waiting: Vec<(Vec<KeyCode>, Source)>,
    /// The keys pressed last frame, released at the start of this one.
    down: Vec<KeyCode>,
}

impl Chords {
    /// Ask for a command's chord to be pressed: the one the settings gave it ([`keys::Rebound`], in
    /// `live`), or else the census's. `source` says whose hands asked for it.
    pub fn queue_from(&mut self, command: &Command, live: keys::Live, source: Source) {
        self.waiting.push((
            match live.3.chord(command.action) {
                Some(chord) => keys_of_chord(chord),
                None => keys_of(keys::binding(command.action)),
            },
            source,
        ));
    }

    /// **Ask for a fixed chord to be pressed**, for `blueprint/run_command` to reach the controls that
    /// are not census rows (#668). `fixed` names one of [`crate::chords::CHORDS`]; the keys are that
    /// chord's own, read now, and the press goes through the same queue every command's does — so a
    /// chord an agent asks for and a chord a person presses are the same key sequence in the same
    /// frame, and there is no second dispatch to drift from the first.
    pub fn queue_fixed(&mut self, fixed: crate::chords::Fixed, source: Source) {
        self.waiting.push((keys_of_chord(crate::chords::chord(fixed)), source));
    }

    /// The same, for the person's own hands: every caller but an agent's door is a person's click.
    pub fn queue(&mut self, command: &Command, live: keys::Live) {
        self.queue_from(command, live, Source::Human);
    }

    /// How many chords have not gone down yet.
    pub fn waiting(&self) -> usize {
        self.waiting.len()
    }
}

/// **The keys a binding asks of the keyboard**, modifiers first so the key lands with them held.
/// A row that does not ask for Shift is pressed without it: `Cmd+Z` undoes, `Shift+Cmd+Z` redoes. Alt likewise.
fn keys_of(binding: &Binding) -> Vec<KeyCode> {
    let mut chord = Vec::with_capacity(4);
    if binding.needs_mod {
        chord.push(keys::MOD_KEYS[0]);
    }
    if binding.needs_shift == Some(true) {
        chord.push(KeyCode::ShiftLeft);
    }
    if binding.needs_alt {
        chord.push(keys::ALT_KEYS[0]);
    }
    chord.push(binding.key);
    chord
}

/// **The keys a rebound chord asks of the keyboard**, exactly the modifiers it names.
fn keys_of_chord(shortcut: Shortcut) -> Vec<KeyCode> {
    let Modifiers { ctrl, shift, alt, meta } = shortcut.modifiers;
    let mut chord = Vec::with_capacity(5);
    for (held, key) in [
        (ctrl, KeyCode::ControlLeft),
        (shift, KeyCode::ShiftLeft),
        (alt, KeyCode::AltLeft),
        (meta, KeyCode::SuperLeft),
    ] {
        if held {
            chord.push(key);
        }
    }
    chord.push(shortcut.key);
    chord
}

fn message(key: KeyCode, state: ButtonState) -> KeyboardInput {
    KeyboardInput {
        key_code: key,
        logical_key: Key::Unidentified(NativeKey::Unidentified),
        state,
        text: None,
        repeat: false,
        window: Entity::PLACEHOLDER,
    }
}

/// **Let go of last frame's chord, then press the next waiting one.** Runs before the keyboard system
/// reads its messages, so a press written here is `just_pressed` in this frame's `Update`, and the
/// release written next frame is read before that frame's `Update`.
///
/// A chord is never pressed in the frame the last one came up: the key census must see the released
/// state once, or two chords on one key would read as one held press.
///
/// **The frame a chord goes down in carries its hands** (DL-02, #470): `blueprint/run_command` queues one as an agent's, so a key this system presses
/// for an agent is an agent's input and an answer it produces is no verdict of the person's.
pub fn press_waiting(mut chords: ResMut<Chords>, mut keyboard: MessageWriter<KeyboardInput>, mut hands: Option<ResMut<crate::hands::Hands>>) {
    if !chords.down.is_empty() {
        for key in std::mem::take(&mut chords.down).into_iter().rev() {
            keyboard.write(message(key, ButtonState::Released));
        }
        return;
    }
    if chords.waiting.is_empty() {
        return;
    }
    let (chord, source) = chords.waiting.remove(0);
    if source == Source::Agent
        && let Some(hands) = hands.as_mut()
    {
        hands.say_an_agent();
    }
    for key in &chord {
        keyboard.write(message(*key, ButtonState::Pressed));
    }
    chords.down = chord;
}

/// The plugin: the queue and the system that presses it.
pub struct CommandRunPlugin;

impl Plugin for CommandRunPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Chords>()
            .add_systems(PreUpdate, press_waiting.before(InputSystems).after(crate::hands::count_the_frame));
    }
}
