//! **A frame profile a person can open** (#504).
//!
//! # The gap this closes
//!
//! `ui_budget` times each content-following system into [`crate::ui_budget::UiFrame`] and holds a settled frame to
//! [`crate::ui_budget::MAX_MS_PER_FRAME`]. That is a **test gate**, not a surface: `UiFrame` reaches exactly three
//! instrumented systems and nothing else. Whole-frame numbers exist only over BRP — `ops_brp::frame_json`'s `frame_ms`,
//! `fps`, `busy_ms` — which is an *agent's* readout. So the only way to answer "why is it slow" was to attach an MCP
//! client, and the question the budget asks (2 ms) is answerable while "the other 15 ms" is not.
//!
//! # Cheap while off, and the one thing that is not
//!
//! **`trace_tracy` is out of the question**: it is a Bevy feature, not a plugin, and would need the whole workspace
//! rebuilt. What is here is a **self-hosted span ring**: [`HISTORY_FRAMES`] frames of `(system, ms)`, collected from the
//! systems `ui_budget` already times, plus the panel that lists them.
//!
//! **Collecting spans is gated on one relaxed atomic load.** [`record_span`] is called by every instrumented system on
//! every frame, and with recording off it loads [`RECORDING`], finds it false and returns — no lock, no allocation, no
//! formatting. That is Renzora's *"one atomic load a frame, and nothing else"*.
//!
//! **The frame clock is not gated, and that is deliberate.** `blueprint/state.frame.frame_ms` and the panel must report
//! the same number for the same frame — a person and an agent disagreeing about how long a frame took is the defect this
//! issue is about. A number that only existed while recording was on would make them disagree exactly when somebody
//! looked, so the frame is always timed (two `Instant::now()` calls) and only the *spans* are gated.
//!
//! # The history is bounded, and the mistake it avoids
//!
//! [`HISTORY_FRAMES`] is 120 — about two seconds at 60 fps, Renzora's own number and its own measured reason: at an
//! editor's load a frame is about 5,500 spans, and six hundred of them were 105 MB of memory nothing read. So the ring is
//! a `VecDeque` trimmed from the front, [`Profiler::dropped`] says how many fell off, and a frame's span list is only
//! ever as long as the number of instrumented systems — a `Vec<ProfileFrame>` grown per frame would be the same mistake.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex, MutexGuard};
use std::time::Instant;

use bevy::prelude::*;

use crate::chrome;

/// **How many frames the ring keeps.** About two seconds at 60 fps: long enough to see a stutter, short enough that the
/// ring is not a second copy of the frame.
pub const HISTORY_FRAMES: usize = 120;

/// **The chord that shows and hides the profile panel.** `Cmd`/`Ctrl`+`Shift`+`D` is bound in no context, is not one of
/// the chords `keymap_screen` reserves (palette, terminal, settings, interface scale, the console), and `D` already means
/// "move" here, so the modifier carries the same word.
pub fn toggle_chord() -> bevy_gooey::Shortcut {
    bevy_gooey::Shortcut::new(bevy_gooey::Modifiers::PRIMARY | bevy_gooey::Modifiers::SHIFT, KeyCode::KeyD)
}

/// **Whether spans are being collected.** One relaxed load is the whole cost of an instrumented system with recording
/// off; it is a `static` rather than a field because [`record_span`] is called from inside `ui_budget`'s guard, which
/// holds a resource borrow and has no world to reach a second one through.
static RECORDING: AtomicBool = AtomicBool::new(false);

/// **Where the spans of the frame being run are put.** A `static` for [`record_span`]'s reason. Only touched while
/// [`RECORDING`] is on.
static PENDING: LazyLock<Mutex<Vec<Span>>> = LazyLock::new(|| Mutex::new(Vec::new()));

/// The spans recorded so far this frame, locked. A poisoned lock means a panic happened while profiling, and the spans
/// are still worth having: `log_file::OneSink` makes the same call for the same reason.
fn pending() -> MutexGuard<'static, Vec<Span>> {
    PENDING.lock().unwrap_or_else(|e| e.into_inner())
}

/// One instrumented system's cost this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Span {
    /// The system's name, as [`crate::ui_budget::measure`] was given it.
    pub system: &'static str,
    pub ms: f64,
}

