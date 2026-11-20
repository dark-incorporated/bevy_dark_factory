//! **The editor's own log, in the window** (#509).
//!
//! # The gap this closes
//!
//! `log_file.rs` writes every `tracing` event to `<data_dir>/Blueprint/dark_factory.log`, and it exists because a
//! release DMG launched from Finder drew a black window and *nothing anywhere said why*: a bundle launch has no
//! terminal for stderr. The fix was a file — and there is still no panel, so the person reads it over ssh exactly as
//! before.
//!
//! What the window does show is the **journal** (`chrome::Journal`, `Cmd+E`): refusals and an agent's steps, capped at
//! [`chrome::JOURNAL_CAP`]. That is *"what went wrong, and what an agent did"*. This is the other half and it is
//! deliberately a **separate panel**: *"what the process said"* — every `info!`, `warn!` and `error!` from any system,
//! at the level the log file already records.
//!
//! # The ring, and why it is shared
//!
//! A `tracing` layer lives in the subscriber, outside the ECS; the panel is a resource inside it. So the layer and the
//! resource hold **one** [`Ring`] behind a mutex, and the layer is the only writer of lines. The panel reads
//! [`Ring::pushed`] — a **monotonic** count of every entry ever appended — so a rebuild appends only what is new rather
//! than walking the list every frame, which is the shape Renzora's console documents and this repo's `reload` panel
//! already uses (a `Local` of what was drawn, and a comparison).
//!
//! The ring is **bounded and trimmed from the front** ([`MAX_ROWS`]), because a batch that logs one line per frame is a
//! real thing this editor does — `log_file`'s own audit watched it write 13.5 GB in five minutes from one repeated line.
//! A line identical to the one just before it folds into it with a repeat count instead of occupying a second row.
//!
//! # The entity chip, which is why a console is worth having *here*
//!
//! A refusal in this editor is usually about one named piece, and `Status::problem_about` already carries that id. A log
//! event that names one — `warn!(piece = %id, "…")`, the field [`PIECE_FIELD`] — becomes a **chip** in the details pane,
//! and pressing it calls [`Selection::only`], so a line in a log is a way into the map rather than a sentence to retype.
//!
//! # The chord is outside the key census, on purpose
//!
//! `Cmd`/`Ctrl`+`Shift`+`E` is a [`bevy_gooey::Shortcut`] read directly, like `scale_chords`' three and
//! `terminal_pane`'s two: it acts on the editor's frame, it must work on every screen, and it is listed among
//! `keymap_screen`'s reserved chords so no command can be rebound onto it. It is **not** a `keys::Action`, because the
//! census counts what a tab gains and this belongs to no tab.
//!
//! **The journal keeps `Cmd+E`.** The pair reads: *errors* (what was refused, what an agent did) and *log* (what the
//! process said), one key apart, neither a second front end onto the other's list.

use std::collections::VecDeque;
use std::fmt::Debug;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use bevy::prelude::*;
use bevy_gooey::{Modifiers, Shortcut};

use crate::chrome::{self, Severity};
use bevy::picking::hover::Hovered;

/// **How many lines the ring keeps.** Older ones are trimmed from the front, and [`Ring::dropped`] says how many fell off
/// so the panel can say what it left out rather than looking complete.
pub const MAX_ROWS: usize = 400;

/// **The tracing field that names the piece a line is about.** A line carrying it offers a chip that selects that piece;
/// the shape `Status::problem_about` already keeps (`blueprint_model::problem::Location::entity`).
pub const PIECE_FIELD: &str = "piece";

/// **The chord that shows and hides the console**, beside the journal's `Cmd+E`. Not a `const` because
/// `Shortcut::new` is not `const` at this pin, and one function is one definition.
pub fn toggle_chord() -> Shortcut {
    Shortcut::new(Modifiers::PRIMARY | Modifiers::SHIFT, KeyCode::KeyE)
}

/// **The frame a line was recorded in**, published by [`count_the_frames`].
///
/// An atomic rather than a resource because the layer runs on whatever thread emitted the event and has no world to
/// read; one relaxed load per event is the whole cost, and zero when nothing logs.
static FRAME: AtomicU64 = AtomicU64::new(0);

/// **What a line's level is, as the pills filter.** Deliberately not `tracing::Level`: the panel has four pills and
/// `TRACE` is not one of them (nothing in this crate emits it, and a `trace` line would drown the panel it was put in).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Debug,
    Info,
    Warn,
    Error,
}

