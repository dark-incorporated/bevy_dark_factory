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
//! every frame, and with recording off it loads `RECORDING`, finds it false and returns — no lock, no allocation, no
//! formatting. That is Renzora's *"one atomic load a frame, and nothing else"*.
//!
//! **The frame clock is not gated, and that is deliberate.** `blueprint/state.frame.frame_ms` and the panel must report
//! the same number for the same frame — a person and an agent disagreeing about how long a frame took is the defect this
//! issue is about. A number that only existed while recording was on would make them disagree exactly when somebody
//! looked, so the frame is always timed — an `Instant::now()` at its head and one at the end of each main schedule — and
//! only the *spans* are gated.
//!
//! # The history is bounded, and the mistake it avoids
//!
//! [`HISTORY_FRAMES`] is 120 — about two seconds at 60 fps, Renzora's own number and its own measured reason: at an
//! editor's load a frame is about 5,500 spans, and six hundred of them were 105 MB of memory nothing read. So the ring is
//! a `VecDeque` trimmed from the front, [`Profiler::dropped`] says how many fell off, and a frame's span list is only
//! ever as long as the number of instrumented systems — a `Vec<ProfileFrame>` grown per frame would be the same mistake.
//!
//! # Two clocks, because a frame answers two questions (#677)
//!
//! [`Frame::busy_ms`] is the main schedules' work: from the frame's head to the end of its last main schedule. Waiting
//! on the present does not lengthen it. [`Frame::wall_ms`] runs from the head to the **next** frame's head, so it also
//! holds what runs between two frames — the render world's extract, the wait for the render thread and its present, the
//! event loop's own wait. Until #677 the ring held only the first, and `blueprint/state` reported it twice: as
//! `frame_ms`, and as `busy_ms` from a second clock with the same `First`-to-`Last` bounds. The two agreed to 0.05 ms by
//! construction, so "is this the editor or the machine" had no answer. A frame is **whole** once the next one has
//! started; [`Profiler::last_whole`] is the newest whole frame, and the panel and the agent both read that one.
//!
//! # Where the busy time went: one share per main schedule (#678)
//!
//! The instrumented systems are three, and a workspace switch spends its time in none of them. So the frame's head, the
//! end of every schedule in `MainScheduleOrder`, and its foot are each a one-system schedule of their own, interleaved
//! into the order (`mark_the_main_schedules`), and a frame keeps the time between two marks under the name of the
//! schedule that ran between them: `StateTransition` (where every `OnExit` and `OnEnter` runs), `Update`, `PostUpdate`
//! (layout and text). The shares add up to `busy_ms` exactly. They are the clock read at more places, so they are not
//! gated either, and a frame's list reuses the allocation of the frame that fell off the ring.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex, MutexGuard};
use std::time::Instant;

use bevy::app::MainScheduleOrder;
use bevy::ecs::schedule::{InternedScheduleLabel, ScheduleLabel, SingleThreadedExecutor};
use bevy::prelude::*;

use crate::chrome;

/// **How many frames the ring keeps.** About two seconds at 60 fps: long enough to see a stutter, short enough that the
/// ring is not a second copy of the frame.
pub const HISTORY_FRAMES: usize = 120;

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

/// **One main schedule's share of a frame**: the time from the end of the schedule before it (or from the frame's head)
/// to its own end (#678). A frame's list is in run order and adds up to its [`Frame::busy_ms`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScheduleSpan {
    /// The schedule, as `MainScheduleOrder` names it: `StateTransition`, `Update`, `PostUpdate`…
    pub schedule: InternedScheduleLabel,
    pub ms: f64,
}

impl ScheduleSpan {
    /// The schedule's name, as the panel and `blueprint/state.frame.schedules` print it.
    pub fn name(&self) -> String {
        format!("{:?}", self.schedule)
    }
}