/// One finished frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Frame {
    /// The frame's number, from Bevy's own counter, so a person and an agent name the same frame.
    pub number: u64,
    /// **Wall time from the head of the frame to its foot**, the number the panel and `blueprint/state.frame.frame_ms`
    /// both report.
    pub ms: f64,
    /// **The instrumented systems' spans**, heaviest first. Empty when recording was off for this frame.
    pub spans: Vec<Span>,
}

impl Frame {
    /// **The system that took the most time**, or `None` on a frame with no spans (recording was off).
    pub fn heaviest(&self) -> Option<&Span> {
        self.spans.first()
    }

    /// What the spans add up to, which is [`crate::ui_budget::UiFrame::last_ms`] for the same frame — the check that the
    /// two measurements of one thing agree.
    pub fn measured_ms(&self) -> f64 {
        self.spans.iter().map(|span| span.ms).sum()
    }
}

/// **Record one system's cost**, called by [`crate::ui_budget`]'s guard as it drops.
pub fn record_span(system: &'static str, ms: f64) {
    // **The one atomic load.** With recording off nothing below runs: no lock, no allocation, no formatting.
    if !RECORDING.load(Ordering::Relaxed) {
        return;
    }
    pending().push(Span { system, ms });
}

/// **The frame profile the editor holds**: the ring, whether it is recording, and the frame in flight.
#[derive(Resource, Default)]
pub struct Profiler {
    frames: VecDeque<Frame>,
    dropped: u64,
    /// **Whether spans are collected.** Written here and published to [`RECORDING`] by the same call, so the panel and
    /// the systems cannot disagree about whether they are being measured.
    recording: bool,
    began: Option<Instant>,
}

impl Profiler {
    /// The frames held, oldest first. **Double-ended**, because the useful questions count back from the newest — the
    /// test that holds the panel's total against `UiFrame`'s is the second-newest against the budget's newest.
    pub fn frames(&self) -> impl DoubleEndedIterator<Item = &Frame> {
        self.frames.iter()
    }

    /// How many frames are held now.
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    /// Whether no frame has been closed yet.
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// How many frames fell off the front at [`HISTORY_FRAMES`].
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// **The frame that finished**, which is what the panel draws and `blueprint/state.frame` reports.
    pub fn last(&self) -> Option<&Frame> {
        self.frames.back()
    }

    /// The frame that finished, as the panel and the agent both read it: `frame_ms`.
    pub fn last_ms(&self) -> Option<f64> {
        self.last().map(|frame| frame.ms)
    }

    /// Whether spans are being collected.
    pub fn recording(&self) -> bool {
        self.recording
    }

    /// **Start or stop collecting spans**, publishing the flag [`record_span`] reads.
    pub fn set_recording(&mut self, on: bool) {
        self.recording = on;
        RECORDING.store(on, Ordering::Relaxed);
        if !on {
            // **A half-collected frame is dropped**, so the ring never holds a frame whose span list is the three
            // systems that happened to run after the switch.
            pending().clear();
        }
    }

    /// **The heaviest frame in the ring**, which is what "why is it slow" is asking for.
    pub fn heaviest_frame(&self) -> Option<&Frame> {
        self.frames.iter().max_by(|a, b| a.ms.total_cmp(&b.ms))
    }
}

/// **The head of the frame.** The clock is kept whether or not spans are collected — see the module note.
fn begin_the_frame(mut profiler: ResMut<Profiler>) {
    profiler.began = Some(Instant::now());
}

/// **The foot of the frame**: time it, take what the instrumented systems recorded, and keep it in the ring.
fn close_the_frame(mut profiler: ResMut<Profiler>, frames: Res<bevy::diagnostic::FrameCount>) {
    let Some(began) = profiler.began.take() else { return };
    let ms = began.elapsed().as_secs_f64() * 1000.0;
    let mut spans = if RECORDING.load(Ordering::Relaxed) {
        let mut spans = std::mem::take(&mut *pending());
        spans.sort_by(|a, b| b.ms.total_cmp(&a.ms));
        spans
    } else {
        Vec::new()
    };
    // **A frame keeps the spans the panel lists and nothing else**: a `Vec<Span>` as long as the instrumented system list,
    // which is three today, is not a second copy of the frame.
    spans.shrink_to_fit();
    profiler.frames.push_back(Frame { number: u64::from(frames.0), ms, spans });
    while profiler.frames.len() > HISTORY_FRAMES {
        profiler.frames.pop_front();
        profiler.dropped += 1;
    }
}

// ── The panel ────────────────────────────────────────────────────────────────────────────────────

/// The profile panel's overlay, so a test can find it and read its `display`.
#[derive(Component)]
pub struct ProfileOverlay;