impl Level {
    /// **Every level, in the order the pills stand**: quietest first, so the row reads as a severity ladder.
    pub const ALL: [Level; 4] = [Level::Debug, Level::Info, Level::Warn, Level::Error];

    /// The pill's word.
    pub fn label(self) -> &'static str {
        match self {
            Level::Debug => "DEBUG",
            Level::Info => "INFO",
            Level::Warn => "WARN",
            Level::Error => "ERROR",
        }
    }

    /// Where it stands in [`Level::ALL`], for a `[bool; 4]` of what is hidden.
    pub fn index(self) -> usize {
        match self {
            Level::Debug => 0,
            Level::Info => 1,
            Level::Warn => 2,
            Level::Error => 3,
        }
    }

    /// The level a `tracing` event is, or `None` for one this panel does not carry.
    fn of(level: &bevy::log::tracing::Level) -> Level {
        match *level {
            bevy::log::tracing::Level::ERROR => Level::Error,
            bevy::log::tracing::Level::WARN => Level::Warn,
            bevy::log::tracing::Level::INFO => Level::Info,
            _ => Level::Debug,
        }
    }

    /// **The ink a line of this level draws in** — the panel's own colours, since gooey has no per-severity text role.
    pub fn ink(self) -> Color {
        match self {
            Level::Debug => chrome::ink().dim,
            Level::Info => chrome::ink().text,
            Level::Warn => chrome::ink().accent,
            Level::Error => chrome::ink().danger,
        }
    }
}

/// **One line the process said.**
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    /// **Its place in the ring's history**: `pushed` as it was when the line was appended, so it never moves and a
    /// click can name the line it was on even after older ones are trimmed.
    pub id: u64,
    pub level: Level,
    /// The `tracing` target — the module that said it.
    pub target: String,
    pub message: String,
    /// **The piece this line is about**, when the event carried a [`PIECE_FIELD`] field. Selects on a chip.
    pub piece: Option<String>,
    /// The frame it was recorded in, as [`FRAME`] stood. A line before the first frame is frame 0.
    pub frame: u64,
    /// **How many times in a row this exact line arrived.** A fold, not a second row: a batch that logs one line per
    /// frame would otherwise fill the ring with the same sentence.
    pub count: u32,
}

/// **The bounded history of what the process said**, shared by the layer and the panel.
#[derive(Default)]
pub struct Ring {
    lines: VecDeque<Line>,
    /// **Every entry ever appended**, which only goes up. The panel compares this to what it drew, so a settled frame
    /// costs one load and nothing else — Renzora's `pushed`, and the reason a busy frame does not rebuild the list.
    pushed: u64,
    /// How many fell off the front at [`MAX_ROWS`], so the panel can say what it left out.
    dropped: u64,
}

impl Ring {
    /// **Append one line**, folding it into the one before it when it is the same sentence from the same place.
    pub fn push(&mut self, level: Level, target: &str, message: String, piece: Option<String>) {
        let frame = FRAME.load(Ordering::Relaxed);
        if let Some(last) = self.lines.back_mut()
            && last.level == level
            && last.target == target
            && last.message == message
        {
            last.count += 1;
            self.pushed += 1;
            return;
        }
        let id = self.pushed;
        self.pushed += 1;
        self.lines.push_back(Line { id, level, target: target.to_owned(), message, piece, frame, count: 1 });
        while self.lines.len() > MAX_ROWS {
            self.lines.pop_front();
            self.dropped += 1;
        }
    }

    /// The lines the ring holds, oldest first.
    pub fn lines(&self) -> impl Iterator<Item = &Line> {
        self.lines.iter()
    }

    /// How many lines are held now.
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// Whether the ring holds nothing.
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Every entry ever appended.
    pub fn pushed(&self) -> u64 {
        self.pushed
    }

    /// How many fell off the front at [`MAX_ROWS`].
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// The line with this id, if it has not been trimmed yet.
    pub fn line(&self, id: u64) -> Option<&Line> {
        self.lines.iter().find(|line| line.id == id)
    }

    /// **How many lines of each level the ring holds**, in [`Level::ALL`]'s order, for the pills' counts.
    pub fn counts(&self) -> [usize; 4] {
        let mut counts = [0; 4];
        for line in &self.lines {
            counts[line.level.index()] += 1;
        }
        counts
    }