/// One finished frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Frame {
    /// The frame's number, from Bevy's own counter, so a person and an agent name the same frame.
    pub number: u64,
    /// **The main schedules' work**: from the frame's head to the end of its last main schedule, which waiting on the
    /// present does not lengthen. `blueprint/state.frame.busy_ms`.
    pub busy_ms: f64,
    /// **The whole frame**: from its head to the next frame's head, so the extract, the render thread's present and the
    /// event loop's wait are in it too. `None` until the next frame starts. `blueprint/state.frame.frame_ms`.
    pub wall_ms: Option<f64>,
    /// **The instrumented systems' spans**, heaviest first. Empty when recording was off for this frame.
    pub spans: Vec<Span>,
    /// **Where `busy_ms` went**: one share per main schedule, in the order they ran (#678).
    pub schedules: Vec<ScheduleSpan>,
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

    /// **What the frame spent outside the main schedules**: `wall_ms - busy_ms`, the machine and the present rather than
    /// the editor's own systems (#677). `None` until the frame is whole.
    pub fn outside_ms(&self) -> Option<f64> {
        self.wall_ms.map(|wall| wall - self.busy_ms)
    }

    /// **The main schedule that took most of `busy_ms`**, or `None` for a frame no mark has timed.
    pub fn heaviest_schedule(&self) -> Option<&ScheduleSpan> {
        self.schedules.iter().max_by(|a, b| a.ms.total_cmp(&b.ms))
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
    /// The head of the frame being run, kept until the next head so this frame's `wall_ms` can be closed.
    began: Option<Instant>,
    /// The last mark: the head, or the end of the main schedule that ran last.
    mark: Option<Instant>,
    /// This frame's schedule shares so far, reused frame to frame so a mark allocates nothing.
    this_frame: Vec<ScheduleSpan>,
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

    /// **The frame that finished last**: its spans and its `busy_ms` are in, and its `wall_ms` is not until the next
    /// frame starts.
    pub fn last(&self) -> Option<&Frame> {
        self.frames.back()
    }

    /// **The newest whole frame**: the newest whose next frame has started, so its `wall_ms` is known. The panel draws it
    /// and `blueprint/state.frame` reports it, so `frame_ms` and `busy_ms` are always one frame's two numbers.
    pub fn last_whole(&self) -> Option<&Frame> {
        self.frames.iter().rev().find(|frame| frame.wall_ms.is_some())
    }

    /// The newest whole frame's `wall_ms`, as the panel and the agent both read it: `frame_ms`.
    pub fn last_ms(&self) -> Option<f64> {
        self.last_whole().and_then(|frame| frame.wall_ms)
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

    /// **The heaviest frame in the ring by the editor's own work** (`busy_ms`), which is what "why is it slow" asks when
    /// the answer is code.
    pub fn heaviest_frame(&self) -> Option<&Frame> {
        self.frames.iter().max_by(|a, b| a.busy_ms.total_cmp(&b.busy_ms))
    }

    /// **The end of one main schedule**: the time since the last mark is that schedule's share of the frame.
    fn end_of(&mut self, schedule: InternedScheduleLabel) {
        let now = Instant::now();
        if let Some(mark) = self.mark.replace(now) {
            self.this_frame.push(ScheduleSpan { schedule, ms: now.duration_since(mark).as_secs_f64() * 1000.0 });
        }
    }
}

/// **The head of the frame**, run before the first main schedule: it makes the previous frame whole and starts this one's
/// clock. Kept whether or not spans are collected — see the module note.
fn begin_the_frame(mut profiler: ResMut<Profiler>) {
    let now = Instant::now();
    let profiler = &mut *profiler;
    // **The previous frame is whole now**: everything between its last schedule and this head — the extract, the render
    // thread's present, the event loop's wait — is in its wall time and not in its busy time (#677).
    if let Some(began) = profiler.began
        && let Some(previous) = profiler.frames.back_mut()
        && previous.wall_ms.is_none()
    {
        previous.wall_ms = Some(now.duration_since(began).as_secs_f64() * 1000.0);
    }
    profiler.began = Some(now);
    profiler.mark = Some(now);
    profiler.this_frame.clear();
}

/// **The foot of the frame**, run after the last main schedule. Its `busy_ms` is the head to the last mark, so the busy
/// time and the schedule shares are one clock and add up exactly. It takes what the instrumented systems recorded and
/// keeps the frame in the ring.
fn close_the_frame(mut profiler: ResMut<Profiler>, frames: Res<bevy::diagnostic::FrameCount>) {
    let profiler = &mut *profiler;
    let (Some(began), Some(mark)) = (profiler.began, profiler.mark) else { return };
    let busy_ms = mark.duration_since(began).as_secs_f64() * 1000.0;
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
    // **The ring is trimmed before it grows**, so the frame that falls off lends its schedule list to this one and a full
    // ring allocates nothing for it.
    let mut schedules = Vec::new();
    while profiler.frames.len() >= HISTORY_FRAMES {
        let Some(fell_off) = profiler.frames.pop_front() else { break };
        schedules = fell_off.schedules;
        profiler.dropped += 1;
    }
    schedules.clear();
    schedules.extend_from_slice(&profiler.this_frame);
    profiler.frames.push_back(Frame { number: u64::from(frames.0), busy_ms, wall_ms: None, spans, schedules });
}

/// The frame's head: a schedule of its own, run before the first main schedule.
#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct FrameHead;

/// The frame's foot: a schedule of its own, run after the last main schedule.
#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct FrameFoot;

/// **The end of one main schedule**: a schedule of one system, run straight after the schedule it names.
#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct EndOf(InternedScheduleLabel);

/// **Put the profile's marks between the main schedules** (#678): its head before the first, an [`EndOf`] after each, its
/// foot after the last. `build` runs it with the schedules the plugins before this one added, and `finish` runs it again
/// once every plugin has added its own — a schedule inserted after a neighbour would otherwise be counted in that
/// neighbour's share — so it rebuilds the order from the schedules that are not its own, and running it twice is running
/// it once.
fn mark_the_main_schedules(app: &mut App) {
    let order = app.world().resource::<MainScheduleOrder>().labels.clone();
    let (head, foot) = (FrameHead.intern(), FrameFoot.intern());
    let ours = |label: InternedScheduleLabel| {
        label == head || label == foot || order.iter().any(|&other| label == EndOf(other).intern())
    };
    let theirs: Vec<InternedScheduleLabel> = order.iter().copied().filter(|&label| !ours(label)).collect();
    let mut labels = Vec::with_capacity(theirs.len() * 2 + 2);
    labels.push(head);
    for &label in &theirs {
        labels.push(label);
        labels.push(EndOf(label).intern());
    }
    labels.push(foot);
    app.world_mut().resource_mut::<MainScheduleOrder>().labels = labels;
    for label in theirs {
        if app.get_schedule(EndOf(label)).is_none() {
            app.edit_schedule(EndOf(label), |schedule| {
                schedule
                    .set_executor(SingleThreadedExecutor::new())
                    .add_systems(move |mut profiler: ResMut<Profiler>| profiler.end_of(label));
            });
        }
    }
}

/// **The frame clock**: the ring, the head and foot around the main schedules, and a mark after each (#677, #678).
fn time_the_frame(app: &mut App) {
    app.init_resource::<Profiler>()
        .edit_schedule(FrameHead, |schedule| {
            schedule.set_executor(SingleThreadedExecutor::new()).add_systems(begin_the_frame);
        })
        .edit_schedule(FrameFoot, |schedule| {
            schedule.set_executor(SingleThreadedExecutor::new()).add_systems(close_the_frame);
        });
    mark_the_main_schedules(app);
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
            // Its gap and inset are the sheet's `.chrome-overlay`, the journal's.
            bevy_gooey::StyleClass::of("chrome-overlay"),
            bevy_gooey::BorderPx(bevy_gooey::Sides::all(crate::style::metrics().layout.edge_w)),
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
                crate::terminal_pane::chord_words(&crate::chords::chord(crate::chords::Fixed::ProfileToggle))
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
    if !editing.is_empty() || !crate::chords::chord(crate::chords::Fixed::ProfileToggle).just_pressed(&keys) {
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

/// **Draw the newest whole frame, when it changed.** `Local` holds the frame number already drawn, so a settled panel
/// costs one comparison and no rebuild — the shape `editor::refresh_reload_list` and the console both use.
fn draw_the_profile(
    mut commands: Commands,
    profiler: Res<Profiler>,
    overlays: Query<&Node, With<ProfileOverlay>>,
    lists: Query<Entity, With<ProfileSpans>>,
    mut tally: Query<&mut Text, (With<ProfileTally>, Without<ProfileSpans>)>,
    mut drawn: Local<Option<u64>>,
) {
    let Some(frame) = profiler.last_whole() else { return };
    let Some(wall_ms) = frame.wall_ms else { return };
    if drawn.as_ref() == Some(&frame.number) {
        return;
    }
    if overlays.iter().all(|node| node.display == Display::None) {
        *drawn = Some(frame.number);
        return;
    }
    *drawn = Some(frame.number);
    if let Ok(mut text) = tally.single_mut() {
        // **Both clocks of one frame** (#677): the whole frame, and how much of it the main schedules spent.
        let clocks = format!("frame {} · {wall_ms:.3} ms, {:.3} ms of it the schedules", frame.number, frame.busy_ms);
        let want = match (frame.heaviest(), profiler.dropped()) {
            (Some(span), 0) => format!("{clocks} · heaviest {} {:.3} ms", span.system, span.ms),
            (Some(span), dropped) => {
                format!("{clocks} · heaviest {} {:.3} ms · {dropped} fell off at {HISTORY_FRAMES}", span.system, span.ms)
            }
            (None, _) => format!("{clocks} · recording is off, so no span was named"),
        };
        if text.0 != want {
            text.0 = want;
        }
    }
    let share = |ms: f64| if wall_ms > 0.0 { ms / wall_ms * 100.0 } else { 0.0 };
    for list in &lists {
        commands.entity(list).despawn_related::<Children>();
        commands.entity(list).with_children(|p| {
            // **Where the frame went** (#678): each main schedule's share, then what ran outside them all (#677).
            for schedule in &frame.schedules {
                row(p, format!("{:>7.3} ms  {:>5.1}%  {}", schedule.ms, share(schedule.ms), schedule.name()), chrome::ink().text);
            }
            if let Some(outside) = frame.outside_ms() {
                let words = format!("{outside:>7.3} ms  {:>5.1}%  outside the schedules: extract, present, wait", share(outside));
                row(p, words, chrome::ink().dim);
            }
            if frame.spans.is_empty() {
                row(p, "no spans: press the chord to record, and the next frame names what it ran".to_owned(), chrome::ink().dim);
                return;
            }
            for span in &frame.spans {
                row(p, format!("{:>7.3} ms  {:>5.1}%  {}", span.ms, share(span.ms), span.system), chrome::ink().text);
            }
        });
    }
}

/// One line of the profile's list.
fn row(p: &mut ChildSpawnerCommands, words: String, ink: Color) {
    p.spawn((Text::new(words), TextColor(ink), chrome::typed(chrome::text::LABEL)));
}

/// **The profiler's own plugin**: the ring, the frame boundary, the panel and its chord.
pub struct ProfilePlugin;

impl Plugin for ProfilePlugin {
    fn build(&self, app: &mut App) {
        time_the_frame(app);
        app.add_systems(OnEnter(crate::screen::Screen::Editor), spawn_the_profile.after(chrome::FrameSystems)).add_systems(
            Update,
            (
                toggle_the_profile.in_set(crate::keys::Phase::Act),
                draw_the_profile.after(crate::keys::Phase::Act).after(toggle_the_profile),
            ),
        );
    }

    /// **Every plugin has added its schedules by now**, so the marks are put between them again: a schedule a later
    /// plugin inserted is its own share and not part of its neighbour's.
    fn finish(&self, app: &mut App) {
        mark_the_main_schedules(app);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ring_keeps_a_hundred_and_twenty_frames_and_says_what_fell_off() {
        let mut profiler = Profiler::default();
        for i in 0..500u64 {
            profiler.frames.push_back(Frame { number: i, busy_ms: 1.0, ..Frame::default() });
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
            busy_ms: 5.0,
            spans: vec![Span { system: "outliner", ms: 2.5 }, Span { system: "history", ms: 1.0 }],
            ..Frame::default()
        };
        assert_eq!(frame.heaviest().map(|s| s.system), Some("outliner"));
        assert!((frame.measured_ms() - 3.5).abs() < f64::EPSILON);
    }

    #[test]
    fn the_chord_is_free_and_answers_to_no_other() {
        use crate::chords::{Fixed, default_of};
        // **`Cmd+Shift+D` was the project report's** (#560): this test once pinned `KeyD` and hand-listed six
        // "others", which is why the collision with `report.project` survived — the list never named it. It now
        // compares against the whole table, and the profile's own key moved to `F`.
        let chord = default_of(Fixed::ProfileToggle);
        assert_eq!(chord.key, KeyCode::KeyF);
        assert!(chord.modifiers.shift);
        // **Every other fixed chord**, from the table itself: a hand-written list is what let the collision with
        // `report.project` live, because nobody thought to add it.
        for named in crate::chords::CHORDS {
            if named.fixed == Fixed::ProfileToggle {
                continue;
            }
            assert_ne!((named.default)(), chord, "{} shares the profile's chord", named.id);
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

    /// **The marks stand between the main schedules, and marking twice is marking once** (#678): `finish` runs the same
    /// function `build` did, and must not stack a second mark after a schedule.
    #[test]
    fn the_marks_stand_between_the_main_schedules_and_marking_twice_changes_nothing() {
        let mut app = App::new();
        mark_the_main_schedules(&mut app);
        let once = app.world().resource::<MainScheduleOrder>().labels.clone();
        mark_the_main_schedules(&mut app);
        let twice = app.world().resource::<MainScheduleOrder>().labels.clone();
        assert_eq!(once, twice, "a second marking changed the order");
        assert_eq!(once.first(), Some(&FrameHead.intern()), "the head runs first: {once:?}");
        assert_eq!(once.last(), Some(&FrameFoot.intern()), "the foot runs last: {once:?}");
        let between = &once[1..once.len() - 1];
        assert!(between.len() >= 2 && between.len().is_multiple_of(2), "{once:?}");
        for pair in between.chunks(2) {
            assert_eq!(pair[1], EndOf(pair[0]).intern(), "every schedule is followed by its own mark: {once:?}");
        }
        assert!(between.contains(&Update.intern()), "and the main schedules are still there: {once:?}");
    }

    /// **A wait between frames is in the wall time and not in the busy time** (#677): the gap is a sleep outside
    /// `app.update()`, which is where the present and the event loop's wait fall in a running editor. Before the split
    /// the ring held one clock, head to foot, so the sleep was in neither number and the two agreed.
    #[test]
    fn a_wait_between_frames_is_in_the_wall_time_and_not_in_the_busy_time() {
        const WORK: std::time::Duration = std::time::Duration::from_millis(20);
        const WAIT: std::time::Duration = std::time::Duration::from_millis(30);
        let mut app = App::new();
        app.init_resource::<bevy::diagnostic::FrameCount>();
        time_the_frame(&mut app);
        app.add_systems(Update, || std::thread::sleep(WORK));
        app.update();
        std::thread::sleep(WAIT);
        app.update();
        let profiler = app.world().resource::<Profiler>();
        let whole = profiler.last_whole().cloned().expect("the first frame is whole once the second has started");
        let wall = whole.wall_ms.expect("a whole frame has a wall time");
        let wait_ms = WAIT.as_secs_f64() * 1000.0;
        assert!(whole.busy_ms >= WORK.as_secs_f64() * 1000.0, "the busy time holds the schedules' work: {whole:?}");
        assert!(wall - whole.busy_ms >= wait_ms, "the wait between frames is in the wall time and not the busy time: {whole:?}");
        assert_eq!(profiler.last_ms(), Some(wall), "`frame_ms` is the whole frame's wall time");
        let shares: f64 = whole.schedules.iter().map(|s| s.ms).sum();
        assert!((shares - whole.busy_ms).abs() < 1e-6, "the schedule shares add up to the busy time: {shares} against {whole:?}");
        let heaviest = whole.heaviest_schedule().expect("a timed frame has shares");
        assert_eq!(heaviest.schedule, Update.intern(), "and the schedule that slept is the one named: {whole:?}");
    }
}