/// The tally line: the frame's number, its time, and how many fell off.
#[derive(Component)]
pub struct ProfileTally;

/// The column the heaviest spans are listed into.
#[derive(Component)]
pub struct ProfileSpans;

/// **The profile in the window** — an overlay over the viewport, hidden until its chord, the shape the journal and the
/// console both use.
fn spawn_the_profile(mut commands: Commands, frame: Res<chrome::Frame>) {
    let overlay = commands
        .spawn((
            bevy_gooey::placed(UiRect { left: Val::Percent(30.0), top: Val::Percent(10.0), ..UiRect::all(Val::Auto) }),
            bevy_gooey::Sizing { width: Some(bevy_gooey::Extent::Px(520.0)), max_height: Some(bevy_gooey::Extent::Percent(70.0)), ..default() },
            bevy_gooey::RowLayout { direction: Some(FlexDirection::Column), ..default() },
            bevy_gooey::GapPx(chrome::GAP_ROW),
            bevy_gooey::Spacing { padding: bevy_gooey::Sides::all(chrome::STEP_PAD), ..default() },
            bevy_gooey::BorderPx(bevy_gooey::Sides::all(chrome::EDGE_W)),
            bevy_gooey::InkFollow,
            bevy_gooey::ShownFollow::closed(),
            BackgroundColor(chrome::ink().panel_bg),
            BorderColor::all(chrome::ink().key),
            ProfileOverlay,
            bevy::picking::hover::Hovered::default(),
        ))
        .insert(Pickable::default())
        .id();
    commands.entity(overlay).with_children(|p| {
        chrome::title(p, "FRAME PROFILE");
        p.spawn((
            Text::new(format!(
                "{} frames of history  ·  {} shows and hides it",
                HISTORY_FRAMES,
                crate::terminal_pane::chord_words(&toggle_chord())
            )),
            TextColor(chrome::ink().dim),
            chrome::typed(chrome::text::LABEL),
        ));
        p.spawn((
            Text::new(String::new()),
            TextColor(chrome::ink().text),
            chrome::typed(chrome::text::LABEL),
            ProfileTally,
        ));
        p.spawn((
            bevy_gooey::column(crate::ui::RELOAD_ROW_GAP),
            bevy_gooey::TextFlow { grow: Some(1.0), ..default() },
            ProfileSpans,
        ));
    });
    commands.entity(overlay).insert(ChildOf(frame.viewport));
}

/// **The chord shows the panel, and turns recording on with it** — a panel that drew empty history until a second
/// control was found would be the thing this issue is about. Pressing it again hides the panel and stops collecting.
fn toggle_the_profile(
    keys: Res<ButtonInput<KeyCode>>,
    editing: Query<(), With<bevy_gooey::TextEditing>>,
    mut panels: Query<&mut Node, With<ProfileOverlay>>,
    mut profiler: ResMut<Profiler>,
) {
    if !editing.is_empty() || !toggle_chord().just_pressed(&keys) {
        return;
    }
    let open = panels.iter().any(|node| node.display != Display::None);
    for mut node in &mut panels {
        node.display = if open { Display::None } else { Display::Flex };
    }
    profiler.set_recording(!open);
}

/// Whether the panel is up, for a test and for a second front end.
pub fn panel_is_open(panels: &Query<&Node, With<ProfileOverlay>>) -> bool {
    panels.iter().any(|node| node.display != Display::None)
}