    /// The lines a set of level filters lets through, newest first — what the panel draws.
    pub fn shown(&self, hidden: &[bool; 4]) -> Vec<&Line> {
        self.lines.iter().filter(|line| !hidden[line.level.index()]).rev().collect()
    }
}

/// **The one handle the layer and the panel share.** Cloning it is cloning an `Arc`; the mutex is held for one append.
#[derive(Clone, Default)]
pub struct Sink(pub(crate) Arc<Mutex<Ring>>);

impl Sink {
    /// The ring, locked. A poisoned lock means a panic happened while logging, and the lines are still the best
    /// record, so the guard is taken rather than panicked on — `log_file::OneSink` makes the same call for the same
    /// reason.
    fn lock(&self) -> MutexGuard<'_, Ring> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// **Append a line.** The panel's tests push through this, and so does the layer.
    pub fn push(&self, level: Level, target: &str, message: String, piece: Option<String>) {
        self.lock().push(level, target, message, piece);
    }

    /// Read the ring, for a test or the panel.
    pub fn read(&self) -> MutexGuard<'_, Ring> {
        self.lock()
    }
}

/// **The console as the editor holds it**: the ring, and what the panel is showing of it.
#[derive(Resource, Default)]
pub struct Console {
    sink: Sink,
    /// **Which levels the person has turned off**, in [`Level::ALL`]'s order. Turning one off hides exactly that level
    /// and leaves the counts alone: a pill says how many lines there *are*, which is what makes it worth pressing.
    pub hidden: [bool; 4],
    /// **The line whose details are open**, by [`Line::id`]. Clicking a row opens it; clicking it again closes it.
    pub open: Option<u64>,
}

impl Console {
    /// The shared sink, for a test that wants to fill the ring or read it.
    pub fn sink(&self) -> &Sink {
        &self.sink
    }

    /// Read the ring.
    pub fn read(&self) -> MutexGuard<'_, Ring> {
        self.sink.read()
    }
}

/// **The `tracing` layer that fills the ring.** Installed beside the file layer, under the same filter, so the panel and
/// the file say the same thing.
pub fn layer(sink: &Sink) -> bevy::log::BoxedLayer {
    bevy::log::tracing_subscriber::layer::Layer::boxed(SinkLayer { sink: sink.clone() })
}

/// **`LogPlugin::custom_layer`'s target**: the console layer, when this run has a console at all.
///
/// `custom_layer` takes a `fn` pointer rather than a closure, so the sink is found in the world, where [`ConsolePlugin`]
/// (or `main`) put it before `DefaultPlugins` was added.
pub fn plugin_layer(app: &mut bevy::app::App) -> Option<bevy::log::BoxedLayer> {
    app.world().get_resource::<Console>().map(|console| layer(&console.sink))
}

/// **The file and the console, as one layer**, so `main` sets `LogPlugin::custom_layer` once.
pub fn both_layers(app: &mut bevy::app::App) -> Option<bevy::log::BoxedLayer> {
    use bevy::log::tracing_subscriber::layer::Layer;
    let file = crate::log_file::plugin_layer(app);
    let console = plugin_layer(app);
    match (file, console) {
        // **Two boxed layers stacked**, which is what `custom_layer` returns and what `LogPlugin` composes over the
        // filter — a tuple of two `Box<dyn Layer>` is not itself a layer, so each is layered on the other.
        (Some(file), Some(console)) => Some(Layer::and_then(file, console).boxed()),
        (Some(file), None) => Some(file),
        (None, Some(console)) => Some(console),
        (None, None) => None,
    }
}

/// The layer itself: one `on_event`, which reads the level, the target, the message and a [`PIECE_FIELD`] field.
struct SinkLayer {
    sink: Sink,
}

impl<S: bevy::log::tracing::Subscriber> bevy::log::tracing_subscriber::Layer<S> for SinkLayer {
    fn on_event(&self, event: &bevy::log::tracing::Event<'_>, _ctx: bevy::log::tracing_subscriber::layer::Context<'_, S>) {
        let mut fields = Fields::default();
        event.record(&mut fields);
        // **A line with no message is not a line this panel can show**, and `log_file`'s layer would print the same
        // emptiness. It is dropped rather than drawn as a blank row.
        if fields.message.is_empty() {
            return;
        }
        let metadata = event.metadata();
        self.sink.push(Level::of(metadata.level()), metadata.target(), fields.message, fields.piece);
    }