/// **Draw the last frame, when it changed.** `Local` holds the frame number already drawn, so a settled panel costs one
/// comparison and no rebuild — the shape `editor::refresh_reload_list` and the console both use.
fn draw_the_profile(
    mut commands: Commands,
    profiler: Res<Profiler>,
    overlays: Query<&Node, With<ProfileOverlay>>,
    lists: Query<Entity, With<ProfileSpans>>,
    mut tally: Query<&mut Text, (With<ProfileTally>, Without<ProfileSpans>)>,
    mut drawn: Local<Option<u64>>,
) {
    let Some(frame) = profiler.last() else { return };
    if drawn.as_ref() == Some(&frame.number) {
        return;
    }
    if overlays.iter().all(|node| node.display == Display::None) {
        *drawn = Some(frame.number);
        return;
    }
    *drawn = Some(frame.number);
    if let Ok(mut text) = tally.single_mut() {
        let want = match (frame.heaviest(), profiler.dropped()) {
            (Some(span), 0) => format!("frame {} · {:.3} ms · heaviest {} {:.3} ms", frame.number, frame.ms, span.system, span.ms),
            (Some(span), dropped) => {
                format!("frame {} · {:.3} ms · heaviest {} {:.3} ms · {dropped} fell off at {HISTORY_FRAMES}", frame.number, frame.ms, span.system, span.ms)
            }
            (None, _) => format!("frame {} · {:.3} ms · recording is off, so no span was named", frame.number, frame.ms),
        };
        if text.0 != want {
            text.0 = want;
        }
    }
    for list in &lists {
        commands.entity(list).despawn_related::<Children>();
        commands.entity(list).with_children(|p| {
            if frame.spans.is_empty() {
                p.spawn((
                    Text::new("no spans: press the chord to record, and the next frame names what it ran".to_owned()),
                    TextColor(chrome::ink().dim),
                    chrome::typed(chrome::text::LABEL),
                ));
                return;
            }
            for span in &frame.spans {
                let share = if frame.ms > 0.0 { span.ms / frame.ms * 100.0 } else { 0.0 };
                p.spawn((
                    Text::new(format!("{:>7.3} ms  {:>5.1}%  {}", span.ms, share, span.system)),
                    TextColor(chrome::ink().text),
                    chrome::typed(chrome::text::LABEL),
                ));
            }
        });
    }
}

/// **The profiler's own plugin**: the ring, the frame boundary, the panel and its chord.
pub struct ProfilePlugin;

impl Plugin for ProfilePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Profiler>()
            // **The head of the frame and its foot.** `First` is where `ui_budget::roll_the_frame` stands, and `Last` is
            // after every system a span could have been recorded in.
            .add_systems(First, begin_the_frame.after(crate::ui_budget::roll_the_frame))
            .add_systems(Last, close_the_frame)
            .add_systems(OnEnter(crate::screen::Screen::Editor), spawn_the_profile.after(chrome::FrameSystems))
            .add_systems(
                Update,
                (
                    toggle_the_profile.in_set(crate::keys::Phase::Act),
                    draw_the_profile.after(crate::keys::Phase::Act).after(toggle_the_profile),
                ),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ring_keeps_a_hundred_and_twenty_frames_and_says_what_fell_off() {
        let mut profiler = Profiler::default();
        for i in 0..500u64 {
            profiler.frames.push_back(Frame { number: i, ms: 1.0, spans: Vec::new() });
            while profiler.frames.len() > HISTORY_FRAMES {
                profiler.frames.pop_front();
                profiler.dropped += 1;
            }
        }
        assert_eq!(profiler.len(), HISTORY_FRAMES);
        assert_eq!(profiler.dropped(), 500 - HISTORY_FRAMES as u64);
        assert_eq!(profiler.last().map(|f| f.number), Some(499));
    }

    #[test]
    fn a_frames_heaviest_span_is_its_first_because_they_are_sorted() {
        let frame = Frame {
            number: 1,
            ms: 5.0,
            spans: vec![Span { system: "outliner", ms: 2.5 }, Span { system: "history", ms: 1.0 }],
        };
        assert_eq!(frame.heaviest().map(|s| s.system), Some("outliner"));
        assert!((frame.measured_ms() - 3.5).abs() < f64::EPSILON);
    }

    #[test]
    fn the_chord_is_free_and_answers_to_no_other() {
        let chord = toggle_chord();
        assert_eq!(chord.key, KeyCode::KeyD);
        assert!(chord.modifiers.shift);
        assert_ne!(crate::console::toggle_chord(), chord);
        assert_ne!(crate::terminal_pane::COLLAPSE_CHORD, chord);
        for (_, _, reserved) in crate::scale_chords::reserved_chords() {
            assert_ne!(reserved, chord);
        }
        for (_, chord) in crate::command_palette::chords() {
            assert_ne!(chord, toggle_chord());
        }
    }

    #[test]
    fn recording_off_collects_nothing_and_recording_on_collects() {
        let profiler = Profiler::default();
        assert!(!profiler.recording(), "nothing is collected until somebody asks");
        let mut on = Profiler::default();
        on.set_recording(true);
        record_span("outliner", 1.5);
        assert_eq!(pending().len(), 1);
        on.set_recording(false);
        assert!(pending().is_empty(), "turning it off drops a half-collected frame rather than keeping it");
        let after = pending().len();
        record_span("outliner", 2.0);
        assert_eq!(pending().len(), after, "and nothing is collected again until it is turned back on");
    }
}