    fn max_level_hint(&self) -> Option<bevy::log::tracing_subscriber::filter::LevelFilter> {
        // Everything the subscriber's filter lets through, so this layer never narrows what the file records.
        None
    }
}

/// What one event carried.
#[derive(Default)]
struct Fields {
    message: String,
    piece: Option<String>,
}

impl bevy::log::tracing::field::Visit for Fields {
    fn record_debug(&mut self, field: &bevy::log::tracing::field::Field, value: &dyn Debug) {
        let name = field.name();
        if name == "message" {
            self.message = format!("{value:?}");
        } else if name == PIECE_FIELD {
            // A string field records as `"wall@0"` through `record_debug`, and the quotes are not part of the id.
            self.piece = Some(format!("{value:?}").trim_matches('"').to_owned());
        }
    }

    fn record_str(&mut self, field: &bevy::log::tracing::field::Field, value: &str) {
        let name = field.name();
        if name == "message" {
            self.message = value.to_owned();
        } else if name == PIECE_FIELD {
            self.piece = Some(value.to_owned());
        }
    }
}

/// **Markers the panel is drawn by**, so a test can find each half rather than a column of words.
#[derive(Component)]
pub struct ConsoleOverlay;

/// One level pill, by the level it turns on and off.
#[derive(Component, Clone, Copy)]
pub struct ConsolePill(pub Level);

/// The pill's label, rewritten when its count moves.
#[derive(Component, Clone, Copy)]
pub struct ConsolePillLabel(pub Level);

/// The line above the rows: how many are shown of how many the ring holds, and how many fell off.
#[derive(Component)]
pub struct ConsoleTally;

/// The column the rows are drawn into.
#[derive(Component)]
pub struct ConsoleRows;

/// One drawn row, by the id of the line it shows.
#[derive(Component, Clone, Copy)]
pub struct ConsoleRow(pub u64);

/// The pane an open line is read in.
#[derive(Component)]
pub struct ConsoleDetails;

/// The chip in the details pane that selects the piece a line names.
#[derive(Component, Clone)]
pub struct ConsolePieceChip(pub String);

/// **The console's own plugin**: the ring, the panel, and the frame the lines are stamped with.
pub struct ConsolePlugin;

impl Plugin for ConsolePlugin {
    fn build(&self, app: &mut App) {
        // **The resource must exist before `DefaultPlugins` builds its subscriber** when `main` is the caller, which is
        // why `main` inserts it too; `init_resource` here is what a test app and any other front end gets.
        // **The resource is inserted if it is not there, and kept if it is.** `main` inserts it before `DefaultPlugins`
        // so the subscriber's layer finds it; a headless test's app has no `LogPlugin` at all, so this is where its ring
        // comes from. `init_resource` would be the same call but would also register it in a world that already had one,
        // so the check stays.
        if !app.world().contains_resource::<Console>() {
            app.init_resource::<Console>();
        }
        app.add_systems(
            OnEnter(crate::screen::Screen::Editor),
            spawn_the_console.after(chrome::FrameSystems),
        )
        .add_systems(
            Update,
            (
                count_the_frames,
                toggle_the_console.in_set(crate::keys::Phase::Act),
                // **After the chord**, so the frame a press opens the panel is the frame it is drawn in.
                draw_the_console.after(crate::keys::Phase::Act).after(toggle_the_console),
            ),
        );
    }
}

/// **Publish this frame's number to the layer**, which has no world to read.
fn count_the_frames(frames: Res<bevy::diagnostic::FrameCount>) {
    FRAME.store(u64::from(frames.0), Ordering::Relaxed);
}

/// **`Cmd`/`Ctrl`+`Shift`+`E` shows the console, and the same chord puts it away.**
///
/// A field that owns the keyboard keeps it: the census refuses every chord while a box is typing, and this is no
/// different (`scale_chords::the_scale_chords` makes the same call).
fn toggle_the_console(
    keys: Res<ButtonInput<KeyCode>>,
    editing: Query<(), With<bevy_gooey::TextEditing>>,
    mut panels: Query<&mut Node, With<ConsoleOverlay>>,
    mut console: ResMut<Console>,
) {
    if !editing.is_empty() || !toggle_chord().just_pressed(&keys) {
        return;
    }
    let open = panels.iter().any(|node| node.display != Display::None);
    for mut node in &mut panels {
        node.display = if open { Display::None } else { Display::Flex };
    }
    if open {
        // A closed panel keeps no open row, so reopening it does not stand on a line the person has forgotten.
        console.open = None;
    }
}

/// **Whether the console is up**, for a test and for a second front end.
pub fn is_open(panels: &Query<&Node, With<ConsoleOverlay>>) -> bool {
    panels.iter().any(|node| node.display != Display::None)
}

/// **The console stands where the journal does** — an overlay over the viewport, hidden until its chord — and the journal
/// keeps `Cmd+E`.
fn spawn_the_console(mut commands: Commands, frame: Res<chrome::Frame>) {
    let overlay = commands
        .spawn((
            bevy_gooey::placed(UiRect { left: Val::Percent(6.0), top: Val::Percent(8.0), ..UiRect::all(Val::Auto) }),
            bevy_gooey::Sizing { width: Some(bevy_gooey::Extent::Percent(80.0)), max_height: Some(bevy_gooey::Extent::Percent(76.0)), ..default() },
            bevy_gooey::RowLayout { direction: Some(FlexDirection::Column), ..default() },
            bevy_gooey::GapPx(chrome::GAP_ROW),
            bevy_gooey::Spacing { padding: bevy_gooey::Sides::all(chrome::STEP_PAD), ..default() },
            bevy_gooey::BorderPx(bevy_gooey::Sides::all(chrome::EDGE_W)),
            bevy_gooey::InkFollow,
            bevy_gooey::ShownFollow::closed(),
            BackgroundColor(chrome::ink().panel_bg),
            BorderColor::all(chrome::ink().key),
            ConsoleOverlay,
            Hovered::default(),
        ))
        // The overlay is an opaque box under the pointer: without this a click would place or delete on the map behind it
        // (`chrome::notifications_overlay` carries the same line for the journal).
        .insert(Pickable::default())
        .id();
    commands.entity(overlay).with_children(|p| {
        chrome::title(p, "CONSOLE");
        p.spawn((
            Text::new(format!(
                "every info, warn and error this process said  ·  {} shows and hides it  ·  {} closes it",
                crate::terminal_pane::chord_words(&toggle_chord()),
                crate::keys::chord(crate::keys::Action::Cancel)
            )),
            TextColor(chrome::ink().dim),
            chrome::typed(chrome::text::LABEL),
        ));
        // The pills and the tally, on one row: each pill carries its own count, which is what makes it worth pressing.
        p.spawn(bevy_gooey::row(chrome::GAP_ROW)).with_children(|row| {
            for level in Level::ALL {
                let mut pill = chrome::chip(
                    row,
                    ConsolePill(level),
                    level.label(),
                    chrome::text::LABEL,
                    level.ink(),
                    chrome::ink().row_bg,
                    chrome::ink().panel_edge,
                );
                pill.observe(switch_a_level);
                row.spawn((
                    Text::new(String::new()),
                    TextColor(level.ink()),
                    chrome::typed(chrome::text::LABEL),
                    ConsolePillLabel(level),
                ));
            }
            row.spawn((
                Text::new(String::new()),
                TextColor(chrome::ink().dim),
                chrome::typed(chrome::text::LABEL),
                ConsoleTally,
            ));
        });
        p.spawn((
            Text::new(String::new()),
            TextColor(chrome::ink().text),
            chrome::typed(chrome::text::LABEL),
            ConsoleDetails,
        ));
        p.spawn((
            bevy_gooey::column(crate::ui::RELOAD_ROW_GAP),
            bevy_gooey::TextFlow { grow: Some(1.0), ..default() },
            ConsoleRows,
        ));
    });
    commands.entity(overlay).insert(ChildOf(frame.viewport));
}

/// **A pill turned off hides that level and every other one stays.** The pill's own words and count are its level's; the
/// panel's `hidden` says which of the four it is.
fn switch_a_level(
    activate: On<bevy_gooey::Activate>,
    pills: Query<&ConsolePill>,
    mut console: ResMut<Console>,
) {
    let Ok(pill) = pills.get(activate.entity) else { return };
    let at = pill.0.index();
    console.hidden[at] = !console.hidden[at];
}

/// **A row clicked opens its details; the same row clicked again closes them.**
fn open_a_line(
    click: On<bevy::picking::events::PointerClick>,
    rows: Query<&ConsoleRow>,
    mut console: ResMut<Console>,
) {
    let Ok(row) = rows.get(click.entity) else { return };
    console.open = if console.open == Some(row.0) { None } else { Some(row.0) };
}

/// **The chip selects the piece the open line names**, through the one call a click on the outliner uses.
fn select_the_piece(
    activate: On<bevy_gooey::Activate>,
    chips: Query<&ConsolePieceChip>,
    mut selection: Option<ResMut<crate::selection::Selection>>,
) {
    let Ok(chip) = chips.get(activate.entity) else { return };
    if let Some(selection) = selection.as_deref_mut() {
        selection.only(&chip.0);
    }
}

/// **Draw what changed, and nothing when nothing did.**
///
/// `Local` holds what was drawn — the ring's `pushed`, the level filters and the open line — because the ring moves
/// outside the ECS and `is_changed` on the resource would be true whenever *anything* touched it. A settled frame
/// therefore costs three comparisons and one relaxed read.
fn draw_the_console(
    mut commands: Commands,
    console: Res<Console>,
    overlay: Query<&Node, With<ConsoleOverlay>>,
    rows: Query<Entity, With<ConsoleRows>>,
    mut details: Query<&mut Text, (With<ConsoleDetails>, Without<ConsolePillLabel>, Without<ConsoleTally>)>,
    mut labels: Query<(&mut Text, &ConsolePillLabel), Without<ConsoleDetails>>,
    mut tally: Query<&mut Text, (With<ConsoleTally>, Without<ConsoleDetails>, Without<ConsolePillLabel>)>,
    mut drawn: Local<Option<(u64, [bool; 4], Option<u64>)>>,
) {
    let ring = console.read();
    let key = (ring.pushed(), console.hidden, console.open);
    if drawn.as_ref() == Some(&key) {
        return;
    }
    let counts = ring.counts();
    let dropped = ring.dropped();
    let total = ring.len();
    let shown: Vec<Line> = ring.shown(&console.hidden).into_iter().cloned().collect();
    let open = console.open.and_then(|id| ring.line(id).cloned());
    drop(ring);
    // **A closed panel costs one comparison.** Its nodes are `Display::None`, so nothing below would be laid out anyway,
    // but the list would still be rebuilt every time a line arrived.
    if overlay.iter().all(|node| node.display == Display::None) {
        *drawn = Some(key);
        return;
    }
    *drawn = Some(key);

    for (mut text, label) in &mut labels {
        let want = counts[label.0.index()].to_string();
        if text.0 != want {
            text.0.clone_from(&want);
        }
    }
    if let Ok(mut text) = tally.single_mut() {
        let want = if dropped > 0 {
            format!("{}/{total} shown · {dropped} fell off at {MAX_ROWS}", shown.len())
        } else {
            format!("{}/{total} shown", shown.len())
        };
        if text.0 != want {
            text.0 = want;
        }
    }
    if let Ok(mut text) = details.single_mut() {
        let want = match &open {
            None => "click a line to read it whole".to_owned(),
            Some(line) => {
                let mut out = format!("{} {}: {}\nframe {}", line.level.label(), line.target, line.message, line.frame);
                if line.count > 1 {
                    out.push_str(&format!("  ·  ×{}", line.count));
                }
                if let Some(piece) = &line.piece {
                    out.push_str(&format!("\npiece {piece}"));
                }
                out
            }
        };
        if text.0 != want {
            text.0 = want;
        }
    }
    // **The chip is the details pane's, so it is rebuilt with it**: one piece is one chip, and a line about no piece has
    // none.
    for row in &rows {
        commands.entity(row).despawn_related::<Children>();
        commands.entity(row).with_children(|p| {
            if let Some(piece) = open.as_ref().and_then(|line| line.piece.clone()) {
                let mut chip = chrome::chip(
                    p,
                    ConsolePieceChip(piece.clone()),
                    &format!("select {piece}"),
                    chrome::text::LABEL,
                    chrome::ink().accent,
                    chrome::ink().row_bg,
                    chrome::ink().panel_edge,
                );
                chip.observe(select_the_piece);
            }
            for line in &shown {
                let mut text = format!("{} {}: {}", line.level.label(), line.target, line.message);
                if line.count > 1 {
                    text.push_str(&format!("  ×{}", line.count));
                }
                let mut row = chrome::quiet_row(p, console.open == Some(line.id), ConsoleRow(line.id));
                // **One row, cut at the edge** — the whole line is one click away in the details pane, which is what keeps
                // a long message from wrapping into a wall (the journal draws its own lines the same way).
                row.insert(Node { height: Val::Px(chrome::text::LABEL.px() * 1.4), overflow: Overflow::clip(), ..default() });
                row.observe(open_a_line);
                row.with_children(|row| {
                    row.spawn((
                        Text::new(text),
                        TextColor(line.level.ink()),
                        chrome::typed(chrome::text::LABEL),
                        TextLayout::new(Justify::Left, LineBreak::NoWrap),
                    ));
                });
            }
        });
    }
}

/// **A line's one sentence, for a test and for the tally**: `LEVEL target: message`.
pub fn say(line: &Line) -> String {
    format!("{} {}: {}", line.level.label(), line.target, line.message)
}

/// The severity an entry of the journal's kind would draw with, so the two panels name a level alike.
pub fn severity_of(level: Level) -> Severity {
    match level {
        Level::Debug | Level::Info => Severity::Plain,
        Level::Warn => Severity::Primary,
        Level::Error => Severity::Destructive,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ring_trims_from_the_front_and_says_how_many_fell_off() {
        let sink = Sink::default();
        for i in 0..(MAX_ROWS + 100) {
            sink.push(Level::Info, "dark_factory::test", format!("line {i}"), None);
        }
        let ring = sink.read();
        assert_eq!(ring.len(), MAX_ROWS);
        assert_eq!(ring.dropped(), 100);
        assert_eq!(ring.pushed(), (MAX_ROWS + 100) as u64);
        assert_eq!(ring.lines().next().map(|l| l.message.clone()), Some("line 100".to_owned()));
        assert_eq!(ring.lines().last().map(|l| l.message.clone()), Some(format!("line {}", MAX_ROWS + 99)));
    }

    #[test]
    fn a_line_repeated_folds_into_the_one_before_it() {
        let sink = Sink::default();
        for _ in 0..3 {
            sink.push(Level::Warn, "dark_factory::test", "same".to_owned(), None);
        }
        let ring = sink.read();
        assert_eq!(ring.len(), 1, "three identical lines are one row with a tally");
        assert_eq!(ring.lines().next().map(|l| l.count), Some(3));
        assert_eq!(ring.pushed(), 3, "and each arrival is still counted, so the panel redraws");
    }

    #[test]
    fn the_counts_are_per_level_and_a_hidden_level_leaves_the_others() {
        let sink = Sink::default();
        sink.push(Level::Info, "t", "a".to_owned(), None);
        sink.push(Level::Warn, "t", "b".to_owned(), None);
        sink.push(Level::Error, "t", "c".to_owned(), None);
        sink.push(Level::Error, "t", "d".to_owned(), None);
        let ring = sink.read();
        assert_eq!(ring.counts(), [0, 1, 1, 2]);
        let mut hidden = [false; 4];
        hidden[Level::Error.index()] = true;
        let shown = ring.shown(&hidden);
        assert_eq!(shown.len(), 2);
        assert!(shown.iter().all(|line| line.level != Level::Error));
        assert_eq!(shown[0].message, "b", "newest first");
    }

    #[test]
    fn the_chord_is_the_journals_with_shift_and_answers_to_no_other() {
        assert_eq!(toggle_chord().key, KeyCode::KeyE);
        assert!(toggle_chord().modifiers.shift, "Shift is what keeps it off the journal's Cmd+E");
        for (_, _, reserved) in crate::scale_chords::reserved_chords() {
            assert_ne!(reserved, toggle_chord());
        }
        for (_, chord) in crate::command_palette::chords() {
            assert_ne!(chord, toggle_chord());
        }
        assert_ne!(crate::terminal_pane::COLLAPSE_CHORD, toggle_chord());
    }

    #[test]
    fn a_level_names_its_ink_and_its_pill() {
        for level in Level::ALL {
            assert!(!level.label().is_empty());
            assert!(level.index() < 4);
        }
        assert_eq!(Level::Warn.ink(), chrome::ink().accent);
        assert_eq!(Level::Error.ink(), chrome::ink().danger);
    }
}
