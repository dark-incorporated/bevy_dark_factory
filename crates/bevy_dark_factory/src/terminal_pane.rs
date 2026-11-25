//! **The terminal's pane**: a compact dock across the bottom of the editor's window.
//!
//! `Ctrl+\`` raises it and lowers it. It is a dock and not a modal — the point of the feature is that
//! the person keeps editing while the shell is open, so a pane that stole the whole window would be
//! the alt-tab it exists to replace.
//!
//! # What is in it
//!
//! | part | what it is |
//! |---|---|
//! | the grip | a 4 px edge on top, dragged to resize; clamped to `[TERMINAL_MIN_H, TERMINAL_MAX_H]` |
//! | the bar | `Terminal` on the left; one chip per session, then `+` and `×`, on the right |
//! | the body | the shell's rows, **one `Text` per run of one colour** |
//! | the caret | a 2 px block where the shell's cursor is, blinking on `Time<Real>` |
//!
//! **One `Text` per run of one colour in a row, never per cell.** A 200 × 50 pane is ten thousand
//! cells; ten thousand entities is a frame-time cliff, and ten thousand `Text`s is worse. A compiler
//! line is three to six runs, so a row's entity count is bounded by its *colour changes* rather than
//! by its width, and a single-colour row is one entity however long it is.
//!
//! # It survives the door
//!
//! The **pane** is a `Node` and the door sweep takes it (`crate::screen::scene_roots`); the
//! **shells** are a resource ([`crate::terminal::Terminal`]) and nothing sweeps a resource. That is
//! the whole persistence story, and it is the right one: a `cargo build` started on the Map door is
//! still running when the person crosses to Kit, and the pane they raise there draws the same
//! scrollback because it is the same `TerminalScreen`. Nothing has to be excluded from the sweep, so
//! there is no exclusion list to drift.
//!
//! # Narrow is a layout behaviour, not a hope
//!
//! The column count is `floor(body_width / cell_width)` from a measured cell
//! (`crate::terminal_view::measure_the_cell`), and the shell is told every time it changes —
//! `cargo`'s own wrapping then does the work, and the emulator re-wraps its scrollback so a prompt
//! that was one row at 200 columns is drawn as four at 50 rather than clipped. The pane never draws a
//! cell outside its rect: a clipped terminal loses the right-hand end of every error message with no
//! way to scroll to it.

use bevy::input::keyboard::{KeyCode, KeyboardInput};
use bevy::prelude::*;
use bevy::ui::ComputedNode;

use crate::chrome;
use crate::terminal::Terminal;
use crate::terminal_agent::AgentKind;
use crate::terminal_view::CellMetrics;

/// **The pane's resting height**, logical px — about twenty rows, which is a compiler error and the
/// five lines of context around it. A person's drag replaces it for the session.
pub const TERMINAL_DEFAULT_H: f32 = 320.0;

/// **How much of the screen a pane opens at** until the person drags it (#314): a third, clamped to
/// `[TERMINAL_MIN_H, TERMINAL_DEFAULT_H]`, about VS Code's opening share for its panel. With the
/// terminal open on every screen, a fixed 320 px took more than half of a 1280 x 720 window at the
/// editor's `UiScale` (600 px of UI): a third is 200 px there, seven rows of the shipped face's 24 px,
/// and 320 px from a 960 px tall screen up. A quarter was measured first and gave four rows, under
/// the five a terminal is refused below (`measured_cells`).
pub const TERMINAL_WINDOW_SHARE: f32 = 1.0 / 3.0;

/// **The three positions `Cmd+J` cycles through** (#492, the owner: *"I'd like Cmd+J to toggle three position, terminal
/// maximized, minimized, and 1/4 at the bottom."*). A state, not a pair of booleans: the order is the owner's and it is
/// [`Self::next`], so the chord, the status label and their tests all read one machine.
///
/// `Quarter` is a *stated* fraction of the editor's body ([`TERMINAL_QUARTER_SHARE`]), not "what the split last held", so the
/// position is reproducible. A drag after the cycle is the person taking the height back ([`Terminal::set_height`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect)]
pub enum TerminalPlacement {
    /// Covering the window below the menu bar (Shift+`Cmd+J`'s full state, #426).
    Maximised,
    /// Out of the layout, the status band's label saying how to bring it back (#424).
    Collapsed,
    /// Docked across the bottom at a quarter of the body.
    Quarter,
}

impl TerminalPlacement {
    /// **The position `Cmd+J` reaches next**, the owner's order: `Maximised → Collapsed → Quarter → Maximised`.
    pub fn next(self) -> Self {
        match self {
            Self::Maximised => Self::Collapsed,
            Self::Collapsed => Self::Quarter,
            Self::Quarter => Self::Maximised,
        }
    }

    /// What the status bar's tooltip says the next press will do, because a cycle has no state to see (#492).
    pub fn next_press_words(self) -> &'static str {
        match self.next() {
            Self::Maximised => "the next press makes the terminal cover the window",
            Self::Collapsed => "the next press hides the terminal",
            Self::Quarter => "the next press lowers the terminal to a quarter at the bottom",
        }
    }
}

/// **How much of the editor's body the terminal takes in `Quarter`** (#492): a quarter, the owner's fraction. Measured in the
/// editor's own tests against the window below the menu bar, the same room the maximise fills, so the two positions are one measure.
pub const TERMINAL_QUARTER_SHARE: f32 = 0.25;

/// The window's height in the UI's own pixels — what a `Val::Px` is measured in — for
/// [`Terminal::opening_height`].
fn window_ui_height(windows: &Query<&Window, With<bevy::window::PrimaryWindow>>, ui_scale: &UiScale) -> Option<f32> {
    let window = windows.iter().next()?;
    let h = window.resolution.height() / ui_scale.0.max(f32::EPSILON);
    (h > 0.0).then_some(h)
}

/// **The floor**: about eight rows. Below this a prompt plus one line of output does not fit, and a
/// terminal showing a truncated prompt is worse than one that stops shrinking.
pub const TERMINAL_MIN_H: f32 = 96.0;

/// **The ceiling**, so a pane dragged to the top stops being a terminal and becomes the window. It is
/// a fixed count and deliberately **not** a promise of "the map keeps half the window": that is the
/// limitation `a_dock_resize_never_covers_the_viewport` already states about `SplitMin`, and a bound
/// computed from the body's height is a system this slice does not add.
pub const TERMINAL_MAX_H: f32 = 640.0;

/// **Below this many columns the pane is narrow**: the child is told a small width and reflows into
/// it. It is a *reading* of the situation rather than a second layout path — the drawing is
/// identical, only the column count differs — but it is what the compactness test asserts against.
pub const TERMINAL_NARROW_COLS: u16 = 60;

/// The pane's own bar height, logical px. Fixed, like [`chrome`]'s two bars and for the same reason:
/// a bar that changed height with its content would move the shell under the person's hands.
const TERMINAL_BAR_H: f32 = 24.0;

/// The grip's thickness, logical px.
const TERMINAL_GRIP_H: f32 = 4.0;

/// How long the caret stays lit, in seconds — half a second each way, the platform convention.
const CARET_PERIOD: f32 = 1.0;

/// The caret's width as a bar, logical px. Two, because one pixel on a HiDPI surface is invisible and
/// three is a letter.
pub(crate) const CARET_W: f32 = 2.0;

/// An underline caret's height, logical px: the bar's stroke, laid flat.
pub(crate) const CARET_UNDERLINE_H: f32 = CARET_W;

/// The outline of a caret while the keyboard is elsewhere, logical px.
pub(crate) const CARET_OUTLINE_W: f32 = 1.0;

/// **How much of the accent a block caret's fill is.** The owner, 2026-11-06: "should be somewhat
/// transparent, since it blinks slowly". A full-strength block hid its cell for the long lit half of the
/// blink; at half strength the cell shows through, and the character on it ([`TerminalCaretGlyph`]) is
/// drawn in that cell's own ink, so the letter under the caret reads as it does anywhere else.
pub(crate) const CARET_BLOCK_ALPHA: f32 = 0.5;

/// The find box's width, logical px: room for a query of a few words, its count and its close button.
const FIND_BOX_W: f32 = 300.0;

/// **The pane's root.** The one marker every query here keys on.
///
/// `Reflect`, like `chrome::ViewportSlot`, because **BRP is how an agent looks at this editor**: a
/// marker with no reflection is a pane `world.query` cannot find, which makes the terminal invisible to
/// the one tool built for seeing it. That is not a test convenience — `tools/demo/record.py` aims by
/// querying, and a pane nothing can query is a pane no storyboard can press.
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct TerminalPaneRoot;

/// The bar, so the chips can be rebuilt without walking the tree.
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct TerminalBar;

/// The node the rows are rebuilt into.
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct TerminalBody;

/// One drawn row, and which row of the shell's screen it is.
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct TerminalRow(pub usize);

/// The caret's node.
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct TerminalCaret;

/// **The character a solid block caret covers**, drawn on the block in the pane's ground so the word
/// under the caret stays readable (#320), the way VS Code's terminal draws its block. Empty for every
/// other caret.
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct TerminalCaretGlyph;

/// **The find box** over the terminal's body (#321): the row of the query field. The field is
/// bevy_gooey's `text_input`, so its value is the row's `TextInputValue`.
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct TerminalFindBox;

/// The find box's frame, which holds the field, the count and the close button.
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct TerminalFindFrame;

/// The find box's `n of m`.
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct TerminalFindCount;

/// **Find, open or not, and whether its box has the keys** (#321).
///
/// The terminal keeps the keyboard while the box is open ([`TerminalFocus`] holds), so the editor's
/// bindings stay quiet for a query as they do for the shell; inside the terminal the keys go to the box
/// or to the shell, and this says which. A click in the body gives them to the shell and leaves the
/// matches lit; a click in the box or Primary+F gives them back to the box.
#[derive(Resource, Default, Reflect)]
#[reflect(Resource, Default)]
pub struct TerminalFind {
    open: bool,
    in_box: bool,
    /// **The Escape that closed find this frame**, which is the box's and must reach nobody else:
    /// not a screen behind (gooey reads Escape in a field as "back"), and not the shell, where
    /// readline would join it to the next key typed as Meta.
    #[reflect(ignore)]
    closed_this_frame: bool,
}

impl TerminalFind {
    /// Did an Escape in the box close find this frame? `terminal_view::type_keys` drops that frame's
    /// keys rather than send the Escape on to the shell.
    pub fn closed_this_frame(&self) -> bool {
        self.closed_this_frame
    }
}

impl TerminalFind {
    /// Is the find box open?
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Does the find box have the keys, rather than the shell?
    pub fn has_the_keys(&self) -> bool {
        self.open && self.in_box
    }
}

/// **A screen root the terminal docks across the bottom of** (#314): the projects home, the project
/// page and the loading screen put this on their root, and the pane is built as its last child the
/// moment the root is spawned. The editor's frame is not one: it docks the pane above its status band
/// (`spawn_the_pane`). One marker rather than a spawn call in each screen, so a screen added later
/// gets the terminal by naming this, and a screen that forgets it has no half-built pane.
#[derive(Component, Default, Reflect)]
#[reflect(Component, Default)]
pub struct TerminalDock;

/// The grip, dragged to resize the pane.
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct TerminalGrip;

/// A chip in the bar, and the tab it stands for.
#[derive(Component, Clone, Copy)]
pub struct TerminalTabChip(pub usize);

/// The `+` chip: another shell, started in the open project.
#[derive(Component)]
pub struct TerminalNewTab;

/// **The move chip, "Go to `<folder>`": move the drawn shell to the project the editor is pointed at now.**
///
/// The shell is *not* moved when a project opens — a running `cargo build` belongs to the project it
/// was started in, and yanking its working directory would break it. So the pane offers the move
/// instead: it types `cd '<root>'` into the shell, which is a keystroke the person could type
/// themselves, and the shell's own history records it. Offered for a shell only: an agent tab's input is a
/// prompt for the agent, not a shell (feedback-1794230949329).
#[derive(Component)]
pub struct TerminalCdChip;

/// The `×` chip: close the tab being drawn, and with it its shell.
#[derive(Component)]
pub struct TerminalCloseTab;

/// **The status bar's `Terminal Cmd+J` label** (#424): one click raises the pane, as the chord does. The pane starts collapsed on
/// every screen, so this is where a person who has not met the chord finds it; the label says the chord too.
#[derive(Component)]
pub struct TerminalStatusLabel;

/// **What the status bar's terminal label says**: `Terminal`, and the chord that raises and lowers the pane on this platform.
pub fn status_label_words() -> String {
    format!("Terminal {}", chord_words(&crate::chords::chord(crate::chords::Fixed::TerminalCollapse)))
}

/// **A chord as the editor spells it on screen**: in words, `Cmd+J` on a mac and `Ctrl+J` elsewhere, as the status band's hint does
/// (`keys::chord`), and not the glyphs (`⌘J`) `Shortcut::label` draws on a mac.
pub fn chord_words(chord: &bevy_gooey::Shortcut) -> String {
    chord.label_with(bevy_gooey::Platform::current(), bevy_gooey::ChordGlyphs::Words)
}

/// **What the label's tooltip says** (#492): a cycle has no state to see, so the tooltip names what the next press will do.
pub fn status_tooltip_words(placement: TerminalPlacement) -> String {
    format!("{} — {}", placement.next_press_words(), chord_words(&crate::chords::chord(crate::chords::Fixed::TerminalCollapse)))
}

/// **The label's tooltip follows the pane's position** (#492): the chord cycles, so the one place a person can read what the next
/// press means has to be rewritten as the pane moves. It writes only a change.
fn the_label_says_what_the_next_press_does(terminal: Res<Terminal>, mut labels: Query<&mut bevy_gooey::Tooltip, With<TerminalStatusLabel>>) {
    let want = status_tooltip_words(terminal.placement());
    for mut tip in &mut labels {
        if tip.text != want {
            tip.text.clone_from(&want);
        }
    }
}

/// **The `Agent` chip**: another agent tab, in the open project. Shown while the settings choose an
/// agent (`terminal_agent`) and the pane is not the one inviting the first (see `offers_another_agent`).
#[derive(Component)]
pub struct TerminalAgentChip;

/// **Does the bar offer the `Agent` chip?** Only when it adds something: the settings choose an agent and
/// something is already open. An empty pane with an agent chosen already holds the one way in, the
/// `Invoke Agent` button in the middle of it, and a second control in the bar that did the same thing was
/// "an `omp` and then an `Agent` and another `omp`" to the person who asked what the chips were
/// (feedback-1794088663844): two doors to one room.
fn offers_another_agent(terminal: &Terminal) -> bool {
    terminal.agent() != crate::settings::TerminalAgent::Off && !terminal.invites_the_agent()
}

/// **The `Invoke Agent` button**, in the middle of an empty terminal while the settings choose an
/// agent. No agent starts by itself, because a session costs tokens to warm up: the press opens the
/// agent tab, and until then the pane costs nothing.
#[derive(Component)]
pub struct TerminalInvokeButton;

/// The node that holds [`TerminalInvokeButton`] over the body, so the button stands in its middle. Enter
/// presses the button while the terminal holds the keyboard.
#[derive(Component)]
pub struct TerminalInvite;

/// **The drawn agent tab's status chip**: `omp connected · 22 tools`, or what is still checking or
/// failed. A click brings the self-test card back, and asks a failed omp test again.
#[derive(Component)]
pub struct TerminalAgentStatusChip;

/// **The self-test card** over the terminal's top right, while an agent tab's checks run and for a
/// moment after.
#[derive(Component)]
pub struct TerminalAgentCard;

/// **A row of the self-test card**, and which of the four checks it draws, so a theme edit can repaint
/// its ink without rebuilding the card: `None` is the card's own heading.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct TerminalAgentCardRow(pub Option<usize>);

/// What both of the agent's LEDs are called, beside the dot and before its state word (`Agent off`, `Agent busy`, `Agent live`,
/// `Agent failed`). It is also the LED's accessible name, so a reader of the screen hears what is live and not only that
/// something is.
const AGENT_LED_LABEL: &str = "Agent";

/// **The agent's LED in the bar** (feedback-1794230412409: *"where are the LEDs?"*), on the status chip's row and before it, for as
/// long as the settings choose an agent: dark while no agent tab is drawn, and [`crate::terminal_agent::AgentPost::led`] in light
/// once there is one. It stays when the self-test card goes.
#[derive(Component)]
pub struct TerminalAgentLed;

/// **The agent's LED at the foot of the self-test card**: the verdict the four checks above it add up to.
#[derive(Component)]
pub struct TerminalAgentCardLed;

/// **An LED of this pane, and the type role its words are drawn at** (#422): the bar's at a chip's ([`chrome::text::CONTROL`]), the
/// card's at a row's ([`chrome::text::LABEL`]). `bevy_gooey::led` sizes its words from the skin's own text size, which is not the
/// editor's type scale, so [`size_the_leds`] sets them from the role, at the person's text size.
#[derive(Component, Clone, Copy)]
struct PaneLed(chrome::text::Role);

/// The LED dot's side as a share of its word's size, which is `bevy_gooey`'s own (`LED_TEXT_SIZES`): the dot keeps its relation to the
/// word when the word takes the editor's size.
const LED_DOT_SIZES: f32 = 1.1;

/// The least the card is wide, logical pixels: wide enough for the first check's row (`OK Tools server: dark_factory_mcp
/// on port 15702`), so the card does not start narrower than that and grow row by row.
///
/// **The card takes the width its longest row needs** (feedback-1794089256357): from this up to the terminal's own width (`max_width` 100%), so every row is one line while there is
/// room for it, and only in a terminal narrower than a row does a row wrap, as any text does. It was a fixed 320 px, and a longer row broke onto a second line.
const AGENT_CARD_MIN_W: f32 = 320.0;

/// **The terminal has the keyboard**, which is the one state this feature must not get wrong.
///
/// Written by this module, read by [`crate::editor::sense_context`] — which folds it into the same
/// `typing` flag a text field sets, so **no editor binding fires while a shell has the keys**. A
/// shell that typed `d` and moved the camera would be unusable, and `Live(Context::Typing, …)` is
/// the single gate the whole census already respects; a second swallow written here would be the
/// two-paths failure this crate spends its refusals on.
///
/// Some chords survive, checked in `terminal_view::type_keys` *before* the swallow because
/// they are read there rather than through the census: `Ctrl+\`` closes, and Cmd+V, Cmd+C and Cmd+A (or
/// Ctrl+Shift+V, C and A) paste, copy and select all ([`crate::terminal_view::clipboard_chord`]).
#[derive(Resource, Default, Reflect)]
#[reflect(Resource, Default)]
pub struct TerminalFocus(pub bool);

impl TerminalFocus {
    /// Take the keyboard.
    pub fn take(&mut self) {
        self.0 = true;
    }

    /// Give it back to the editor.
    pub fn release(&mut self) {
        self.0 = false;
    }

    /// Does the terminal own the keys this frame?
    pub fn holds(&self) -> bool {
        self.0
    }

    /// **Is `chord` the shell's this frame, and not the window's?** (#318)
    ///
    /// While the shell has the keyboard, a chord on **Ctrl** is a control code it reads — Ctrl+D is
    /// end-of-file, Ctrl+K kills to the end of the line, Ctrl+R searches history — so a window chord
    /// spelled on Ctrl (every primary chord off a mac) is the shell's, the way VS Code sends chords to
    /// its terminal unless they are on its short skip list. A chord on **Command** never reaches a
    /// shell on a mac, so it stays the window's there. The pane's own chords (Primary+`, Primary+J,
    /// Primary+Shift+C and V) do not ask: they are read before the shell sees anything.
    pub fn owns(&self, chord: &bevy_gooey::Shortcut) -> bool {
        self.0 && chord.modifiers.ctrl
    }
}

/// The pane's own working state: the size it last asked for, and the caret's blink.
#[derive(Resource, Default, Reflect)]
#[reflect(Resource, Default)]
pub struct TerminalPane {
    cols: u16,
    rows: u16,
    /// Seconds into the caret's blink cycle.
    blink: f32,
    /// Where a drag began, in the body's cell coordinates, while a selection is being made.
    drag_from: Option<(usize, usize)>,
    /// **A button held down that the program was told about** (#313): while it holds, a drag is the
    /// program's motion and not a selection, and the release goes to the program too.
    #[reflect(ignore)]
    mouse_down: Option<MouseDown>,
}

/// One reported press, followed until its release.
#[derive(Clone, Copy, Debug, Default)]
struct MouseDown {
    button: Option<crate::terminal::input::MouseButton>,
    /// The cell the press was reported at; a drag is measured from it.
    from: (usize, usize),
    /// The cell the last report named, so motion inside one cell is not reported twice.
    last: (usize, usize),
}

impl TerminalPane {
    /// The cell count the pane last asked the shell for.
    pub fn cells(&self) -> (u16, u16) {
        (self.cols, self.rows)
    }

    /// Record the cell count this frame's layout produced.
    pub fn set_cells(&mut self, cols: u16, rows: u16) {
        self.cols = cols;
        self.rows = rows;
    }

    /// Where a selection drag started, if one is in progress.
    pub fn drag_from(&self) -> Option<(usize, usize)> {
        self.drag_from
    }
}

/// Registers the pane, its painting systems, and the chords that raise and lower it.
pub struct TerminalPanePlugin;

impl Plugin for TerminalPanePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TerminalFocus>()
            .init_resource::<TerminalPane>()
            .init_resource::<TerminalFind>()
            // **Registered, so BRP can name them.** `world.list_resources` and `world.get_resources`
            // only see types the app has reflected, and an agent reading this editor's state is the
            // reason `bevy_fairy` exists. A `Terminal` nothing can read is a shell an agent can start
            // and never observe.
            .register_type::<TerminalFocus>()
            .register_type::<TerminalPane>()
            .register_type::<TerminalPlacement>()
            .register_type::<TerminalPaneRoot>()
            .register_type::<TerminalRow>()
            .register_type::<TerminalBar>()
            .register_type::<TerminalBody>()
            .register_type::<TerminalCaret>()
            .register_type::<TerminalGrip>()
            .register_type::<TerminalDock>()
            .register_type::<TerminalCaretGlyph>()
            .register_type::<TerminalFind>()
            .register_type::<TerminalFindBox>()
            .register_type::<TerminalFindFrame>()
            .register_type::<TerminalFindCount>()
            // **The editor's frame docks it above the status band** (`chrome::spawn_frame` runs
            // `OnEnter(Editor)`); every other screen names [`TerminalDock`] on its root and
            // [`dock_on_every_screen`] builds it there (#314).
            .add_systems(
                OnEnter(crate::screen::Screen::Editor),
                spawn_the_pane.after(chrome::FrameSystems),
            )
            .add_observer(dock_on_every_screen)
            // **Between Bevy's input and its focused-input dispatch**, so this frame's presses and this
            // frame's dispatch already know the shell has the keyboard (#316).
            .add_systems(
                PreUpdate,
                (a_dialog_takes_the_keyboard, forget_a_find_box_that_went, the_find_box_keys, hold_the_keyboard)
                    .chain()
                    .after(bevy::input::InputSystems)
                    .before(bevy::input_focus::InputFocusSystems::Dispatch),
            )
            // **The keyboard goes back when the door closes.** A focus flag that outlived its pane
            // would leave every editor binding refused with no shell on screen to explain it — the
            // bug `leaving_a_tab_puts_the_name_prompt_down` exists for, one level up.
            .add_systems(OnExit(crate::screen::Screen::Editor), give_the_keyboard_back)
            .add_systems(
                Update,
                (
                    blink_the_caret,
                    fit_the_pane_to_the_window,
                    step_aside_for_the_key_overlay,
                    count_the_matches,
                    keep_the_agent_on_screen,
                    measure_the_bar,
                )
                    .run_if(the_pane_is_up),
            )
            // **The agent tab's ink is the running skin's** (UI-02, #210, slice 6a). It runs after every
            // spawn this frame, in `PostUpdate` — the one place a `TextColor` written here is what the
            // frame draws — so a card built a moment ago and a card built before a theme edit both land
            // on the skin's colours, and a rebuild never shows the editor's own darks for a frame.
            .add_systems(PostUpdate, dress_the_agent_tab.run_if(the_pane_is_up))
            .add_systems(Update, size_the_leds.after(bevy_gooey::GooeySystems::Style).run_if(the_pane_is_up))
            .add_systems(Update, quiet_the_row_leds.run_if(the_pane_is_up))
            // The two chords that are the pane's own. Read from `ButtonInput` here rather than
            // through `keys::BINDINGS`: the census is the table of what a *tab* does, and a chord
            // that belongs to the window is the case `command_palette::chords` already set the
            // precedent for. `no_context_gains_a_key` is therefore untouched.
            .add_systems(
                Update,
                (
                    toggle_on_the_chord,
                    collapse_on_the_chord,
                    maximise_on_the_chord,
                    apply_the_maximise,
                    close_the_tab_on_its_chord,
                    open_find_on_its_chord,
                    clear_on_its_chord,
                    enter_invokes_the_agent,
                )
                    .in_set(crate::terminal_view::TerminalChordSet),
            )
            // **The pane's geometry is the host's once gooey has styled it** (see [`host_the_pane_geometry`]).
            .add_systems(Update, host_the_pane_geometry.after(bevy_gooey::GooeySystems::Style))
            .add_systems(Update, resume_the_agent_that_was_running)
            // **The editor development agent**: its chord, and the tab a relaunch asks to be restored (`--restore-dev-agent`).
            .add_systems(Update, (the_chord_opens_the_editor_dev_agent, restore_the_editor_dev_agent))
            // **The status bar carries the terminal's label, and the choice a person makes is kept** (#424).
            .add_systems(
                OnEnter(crate::screen::Screen::Editor),
                spawn_the_status_label.after(chrome::FrameSystems).after(chrome::spawn_status_band),
            )
            .add_systems(Update, keep_the_pane_choice)
            // **The status label's tooltip names what the next press does** (#492), because a cycle has no state to see.
            .add_systems(Update, the_label_says_what_the_next_press_does.after(crate::terminal_view::TerminalChordSet))
            .add_observer(the_label_flips_the_pane)
            .add_observer(a_click_takes_the_keyboard)
            .add_observer(a_click_elsewhere_gives_it_back)
            .add_observer(a_drag_begins)
            .add_observer(a_drag_extends)
            .add_observer(a_drag_ends)
            .add_observer(a_primary_click_opens_a_link)
            .add_observer(a_press_reaches_the_program)
            .add_observer(a_release_reaches_the_program)
            .add_observer(the_wheel_scrolls_the_scrollback)
            .add_observer(the_grip_resizes)
            .add_observer(a_chip_selects_its_tab)
            .add_observer(a_chip_adds_a_tab)
            .add_observer(a_chip_opens_an_agent)
            .add_observer(the_button_invokes_the_agent)
            .add_observer(a_status_chip_reruns_the_test)
            .add_observer(a_chip_moves_the_shell)
            .add_observer(a_chip_closes_a_tab);
    }
}

/// Is a pane on screen this frame? Every per-frame system asks it, so a lowered terminal costs one
/// empty query.
fn the_pane_is_up(panes: Query<(), With<TerminalPaneRoot>>) -> bool {
    !panes.is_empty()
}

/// **Build the pane**, as the body's last row.
///
/// A row of the frame's own column rather than a floating layer: the body already stacks the docks
/// and the viewport, so "docked across the bottom" is one child appended to it. A `GlobalZIndex`
/// layer would paint over the status band and the docks' scrollbars, and z-order is the dialogs' — see
/// [`chrome::MODAL_Z`].
fn spawn_the_pane(
    mut commands: Commands,
    frame: Option<Res<chrome::Frame>>,
    frame_children: Query<&Children>,
    terminal: Res<Terminal>,
    mut focus: ResMut<TerminalFocus>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    ui_scale: Res<UiScale>,
) {
    let Some(frame) = frame else {
        return;
    };
    // **A pane belongs to a screen.** The door sweep takes the old root; this spawns the new one, and
    // the shells in `Terminal` are untouched — that is the whole of what "survives the door" means.
    //
    // **It is parented to the frame's ROOT, not its body, and that is not a detail.**
    // `chrome::Frame::body` is the `flex_direction: Row` that holds the two docks and the viewport, so a
    // child of it is a *row sibling*: the pane was laid out 2560 px wide and pushed off the right edge
    // of a 2560 px window, over the inspector and the map. Measured on the released 0.2.8 build on
    // `mac_air`: size 2560 x 768, centre x 1718, spanning 438..2998. The root is the `Column` that
    // already stacks the chrome bar, the door strip, the body and the status band, so a bottom dock is
    // a row between `body` and `status` — which is what this inserts.
    //
    // **It arrives raised**, unless the person collapsed it (#314, #315): the terminal is the work's
    // centerpiece, open on every screen from the application's first frame, and [`Terminal`] carries
    // a collapse from one screen to the next. The shells are not touched here: a shell that was running
    // when the screen changed is still running, and the new pane draws its scrollback.
    // **The status band's index**, so the pane lands between the body and the band. Found by asking the
    // frame's own children rather than assuming "last but one": a frame that gained a row would
    // otherwise put the terminal in the wrong place, silently.
    let band_at = frame_children
        .get(frame.root)
        .map(|kids| {
            kids.iter()
                .position(|child| child == frame.status)
                .unwrap_or(usize::MAX)
        })
        .unwrap_or(usize::MAX);
    let height = terminal.opening_height(window_ui_height(&windows, &ui_scale));
    build_the_pane(&mut commands, frame.root, height, &terminal, band_at);
    // A door that opens never has the terminal's keys: a lowered pane that still owned the keyboard
    // would be a dead editor with nothing on screen saying why.
    focus.release();
}

/// **The pane on a screen that is not the editor**, as the [`TerminalDock`] root's last child: under
/// the projects home, the project page and the loading card (#314). Like a door, a screen that opens
/// does not have the terminal's keys: the screen's own keyboard works until the person clicks into
/// the shell or presses the chord.
fn dock_on_every_screen(
    dock: On<Add<TerminalDock>>,
    mut commands: Commands,
    terminal: Res<Terminal>,
    mut focus: ResMut<TerminalFocus>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    ui_scale: Res<UiScale>,
) {
    let height = terminal.opening_height(window_ui_height(&windows, &ui_scale));
    build_the_pane(&mut commands, dock.entity, height, &terminal, usize::MAX);
    focus.release();
}

/// Build the pane under `parent`, returning its root. **The one place a pane is made**, raised unless
/// the person collapsed it.
fn build_the_pane(
    commands: &mut Commands,
    parent: Entity,
    height: f32,
    terminal: &Terminal,
    // The index in `parent`'s children the pane goes at — the status band's, so the band stays last.
    before: usize,
) -> Entity {
    let root = commands
        .spawn((
            // The pane root: a gooey sized column the window wide, `height` tall (at least `TERMINAL_MIN_H`), that never shrinks (the height is the person's, and a body that
            // shrank it would turn a remembered 320 px into whatever the docks left over), with a 1 px edge on top. The initial lengths are gooey's (`Sizing`); from the first
            // frame on the host owns them (`HostGeometry`, added by [`host_the_pane_geometry`]), because the grip, the window's fit and the maximise write `height` and the
            // maximise writes `position_type` and the insets. **Collapsed is `Display::None`, written by the host's systems and followed by gooey (`ShownFollow`)**, and it
            // starts closed when the person left it so: a hidden-but-laid-out pane would still take its height out of the screen.
            bevy_gooey::sized(bevy_gooey::Sizing {
                width: Some(bevy_gooey::Extent::Percent(100.0)),
                height: Some(bevy_gooey::Extent::Px(height)),
                min_height: Some(bevy_gooey::Extent::Px(TERMINAL_MIN_H)),
                ..default()
            }),
            bevy_gooey::RowLayout { shrink: Some(0.0), ..default() },
            bevy_gooey::BorderPx(bevy_gooey::Sides { top: Some(crate::style::metrics().layout.edge_w), ..default() }),
            bevy_gooey::InkFollow,
            BackgroundColor(chrome::ink().panel_bg),
            BorderColor::all(chrome::ink().panel_edge),
            // **`Pickable::default()`, not `IGNORE`.** The body has to be clickable to take the
            // keyboard and draggable to select — and `IGNORE` skips an entity *and everything below
            // it*, the trap `chrome::spawn_frame` records for the shell split's divider.
            Pickable::default(),
            TerminalPaneRoot,
            // **Its layer among its siblings** (#426): the pane stands where the frame puts it. While it covers the window it is a `GlobalZIndex` layer
            // over the map's overlays ([`apply_the_maximise`], #435).
            ZIndex(0),
            Name::new("terminal"),
        ))
        .id();
    if terminal.is_collapsed() {
        commands.entity(root).insert(bevy_gooey::ShownFollow::closed());
    } else {
        commands.entity(root).insert(bevy_gooey::ShownFollow);
    }

    let grip = commands
        .spawn((
            // The grip: a gooey sized box the pane wide and `TERMINAL_GRIP_H` tall that never shrinks (the column's stretch gives it the width).
            bevy_gooey::sized(bevy_gooey::Sizing { width: Some(bevy_gooey::Extent::Percent(100.0)), height: Some(bevy_gooey::Extent::Px(TERMINAL_GRIP_H)), ..default() }),
            bevy_gooey::TextFlow { shrink: Some(0.0), ..default() },
            BackgroundColor(chrome::ink().panel_edge),
            TerminalGrip,
            Pickable::default(),
            Name::new("terminal grip"),
        ))
        .id();

    let bar = commands
        .spawn((
            // The bar: a gooey row the pane wide and `TERMINAL_BAR_H` tall, its parts centred, its gap and its inset from each end the sheet's `.terminal-bar`, never shrinking,
            // and clipping what does not fit (#497: the window wins; the Invoke button stays inside it).
            bevy_gooey::row_with(0.0, bevy_gooey::RowLayout { align: Some(AlignItems::Center), shrink: Some(0.0), ..default() }),
            bevy_gooey::StyleClass::of("terminal-bar"),
            bevy_gooey::Sizing { width: Some(bevy_gooey::Extent::Percent(100.0)), height: Some(bevy_gooey::Extent::Px(TERMINAL_BAR_H)), clip: Some(true), ..default() },
            BackgroundColor(chrome::ink().bar_bg),
            TerminalBar,
            Pickable::default(),
            Name::new("terminal bar"),
        ))
        .id();

    let menu = build_the_menu(commands);
    let body_node = commands
        .spawn((
            // The body: a gooey sized column that grows into what the bar and the grip leave (no least height), **clipped, and not a `ScrollArea`**. The scrollback belongs to the
            // emulator (`TerminalScreen::scroll`): a layout scroll would move the *drawn* rows while the shell's history lives in the grid, which is two answers to one question;
            // `clip` is what stops a row painting over the status band. `BODY_INSET` in on each side (neither is a step of the scale).
            bevy_gooey::sized(bevy_gooey::Sizing { width: Some(bevy_gooey::Extent::Percent(100.0)), min_height: Some(bevy_gooey::Extent::Px(0.0)), clip: Some(true), ..default() }),
            bevy_gooey::RowLayout { direction: Some(FlexDirection::Column), ..default() },
            bevy_gooey::TextFlow { grow: Some(1.0), ..default() },
            bevy_gooey::Spacing {
                padding: bevy_gooey::Sides {
                    top: Some(bevy_gooey::Space::Px(crate::terminal_view::body_inset().y)),
                    right: Some(bevy_gooey::Space::Px(crate::terminal_view::body_inset().x)),
                    bottom: Some(bevy_gooey::Space::Px(crate::terminal_view::body_inset().y)),
                    left: Some(bevy_gooey::Space::Px(crate::terminal_view::body_inset().x)),
                },
                ..default()
            },
            BackgroundColor(chrome::ink().panel_bg),
            TerminalBody,
            // **A right-click offers the three things a terminal's clipboard needs.** A shell with no
            // way to copy its output is a toy; gooey's `context_menu` is the editor's one menu
            // machinery, so this is the same widget every other menu uses.
            bevy_gooey::ContextMenu(menu),
            Pickable::default(),
            // **A key the shell is reading goes no further** (#316): with the keyboard in the shell,
            // [`hold_the_keyboard`] puts the screen's input focus on this body, and this stops every
            // focused key here, so Bevy's Tab navigation (an observer on the window) and a field's own
            // key handling never see it.
            bevy_gooey::observe(
                |mut key: On<bevy::input_focus::FocusedInput<bevy::input::keyboard::KeyboardInput>>| {
                    key.propagate(false);
                },
            ),
            Name::new("terminal body"),
        ))
        .id();

    let caret = commands
        .spawn((
            // The caret is not a panel: it marks one cell of the shell's grid, exactly as a list's scrollbar overlay sits on its viewport, and `draw_the_terminal` moves it, writing its own
            // left, top, width and height each frame (`HostGeometry`). `placed` is `Pickable::IGNORE`, which the caret is.
            bevy_gooey::placed(UiRect::all(Val::Auto)),
            bevy_gooey::HostGeometry,
            BackgroundColor(chrome::ink().accent),
            BorderColor::all(chrome::ink().accent),
            // **Above the rows, its siblings** (#320): a block caret is a tint over its cell with the
            // cell's character drawn on it, in that cell's own ink ([`TerminalCaretGlyph`]), so the
            // letter stays crisp through the tint rather than being dimmed by it.
            ZIndex(1),
            TerminalCaret,
            Name::new("terminal caret"),
            children![(
                Text::new(String::new()),
                TextColor(chrome::ink().text),
                TextLayout::no_wrap(),
                crate::terminal_view::terminal_font(1.0),
                TerminalCaretGlyph,
                Pickable::IGNORE,
                Name::new("terminal caret glyph"),
            )],
        ))
        .id();

    commands.entity(body_node).add_child(caret);
    commands.entity(root).add_children(&[grip, bar, body_node]);
    // **Above the status band, below the body.** `insert_children` at the band's index keeps the band
    // at the foot of the window, where it has been since the frame was written; an `add_child` would
    // put the terminal under it. `insert_children` clamps the index, so `usize::MAX` is "last".
    commands.entity(parent).insert_children(before, &[root]);
    fill_the_bar(commands, bar, terminal);
    root
}

/// **The terminal's context menu**: Copy, Paste, Clear.
///
/// Built once per pane and named by the body's [`bevy_gooey::ContextMenu`], so a right-click anywhere in
/// the scrollback opens it at the pointer — gooey's own behaviour, not a second implementation.
///
/// **Copy takes the selection, and says so when there is none** by staying disabled rather than
/// silently doing nothing: a menu item that looks live and does nothing is the failure mode
/// `commands.rs`'s `enabled` exists to prevent. It reads the selection when the menu opens, which is
/// what `On<Add<ContextTarget>>` is for.
fn build_the_menu(commands: &mut Commands) -> Entity {
    commands
        .spawn(bevy_gooey::context_menu(children![
            (
                bevy_gooey::menu_item("Copy"),
                bevy_gooey::observe(copy_the_selection),
            ),
            (
                bevy_gooey::menu_item("Paste"),
                bevy_gooey::observe(paste_the_clipboard),
            ),
            (
                bevy_gooey::menu_item("Clear"),
                bevy_gooey::observe(clear_the_scrollback),
            ),
        ]))
        .id()
}

/// Copy the selection to the system clipboard.
fn copy_the_selection(
    _click: On<bevy_gooey::Activate>,
    terminal: Option<Res<Terminal>>,
) {
    let Some(terminal) = terminal else {
        return;
    };
    let Some(tab) = terminal.tab() else {
        return;
    };
    let Some(text) = tab.screen().selection_text() else {
        return;
    };
    if let Ok(mut clipboard) = arboard::Clipboard::new() {
        let _ = clipboard.set_text(text);
    }
}

/// Paste the clipboard, bracketed when the child asked for it.
fn paste_the_clipboard(
    _click: On<bevy_gooey::Activate>,
    mut terminal: ResMut<Terminal>,
) {
    let Ok(mut clipboard) = arboard::Clipboard::new() else {
        return;
    };
    let Ok(text) = clipboard.get_text() else {
        return;
    };
    let bracketed = terminal
        .tab()
        .is_some_and(|tab| tab.screen().bracketed_paste());
    let mut bytes = Vec::new();
    if bracketed {
        bytes.extend_from_slice(b"\x1b[200~");
        bytes.extend_from_slice(text.as_bytes());
        bytes.extend_from_slice(b"\x1b[201~");
    } else {
        bytes.extend_from_slice(text.as_bytes());
    }
    terminal.write(&bytes);
}

/// **Clear the scrollback** — the shell's history, not its screen: the pane keeps showing what is
/// live, and the lines above stop costing anything to keep.
fn clear_the_scrollback(_click: On<bevy_gooey::Activate>, mut terminal: ResMut<Terminal>) {
    if let Some(tab) = terminal.tab_mut() {
        tab.screen_mut().clear_scrollback();
    }
}

/// What the `x` chip's tooltip says, before its chord.
const TIP_CLOSE_TAB: &str = "Close this session and end its shell";

/// **The folder a new session starts in, by name**: what the bar's tooltips and the move chip call the open project.
fn project_folder(terminal: &Terminal) -> String {
    terminal
        .next_cwd()
        .file_name()
        .map_or_else(|| "the open project".to_owned(), |name| name.to_string_lossy().into_owned())
}

/// The most characters a tab's name takes in the bar (#448): a longer name is cut and ends in an ellipsis, so one tab, named by a whole path (a custom agent's
/// command), cannot crowd the chips beside it. The full name stays in the tab's tooltip.
pub const TAB_NAME_MAX_CHARS: usize = 16;

/// **Each tab's words** (#423): the program that runs in it, and a number from the second of the same name on (`sh`, `sh 2`), so
/// no two tabs of the bar read alike; a name longer than [`TAB_NAME_MAX_CHARS`] is cut with an ellipsis (#448).
pub fn tab_names(labels: &[&str]) -> Vec<String> {
    labels
        .iter()
        .enumerate()
        .map(|(at, label)| {
            let name = match labels[..at].iter().filter(|earlier| *earlier == label).count() {
                0 => (*label).to_owned(),
                before => format!("{label} {}", before + 1),
            };
            if name.chars().count() > TAB_NAME_MAX_CHARS {
                let kept: String = name.chars().take(TAB_NAME_MAX_CHARS - 1).collect();
                format!("{kept}\u{2026}")
            } else {
                name
            }
        })
        .collect()
}

/// **What the bar would hold with all the room it wants** (#423), before it is told how much it has: the tabs' words, which one is
/// drawn, the move chip's words when it is offered, whether the agent's LED stands in it, and whether `Start agent` is offered.
#[derive(Clone, Debug, PartialEq)]
pub struct BarWants {
    pub tabs: Vec<String>,
    pub active: usize,
    pub go_to: Option<String>,
    pub led: bool,
    pub start_agent: bool,
}

/// **What the bar holds in the room it has** (#423): the tabs drawn (by index), and which of the optional items stay.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BarPlan {
    pub tabs: Vec<usize>,
    pub go_to: bool,
    pub start_agent: bool,
    pub led_name: bool,
}

/// **The order a narrow bar gives things up, first to last** (#423), so a bar that is too narrow loses items by a rule a person can
/// learn and never clips one in half: the `Go to` chip, then `Start agent`, then the LED's name (its state word stays), then the tabs
/// that are not the one drawn, last tab first. The heading, the drawn tab, the LED's word, `+` and `x` always stay.
pub const BAR_GIVES_UP: [&str; 4] = ["the Go to chip", "the Start agent chip", "the LED's name", "the tabs that are not drawn"];

/// A glyph's advance as a share of its font size: the shipped face is monospace, so a word's width is its characters times this.
const GLYPH_ADVANCE: f32 = 0.6;
/// The words of the LED's longest state, `failed`, in characters.
const LED_WORD_CHARS: usize = 6;
/// How many more logical px of room the bar must measure before it is measured again: a window being dragged is not a redraw a frame.
const BAR_ROOM_STEP: f32 = 16.0;

/// One word's width at a control's size, logical px.
fn word_width(chars: usize) -> f32 {
    chars as f32 * GLYPH_ADVANCE * chrome::text::CONTROL.px() * chrome::text_scale()
}

/// **A chip's inset across, logical px** — the `.chrome-chip` rule's, as the bar's plan estimates it. The plan is an estimate from the face's
/// advance in any case; a sheet edit that moves the chip's inset moves the drawn chips at once and this estimate with the next build.
const CHIP_INSET_X: f32 = 6.0;

/// One chip's width: its word, its inset and the edge on both sides, and the bar's gap after it.
fn chip_width(chars: usize) -> f32 {
    word_width(chars) + 2.0 * (CHIP_INSET_X + crate::style::metrics().layout.edge_w) + chrome::space(chrome::STEP_TIGHT)
}

/// **Decide what a bar of `room` logical px holds** (#423); `None` is a bar not yet laid out, which holds everything. The widths are
/// estimated from the face's advance (`GLYPH_ADVANCE`) so the plan is a pure function of what is wanted and how much room there is.
pub fn bar_plan(room: Option<f32>, wants: &BarWants) -> BarPlan {
    let mut plan = BarPlan { tabs: (0..wants.tabs.len()).collect(), go_to: wants.go_to.is_some(), start_agent: wants.start_agent, led_name: wants.led };
    let Some(room) = room else {
        return plan;
    };
    let width = |plan: &BarPlan| -> f32 {
        let tight = chrome::space(chrome::STEP_TIGHT);
        let always = 2.0 * chrome::space(chrome::STEP_PAD) + word_width("Terminal".len()) + tight + chip_width(1) * 2.0;
        let led = if wants.led {
            tight + word_width(LED_WORD_CHARS) + word_width(1) * 2.0 + if plan.led_name { word_width(AGENT_LED_LABEL.len()) } else { 0.0 }
        } else {
            0.0
        };
        let tabs: f32 = plan.tabs.iter().map(|at| chip_width(wants.tabs[*at].chars().count())).sum();
        let go_to = if plan.go_to { wants.go_to.as_ref().map_or(0.0, |words| chip_width(words.chars().count())) } else { 0.0 };
        let start = if plan.start_agent { chip_width(START_AGENT.chars().count()) } else { 0.0 };
        always + led + tabs + go_to + start
    };
    // The first three are one step each; the tabs go one at a time.
    if width(&plan) > room {
        plan.go_to = false;
    }
    if width(&plan) > room {
        plan.start_agent = false;
    }
    if width(&plan) > room {
        plan.led_name = false;
    }
    while width(&plan) > room {
        match plan.tabs.iter().rposition(|at| *at != wants.active) {
            Some(last) => {
                plan.tabs.remove(last);
            }
            None => break,
        }
    }
    plan
}

/// What the bar wants, read from the terminal: the one place the bar's contents are decided.
fn bar_wants(terminal: &Terminal) -> BarWants {
    let labels: Vec<&str> = terminal.tabs().iter().map(|tab| tab.label()).collect();
    let want = terminal.next_cwd();
    let go_to = terminal
        .tab()
        .is_some_and(|tab| tab.agent().is_none() && tab.wants_cd_to(&want))
        .then(|| format!("Go to {}", project_folder(terminal)));
    let post = terminal.tab().and_then(|tab| tab.agent());
    BarWants {
        tabs: tab_names(&labels),
        active: terminal.active(),
        go_to,
        led: post.is_some() || terminal.agent() != crate::settings::TerminalAgent::Off,
        start_agent: offers_another_agent(terminal),
    }
}

/// What the bar's `Start agent` chip says, and the invitation's button with it: one word for one action.
const START_AGENT: &str = "Start agent";

/// **The bar** (#423): the pane's name, then its tabs (named for the program in each), then on the right the agent's LED, `Start
/// agent`, `+` and `x`. Every item says what it is without a hover: a tab is a tab, the LED is one light with one word, a button is
/// a verb. A failed agent's reason is the LED's tooltip and the card's, never a chip here.
fn fill_the_bar(commands: &mut Commands, bar: Entity, terminal: &Terminal) {
    let wants = bar_wants(terminal);
    let plan = bar_plan(terminal.bar_room(), &wants);
    commands.entity(bar).despawn_children();
    commands.entity(bar).with_children(|bar| {
        chrome::list_heading(bar, "Terminal");

        let folder = project_folder(terminal);
        // **A tab is a tab, and a button is a button** (#423): the drawn tab is filled and edged in the accent, the others are flat and
        // edgeless, and every button (`Go to`, `Start agent`, `+`, `x`) keeps its outline, so the two shapes are not mistaken.
        // `chrome::chip` carries `RowRest`, so rest / hover / pressed are painted by `style_list_rows` without this pane knowing what
        // hover looks like.
        for index in &plan.tabs {
            let tab = &terminal.tabs()[*index];
            let active = *index == terminal.active();
            let ink = if active { chrome::ink().accent } else { chrome::ink().text };
            let fill = if active { chrome::ink().row_selected } else { chrome::ink().bar_bg };
            let edge = if active { chrome::ink().accent } else { Color::NONE };
            let kind = if tab.agent().is_some() { "agent" } else { "shell" };
            chrome::chip(bar, TerminalTabChip(*index), &wants.tabs[*index], chrome::text::CONTROL, ink, fill, edge)
                .insert(bevy_gooey::Tooltip::new(format!("Show this {kind} session: {} is running in it", tab.label())));
        }
        // **The move chip only when it would do something, and only for a shell.** It names the folder it would move to, so a
        // person reads where it goes rather than trusting a verb, and it says "Go to" and not `cd`: what it types is a shell
        // command, and the tooltip shows it. An agent tab is not a shell: `cd '<root>'` typed into the agent's prompt is a message
        // to the agent, so the chip is never offered there (feedback-1794230949329: *"'cd lavender_hills' doesn't make a lot of
        // sense"*). With no open project, or a shell already in it, there is nothing to offer and no chip.
        if plan.go_to {
            let want = terminal.next_cwd();
            chrome::chip(
                bar,
                TerminalCdChip,
                &format!("Go to {folder}"),
                chrome::text::CONTROL,
                chrome::ink().accent,
                chrome::ink().bar_bg,
                chrome::ink().panel_edge,
            )
            .insert(bevy_gooey::Tooltip::new(format!(
                "Move this shell into the open project: types cd {} for you",
                crate::terminal_agent::shell_quote(&want.to_string_lossy())
            )));
        }
        // A spacer, so the controls sit on the right the way every other bar in this editor puts them.
        bar.spawn(bevy_gooey::spacer());
        // **The agent's LED**: one LED with one word (`off`, `busy`, `live`, `failed`) for as long as the settings choose an agent,
        // dark while no agent tab is drawn. Once there is a tab the LED is also the control that brings the self test back, and its
        // tooltip carries the sentence and any failure's reason: no second chip repeats it in words (#423).
        let post = terminal.tab().and_then(|tab| tab.agent());
        if wants.led {
            let state = post.map_or(bevy_gooey::LedState::Off, |post| post.led());
            let name = if plan.led_name { AGENT_LED_LABEL } else { "" };
            match post {
                Some(post) => {
                    bar.spawn((
                        (bevy_gooey::row_with(0.0, bevy_gooey::RowLayout { align: Some(AlignItems::Center), ..default() }), chrome::Gap::Tight.class()),
                        Pickable::default(),
                        TerminalAgentStatusChip,
                        bevy_gooey::Tooltip::new(post.led_tooltip()),
                        Name::new("terminal agent status"),
                    ))
                    .with_children(|status| {
                        status.spawn((bevy_gooey::led(name, state), TerminalAgentLed, PaneLed(chrome::text::CONTROL)));
                    });
                }
                None => {
                    bar.spawn((bevy_gooey::led(name, state), TerminalAgentLed, PaneLed(chrome::text::CONTROL)));
                }
            }
        }
        if plan.start_agent {
            chrome::chip(
                bar,
                TerminalAgentChip,
                START_AGENT,
                chrome::text::CONTROL,
                chrome::ink().accent,
                chrome::ink().bar_bg,
                chrome::ink().panel_edge,
            )
            .insert(bevy_gooey::Tooltip::new(format!("Start another {} session in {folder}", terminal.agent().name())));
        }
        chrome::chip(
            bar,
            TerminalNewTab,
            "+",
            chrome::text::CONTROL,
            chrome::ink().text,
            chrome::ink().bar_bg,
            chrome::ink().panel_edge,
        )
        .insert(bevy_gooey::Tooltip::new(format!("Open a new shell in {folder}")));
        chrome::chip(
            bar,
            TerminalCloseTab,
            "x",
            chrome::text::CONTROL,
            chrome::ink().danger,
            chrome::ink().bar_bg,
            chrome::ink().panel_edge,
        )
        .insert(bevy_gooey::Tooltip::new(format!("{TIP_CLOSE_TAB}: {}", chord_words(&crate::chords::chord(crate::chords::Fixed::TerminalCloseTab)))));
    });
}

/// **`Ctrl+\`` raises the pane, or lowers it when it already has the keys.**
///
/// Three cases, and each is what a person means:
///
/// - **lowered** — show it and take the keyboard; the pump starts a shell if there is none;
/// - **raised and focused** — lower it, and give the editor its keys back;
/// - **raised and unfocused** — take the keys, which is how you get back into a shell you clicked away
///   from without reaching for the mouse again.
///
/// One pane per screen, always spawned; raising is `Display::Flex`, so there is no second spawn path
/// and nothing to keep in step. The collapse is written to [`Terminal`] too, so the next screen's pane
/// is built the way this one was left (#315).
fn toggle_on_the_chord(
    keys: Res<ButtonInput<KeyCode>>,
    mut panes: Query<&mut Node, With<TerminalPaneRoot>>,
    mut focus: ResMut<TerminalFocus>,
    mut terminal: ResMut<Terminal>,
) {
    // **Read from the chord table** (#640), so it can be rebound and the collision test can see it.
    let chord = crate::chords::chord(crate::chords::Fixed::TerminalRaise);
    if !chord.just_pressed(&keys) {
        return;
    }
    let Some(mut node) = panes.iter_mut().next() else {
        return;
    };
    let lowered = node.display == Display::None;
    if lowered {
        show(&mut node, &mut terminal, false);
        focus.take();
        return;
    }
    if focus.holds() {
        show(&mut node, &mut terminal, true);
        focus.release();
        return;
    }
    // Raised but unfocused: the press means "put me back in the shell", not "hide it".
    focus.take();
}

/// **`Terminal.maximise`**: the chord's own system. It is read here, before the shell sees anything, as the collapse chord is, so a shell
/// that holds the keys never hears it ([`crate::terminal_view`] swallows it).
fn maximise_on_the_chord(keys: Res<ButtonInput<KeyCode>>, mut focus: ResMut<TerminalFocus>, mut terminal: ResMut<Terminal>) {
    if crate::chords::chord(crate::chords::Fixed::TerminalMaximise).just_pressed(&keys) {
        toggle_maximised(&mut terminal, &mut focus);
    }
}

/// **Maximise the pane, or give it back its height**, and put the keys in the shell: a collapsed pane is raised straight to full. The one
/// thing the chord and View ▸ Maximise terminal both do.
pub fn toggle_maximised(terminal: &mut Terminal, focus: &mut TerminalFocus) {
    terminal.toggle_maximised();
    focus.take();
}

/// **View ▸ Maximise terminal**, from a world (the menu row has no system parameters).
pub fn maximise_from_world(world: &mut World) {
    world.resource_scope(|world: &mut World, mut terminal: Mut<Terminal>| {
        if let Some(mut focus) = world.get_resource_mut::<TerminalFocus>() {
            toggle_maximised(&mut terminal, &mut focus);
        }
    });
}

/// **The pane covers the window below the menu bar while it is maximised** (#426): it leaves the frame's column and stands over it, from
/// the menu bar's foot to the window's edges, and the shell is told the new size as on any resize ([`crate::terminal_view`] measures the body each
/// frame). Given back, it is in the frame's column again at the height it had ([`Terminal::height`], which `fit_the_pane_to_the_window`
/// corrects when the person never chose one). Each write is compared first.
///
/// **While it covers the window it is a layer of its own** ([`chrome::TERMINAL_MAXIMISED_Z`], #435): a local `ZIndex` only orders the pane among its
/// siblings, so the map's overlays (`GlobalZIndex`, [`chrome::COMPASS_Z`]) stood over it. The layer is added when it is maximised and taken off when
/// it is given back, because a `GlobalZIndex` on the pane in the frame's column would paint it over the status band and the docks' scrollbars.
fn apply_the_maximise(
    mut commands: Commands,
    terminal: Res<Terminal>,
    menus: Query<&ComputedNode, With<crate::menu_bar::EditorMenuBar>>,
    mut panes: Query<(Entity, &mut Node, Option<&GlobalZIndex>), With<TerminalPaneRoot>>,
) {
    let maximised = terminal.is_maximised();
    let top = menus.iter().next().map_or(0.0, |menu| menu.size().y * menu.inverse_scale_factor());
    for (pane, mut node, layer) in &mut panes {
        // PLACES-ITSELF-OK: a maximised terminal stands over the frame from the menu bar's foot to the window's edges (#426); it is a pane the
        // person asked to be full-window, not a panel `chrome::Frame` places.
        let position = if maximised { PositionType::Absolute } else { PositionType::Relative };
        if node.position_type != position {
            node.position_type = position;
        }
        let (left, right, bottom, above) = if maximised {
            (Val::Px(0.0), Val::Px(0.0), Val::Px(0.0), Val::Px(top))
        } else {
            (Val::Auto, Val::Auto, Val::Auto, Val::Auto)
        };
        if node.left != left || node.right != right || node.bottom != bottom || node.top != above {
            node.left = left;
            node.right = right;
            node.bottom = bottom;
            node.top = above;
        }
        if maximised && node.height != Val::Auto {
            node.height = Val::Auto;
        } else if !maximised && node.height == Val::Auto {
            // **Given back, the pane takes back a pixel height** (#492). It must, and writing nothing left it at `Auto` — a column
            // child at `Auto` grows to its content, which measured as the full body: the pane came back from the maximise
            // *maximised*. `Terminal::height()` is the person's drag when they made one, and `fit_the_pane_to_the_window` states the
            // quarter for a pane they never sized, so this one line serves both positions.
            node.height = Val::Px(terminal.height());
        }
        match (maximised, layer) {
            (true, Some(GlobalZIndex(z))) if *z == crate::style::metrics().z.terminal_maximised => {}
            (true, _) => {
                commands.entity(pane).insert(GlobalZIndex(crate::style::metrics().z.terminal_maximised));
            }
            (false, Some(_)) => {
                commands.entity(pane).remove::<GlobalZIndex>();
            }
            (false, None) => {}
        }
    }
}

/// **Hand the pane's geometry to the host once gooey has laid it out** (G-86/G-89): the first restyle writes the lengths `Sizing` names (width, height, least height), and
/// from then on `HostGeometry` leaves `width`, `height`, `min_*` and the insets alone, so `height` (the grip, the fit, the maximise) and `position_type`/the insets (the
/// maximise) are the host's to write without gooey restyling them back. The marker is added after the node has been styled, as gooey's docs ask, not at spawn, because
/// at spawn the lengths would never be written at all.
#[allow(clippy::type_complexity)]
fn host_the_pane_geometry(mut commands: Commands, panes: Query<(Entity, &Node), (With<TerminalPaneRoot>, Without<bevy_gooey::HostGeometry>)>) {
    for (pane, node) in &panes {
        if node.width == Val::Percent(100.0) {
            commands.entity(pane).insert(bevy_gooey::HostGeometry);
        }
    }
}
/// **Primary+J collapses the pane, or restores it**, on every screen (#315).
///
/// Two cases, VS Code's: a raised pane collapses and gives the screen its keys back, wherever the keys
/// were; a collapsed one comes back with the keyboard in the shell, because a person who restores the
/// terminal means to type in it. The shell never hears the chord ([`crate::terminal_view`]'s key path
/// swallows it), which matters on Linux, where Ctrl+J is also a newline.
fn collapse_on_the_chord(
    keys: Res<ButtonInput<KeyCode>>,
    mut panes: Query<&mut Node, With<TerminalPaneRoot>>,
    mut focus: ResMut<TerminalFocus>,
    mut terminal: ResMut<Terminal>,
) {
    if !crate::chords::chord(crate::chords::Fixed::TerminalCollapse).just_pressed(&keys) {
        return;
    }
    cycle_the_pane(&mut panes, &mut focus, &mut terminal);
}

/// **`Cmd+J`'s press: the pane moves one position round** (#492): `Maximised → Collapsed → Quarter → Maximised`. The one thing
/// the chord and the status bar's label both do, and the reason they are one function: a cycle whose two doors disagreed would
/// leave a person unable to predict the next press.
///
/// The keys go where a person means them: back to the screen when the pane goes away, in the shell when it comes back or changes
/// size, because a person who presses the chord means to work in the terminal.
fn cycle_the_pane(panes: &mut Query<&mut Node, With<TerminalPaneRoot>>, focus: &mut TerminalFocus, terminal: &mut Terminal) {
    if panes.iter_mut().next().is_none() {
        return;
    }
    let placement = terminal.cycle_placement();
    match placement {
        TerminalPlacement::Collapsed => focus.release(),
        TerminalPlacement::Quarter | TerminalPlacement::Maximised => focus.take(),
    }
}

/// **Build the status bar's terminal label** (#424), the last thing in the band, as the editor's frame is entered.
fn spawn_the_status_label(mut commands: Commands, frame: Option<Res<chrome::Frame>>) {
    let Some(frame) = frame else {
        return;
    };
    commands.entity(frame.status).with_children(|band| {
        chrome::chip(
            band,
            TerminalStatusLabel,
            &status_label_words(),
            chrome::text::CONTROL,
            chrome::ink().text,
            chrome::ink().bar_bg,
            chrome::ink().panel_edge,
        )
        .insert(bevy_gooey::Tooltip::new(status_tooltip_words(TerminalPlacement::Collapsed)));
    });
}

/// **A click on the status bar's label raises the pane, or lowers it, as the chord does** (#424). It is outside the pane, so
/// [`a_click_elsewhere_gives_it_back`] leaves it alone: the click is this one's.
fn the_label_flips_the_pane(
    mut click: On<bevy::picking::events::PointerClick>,
    labels: Query<(), With<TerminalStatusLabel>>,
    mut panes: Query<&mut Node, With<TerminalPaneRoot>>,
    mut focus: ResMut<TerminalFocus>,
    mut terminal: ResMut<Terminal>,
) {
    if !labels.contains(click.entity) {
        return;
    }
    click.propagate(false);
    cycle_the_pane(&mut panes, &mut focus, &mut terminal);
}

/// **A raise or a lower a person made is kept** (#424), in the user's settings file as `terminal_collapsed`, so the next launch opens
/// the pane as it was left, the way the window's arrangement is kept (`ui::save_the_layout`).
///
/// It follows the pane's state rather than each chord, so every way of raising and lowering it is kept. It writes nothing for the
/// state the pane started in, nothing for a change the file itself made (the setting already says so), and nothing without a user
/// settings file: a headless test that raises the pane never writes the real config.
fn keep_the_pane_choice(
    terminal: Res<Terminal>,
    file: Option<Res<crate::settings::UserSettingsFile>>,
    settings: Option<Res<crate::settings::EditorSettings>>,
    mut state: Option<ResMut<crate::editor::EditorState>>,
    mut seen: Local<Option<bool>>,
) {
    let collapsed = terminal.is_collapsed();
    let Some(before) = seen.replace(collapsed) else {
        return;
    };
    if before == collapsed || settings.as_deref().is_some_and(|settings| settings.terminal_collapsed == Some(collapsed)) {
        return;
    }
    let Some(file) = file else {
        return;
    };
    if let Err(why) = crate::setting_catalogue::edit_layer(&file.0, |layer| {
        layer.terminal_collapsed = Some(collapsed);
        Ok(())
    }) && let Some(state) = state.as_deref_mut()
    {
        state.status.problem(format!("the terminal's state was not saved: {why}"));
    }
}

/// Collapse the pane or restore it, on this screen and, through [`Terminal`], on every one after it.
fn show(node: &mut Node, terminal: &mut Terminal, collapsed: bool) {
    node.display = if collapsed { Display::None } else { Display::Flex };
    terminal.set_collapsed(collapsed);
}

/// **`Ctrl+Shift+\`` closes the tab**, leaving the pane up — which is what a person wants after
/// typing `exit` into a shell that is now gone.
fn close_the_tab_on_its_chord(
    keys: Res<ButtonInput<KeyCode>>,
    terminal: ResMut<Terminal>,
    bar: Query<Entity, With<TerminalBar>>,
    mut commands: Commands,
) {
    let chord = crate::chords::chord(crate::chords::Fixed::TerminalCloseTab);
    if !chord.just_pressed(&keys) {
        return;
    }
    let mut terminal = terminal;
    terminal.close_active();
    redraw_the_bar(&mut commands, &bar, &terminal);
}

/// **Command+K, with the shell holding the keys, clears the terminal** (#326): the prompt's line becomes
/// the first line and everything above it goes, scrollback and screen
/// ([`crate::terminal::TerminalScreen::clear_keeping_prompt`]). Nothing reaches the shell, neither the
/// chord (`terminal_view::type_keys` never forwards it) nor a request to clear.
fn clear_on_its_chord(keys: Res<ButtonInput<KeyCode>>, focus: Res<TerminalFocus>, mut terminal: ResMut<Terminal>) {
    if !focus.holds() || !crate::chords::chord(crate::chords::Fixed::TerminalClear).just_pressed(&keys) {
        return;
    }
    if let Some(tab) = terminal.tab_mut() {
        tab.screen_mut().clear_keeping_prompt();
    }
}

/// **Primary+F, with the shell holding the keys, opens find** (#321), or gives the keys back to a box
/// already open. The chord is the pane's: `terminal_view::type_keys` never sends it to the shell.
fn open_find_on_its_chord(
    keys: Res<ButtonInput<KeyCode>>,
    focus: Res<TerminalFocus>,
    mut find: ResMut<TerminalFind>,
    bodies: Query<Entity, With<TerminalBody>>,
    mut commands: Commands,
) {
    if !focus.holds() || !crate::chords::chord(crate::chords::Fixed::TerminalFind).just_pressed(&keys) {
        return;
    }
    let Some(body) = bodies.iter().next() else {
        return;
    };
    if !find.open {
        spawn_the_find_box(&mut commands, body);
    }
    find.open = true;
    find.in_box = true;
}

/// **The find box**: a query field, its `n of m` and a close button, over the top right of the body,
/// where VS Code's terminal puts its find widget. It covers the newest output's right-hand end only
/// while it is open, and the matches it finds are scrolled into view below it.
fn spawn_the_find_box(commands: &mut Commands, body: Entity) {
    let frame = commands
        .spawn((
            // The find box floats over the scrollback it searches, as VS Code's terminal find does; it is not a panel `chrome::Frame` places: placed in the body's top right
            // (`BODY_INSET.y` down, a panel's inset in), `FIND_BOX_W` wide, its parts' gap and its inset the sheet's `.terminal-float`, a hairline edge, in the editor's own colours (`InkFollow`).
            bevy_gooey::placed(UiRect { top: Val::Px(crate::terminal_view::body_inset().y), right: Val::Px(chrome::space(chrome::STEP_PAD)), ..UiRect::all(Val::Auto) }),
            bevy_gooey::Sizing { width: Some(bevy_gooey::Extent::Px(FIND_BOX_W)), ..default() },
            bevy_gooey::RowLayout { direction: Some(FlexDirection::Row), align: Some(AlignItems::Center), ..default() },
            bevy_gooey::StyleClass::of("terminal-float"),
            bevy_gooey::BorderPx(bevy_gooey::Sides::all(crate::style::metrics().layout.edge_w)),
            bevy_gooey::InkFollow,
            BackgroundColor(chrome::ink().bar_bg),
            BorderColor::all(chrome::ink().panel_edge),
            // Above the rows and the caret, which are its siblings.
            ZIndex(2),
            TerminalFindFrame,
            Name::new("terminal find"),
            children![
                (
                    bevy_gooey::text_input("", ""),
                    bevy_gooey::TextHint("Find".into()),
                    TerminalFindBox,
                    bevy_gooey::observe(the_query_changed),
                ),
                (
                    Text::new(String::new()),
                    TextColor(chrome::ink().muted),
                    TextLayout::no_wrap(),
                    chrome::typed(chrome::text::LABEL),
                    TerminalFindCount,
                    Pickable::IGNORE,
                    Name::new("terminal find count"),
                ),
                (bevy_gooey::button("×"), bevy_gooey::observe(close_find_on_click)),
            ],
        ))
        // **Not `Pickable::IGNORE`, which `placed` is**: the frame takes the pointer on its padding and edge, so a click there is the box's and does not
        // fall to the shell beneath. Inserted after the spawn, not listed in the tuple: `placed` already carries a `Pickable`, and a bundle that holds one
        // component twice panics at spawn in bevy_ecs 0.20 (as `spawn_the_invitation` does).
        .insert(Pickable::default())
        .id();
    commands.entity(body).add_child(frame);
}

/// **The self-test card** (#421): the harness, then one row per check, each starting with a small LED in the row's state (passed is
/// live, failed is failed, skipped is off, and a check still running is busy) and the check's sentence beside it, in the check's ink,
/// and the agent's LED as the verdict they add up to. There is no `OK`, `FAIL` or `SKIP`: a row's LED keeps its accessible state name
/// ([`quiet_the_row_leds`] hides the word it draws, not the word). Floats where the find box does, and the pointer goes through it to
/// the shell.
///
/// **The ink is the running skin's** (UI-02, #210, slice 6a): every sentence carries
/// [`TerminalAgentCardRow`], so [`dress_the_agent_tab`] repaints the card when a project's skin changes
/// without the card being rebuilt.
fn spawn_the_agent_card(commands: &mut Commands, body: Entity, post: &crate::terminal_agent::AgentPost) {
    let ink = crate::terminal_agent::AgentInk::of(None);
    let sentence = |words: String, ink: Color, which: TerminalAgentCardRow| {
        (Text::new(words), TextColor(ink), chrome::typed(chrome::text::LABEL), which, Pickable::IGNORE)
    };
    let card = commands
        .spawn((
            // The self-test card floats over the terminal it reports on, as the find box does; it is not a panel `chrome::Frame` places: placed at the body's top right
            // (`BODY_INSET.y` down, a panel's inset in), as wide as its content from `AGENT_CARD_MIN_W` up to the terminal's own width, its rows' gap and its inset the sheet's
            // `.terminal-float`, a hairline edge, in the editor's own colours (`InkFollow`). **`placed` is `Pickable::IGNORE`, which the card is**: the pointer goes through it to the shell.
            bevy_gooey::placed(UiRect { top: Val::Px(crate::terminal_view::body_inset().y), right: Val::Px(chrome::space(chrome::STEP_PAD)), ..UiRect::all(Val::Auto) }),
            bevy_gooey::Sizing { min_width: Some(bevy_gooey::Extent::Px(AGENT_CARD_MIN_W)), max_width: Some(bevy_gooey::Extent::Percent(100.0)), ..default() },
            bevy_gooey::StyleClass::of("terminal-float"),
            bevy_gooey::BorderPx(bevy_gooey::Sides::all(crate::style::metrics().layout.edge_w)),
            bevy_gooey::InkFollow,
            BackgroundColor(chrome::ink().bar_bg),
            BorderColor::all(chrome::ink().panel_edge),
            // Above the rows and the caret, which are its siblings.
            ZIndex(2),
            TerminalAgentCard,
            Name::new("terminal agent card"),
        ))
        .with_children(|card| {
            card.spawn(sentence(post.heading(), ink.read, TerminalAgentCardRow(None)));
            for (index, (label, check)) in crate::terminal_agent::CHECK_LABELS.iter().zip(&post.checks).enumerate() {
                card.spawn((
                    (bevy_gooey::row_with(0.0, bevy_gooey::RowLayout { align: Some(AlignItems::Center), ..default() }), chrome::Gap::Tight.class()),
                    Pickable::IGNORE,
                ))
                .with_children(|row| {
                    // The LED is named for the check, so a screen reader hears `Agent started live` and not a bare state.
                    row.spawn((bevy_gooey::led(*label, check.led()), TerminalCardRowLed(index), PaneLed(chrome::text::LABEL)));
                    row.spawn(sentence(format!("{label}: {}", check.detail()), check.ink(&ink), TerminalAgentCardRow(Some(index))));
                });
            }
            // **The servers this session has** (#484): a sentence, not a check, so it stands under the five rows and cannot be
            // read as a sixth thing that passed. Absent on a pre-#484 card, which drew exactly the rows it drew before.
            if let Some(servers) = post.servers_row() {
                card.spawn(sentence(servers, ink.muted, TerminalAgentCardRow(None)));
            }
            card.spawn((bevy_gooey::led(AGENT_LED_LABEL, post.led()), TerminalAgentCardLed, PaneLed(chrome::text::LABEL)));
        })
        .id();
    commands.entity(body).add_child(card);
}

/// **A row of the self-test card starts with this LED** (#421), and which of the four checks it lights for.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalCardRowLed(pub usize);

/// **A row's LED draws its dot, not its words** (#421): the row's own sentence is beside it, and a state word on every row is the
/// `OK` and `FAIL` the owner asked to be rid of. The words stay in the world (hidden, not removed), so the LED keeps the accessible name
/// `bevy_gooey` gives it and a test still reads the state.
fn quiet_the_row_leds(rows: Query<&Children, Added<TerminalCardRowLed>>, mut words: Query<&mut Node, With<Text>>) {
    for children in &rows {
        for child in children.iter() {
            if let Ok(mut word) = words.get_mut(child) {
                word.display = Display::None;
            }
        }
    }
}

/// **The invitation**: a button in the middle of the empty terminal that starts the agent.
fn spawn_the_invitation(commands: &mut Commands, body: Entity) {
    let invite = commands
        .spawn((
            // The invitation floats over the empty body it stands in, as the self-test card floats over a full one; it is not a panel `chrome::Frame` places: gooey's `cover`
            // (placed on every side), its button centred in it. **Not `Pickable::IGNORE`** (which `cover` is): it skips an entity and everything below it, and the button is below
            // it; `Pickable::default()` below puts that back, and a click beside the button bubbles to the pane, which takes the keyboard.
            bevy_gooey::cover(),
            bevy_gooey::RowLayout { align: Some(AlignItems::Center), justify: Some(JustifyContent::Center), ..default() },
            ZIndex(2),
            // **Not `Pickable::IGNORE`**: it skips an entity and everything below it, and the button is
            // below it. A click beside the button bubbles to the pane, which takes the keyboard.
            TerminalInvite,
            Name::new("terminal invitation"),
        ))
        .insert(Pickable::default())
        .with_children(|invite| {
            chrome::chip(
                invite,
                TerminalInvokeButton,
                START_AGENT,
                chrome::text::CONTROL,
                chrome::ink().accent,
                chrome::ink().bar_bg,
                chrome::ink().panel_edge,
            );
        })
        .id();
    commands.entity(body).add_child(invite);
}

/// What [`keep_the_agent_on_screen`] last drew from: the body, the tab count, the drawn tab, its
/// record's revision, whether its card shows, whether find is open, the chosen agent, whether the
/// pane invites the agent, and what the bar holds in the room it has.
type AgentDrawn = (Option<Entity>, usize, usize, u32, bool, bool, crate::settings::TerminalAgent, bool, BarPlan);

/// **The bar's chips and the self-test card follow the agent tabs.** Redrawn only when something they
/// show moved: a tab opened or closed, another tab drawn, a check changed, the card's linger ran out,
/// find opened (the card steps aside for it), the settings chose another agent, a new pane was built, or the bar's room
/// changed what it holds (#423).
#[allow(clippy::too_many_arguments)]
fn keep_the_agent_on_screen(
    mut drawn: Local<Option<AgentDrawn>>,
    terminal: Res<Terminal>,
    find: Res<TerminalFind>,
    time: Res<Time<Real>>,
    bodies: Query<Entity, With<TerminalBody>>,
    cards: Query<Entity, With<TerminalAgentCard>>,
    invitations: Query<Entity, With<TerminalInvite>>,
    bar: Query<Entity, With<TerminalBar>>,
    mut commands: Commands,
) {
    let now = time.elapsed_secs_f64();
    let body = bodies.single().ok();
    let post = terminal.tab().and_then(|tab| tab.agent());
    let shown = post.is_some_and(|post| post.card_shown(now));
    let key = (
        body,
        terminal.len(),
        terminal.active(),
        post.map_or(0, |post| post.revision),
        shown,
        find.is_open(),
        terminal.agent(),
        terminal.invites_the_agent(),
        bar_plan(terminal.bar_room(), &bar_wants(&terminal)),
    );
    if drawn.as_ref() == Some(&key) {
        return;
    }
    *drawn = Some(key);
    redraw_the_bar(&mut commands, &bar, &terminal);
    for card in cards.iter().chain(&invitations) {
        commands.entity(card).despawn();
    }
    if let Some(body) = body
        && terminal.invites_the_agent()
    {
        spawn_the_invitation(&mut commands, body);
    }
    if let (Some(body), Some(post)) = (body, post)
        && shown
        && !find.is_open()
    {
        spawn_the_agent_card(&mut commands, body, post);
    }
}

/// **A query typed into the box searches the active shell's scrollback**, as it is typed.
///
/// A final change is not a search: gooey sends one for Enter, which [`the_find_box_keys`] reads as
/// "the next match", and one when the box loses the keys, which must move nothing.
fn the_query_changed(change: On<bevy_gooey::ValueChange<String>>, mut terminal: ResMut<Terminal>) {
    if change.is_final {
        return;
    }
    if let Some(tab) = terminal.tab_mut() {
        tab.screen_mut().find(&change.value);
    }
}

/// The box's close button.
fn close_find_on_click(
    _click: On<bevy_gooey::Activate>,
    mut find: ResMut<TerminalFind>,
    frames: Query<Entity, With<TerminalFindFrame>>,
    mut terminal: ResMut<Terminal>,
    mut commands: Commands,
) {
    close_find(&mut commands, &mut find, &frames, &mut terminal);
}

/// **Close find**: the box goes, every search is cleared, and the keys stay in the terminal — the
/// shell has them again, as it did before Primary+F.
fn close_find(
    commands: &mut Commands,
    find: &mut TerminalFind,
    frames: &Query<Entity, With<TerminalFindFrame>>,
    terminal: &mut Terminal,
) {
    for frame in frames {
        commands.entity(frame).despawn();
    }
    find.open = false;
    find.in_box = false;
    terminal.clear_searches();
}

/// **Enter and Escape in the find box** (#321): Enter walks to the next match up, in older output, and
/// Shift+Enter down; Escape closes find.
///
/// **Decided in `PreUpdate`, before Bevy dispatches keys to the input focus**, because the focus is the
/// box's text field, and gooey reads Escape in a field as "end the edit, then back" — which reached the
/// screen behind the terminal. Closing here means [`hold_the_keyboard`], next in the chain, puts the
/// focus back on the body before the dispatch, so the body's observer stops the Escape as it stops every
/// key the shell holds.
fn the_find_box_keys(
    mut events: MessageReader<KeyboardInput>,
    keys: Res<ButtonInput<KeyCode>>,
    mut find: ResMut<TerminalFind>,
    frames: Query<Entity, With<TerminalFindFrame>>,
    mut terminal: ResMut<Terminal>,
    mut commands: Commands,
) {
    if find.closed_this_frame {
        find.closed_this_frame = false;
    }
    if !find.has_the_keys() {
        events.clear();
        return;
    }
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    for event in events.read() {
        if event.state != bevy::input::ButtonState::Pressed {
            continue;
        }
        match event.key_code {
            KeyCode::Escape => {
                close_find(&mut commands, &mut find, &frames, &mut terminal);
                find.closed_this_frame = true;
                return;
            }
            KeyCode::Enter | KeyCode::NumpadEnter => {
                if let Some(tab) = terminal.tab_mut() {
                    tab.screen_mut().find_next(!shift);
                }
            }
            _ => {}
        }
    }
}

/// **The box says `n of m`**, `n of m+` past [`crate::terminal::FIND_MATCH_CAP`], `No results`, or
/// nothing while the query is empty. Written only when it changed.
fn count_the_matches(
    find: Res<TerminalFind>,
    terminal: Res<Terminal>,
    boxes: Query<&bevy_gooey::TextInputValue, With<TerminalFindBox>>,
    mut counts: Query<&mut Text, With<TerminalFindCount>>,
) {
    if !find.is_open() {
        return;
    }
    let empty = boxes.iter().next().is_none_or(|query| query.0.is_empty());
    let status = terminal.tab().and_then(|tab| tab.screen().find_status());
    let said = match status {
        _ if empty => String::new(),
        Some(crate::terminal::FindStatus { total, current, capped }) if total > 0 => {
            format!("{current} of {total}{}", if capped { "+" } else { "" })
        }
        _ => "No results".to_owned(),
    };
    for mut count in &mut counts {
        if count.0 != said {
            count.0.clone_from(&said);
        }
    }
}

/// **A find box that went with its pane closes find.** The box is a child of the body, so a door
/// change or a closed pane takes it; the flag and the searches would otherwise outlive it, and the next
/// pane's shell would never get its keys.
fn forget_a_find_box_that_went(
    mut find: ResMut<TerminalFind>,
    frames: Query<(), With<TerminalFindFrame>>,
    mut terminal: ResMut<Terminal>,
) {
    if find.open && frames.is_empty() {
        find.open = false;
        find.in_box = false;
        terminal.clear_searches();
    }
}

/// Give the keyboard back when the door closes.
fn give_the_keyboard_back(mut focus: ResMut<TerminalFocus>) {
    focus.release();
}

/// **The agent tab's ink follows the running skin** (UI-02, #210, slice 6a).
///
/// The card and the bar's chip are rebuilt when their *content* moves — a check passed, a tab
/// changed — which is exactly why their ink cannot be read once at spawn: a project's
/// `family/bevy_gooey.yaml` edited while the card stands reached the panels, the list rows and the
/// status band, and left this tab in the editor's darks. This is the split
/// [`crate::chrome::dim_the_ground`] makes for a panel's ground: the spawn paints what it always
/// painted, and this system reads the skin, so a card built before an edit follows it.
///
/// **It writes ink and nothing else.** The chip's *words* have one writer,
/// [`fill_the_bar`], which is what rebuilds the bar whenever the post's revision moves — and a check
/// passing is exactly that. Two writers would be two answers to one question, and the one that ran
/// last would win the frame.
///
/// It compares before writing, so a settled editor costs one comparison per row.
fn dress_the_agent_tab(
    terminal: Res<Terminal>,
    skin: Option<Res<bevy_gooey::GooeySkin>>,
    mut rows: Query<(&TerminalAgentCardRow, &mut TextColor)>,
) {
    let ink = crate::terminal_agent::AgentInk::of(skin.as_deref());
    let post = terminal.tab().and_then(|tab| tab.agent());


    // The card's rows: the heading, then one per check, each in the ink of its own state.
    for (which, mut colour) in &mut rows {
        let want = match (which.0, post) {
            (None, _) => ink.read,
            (Some(index), Some(post)) => post.checks.get(index).map_or(ink.muted, |check| check.ink(&ink)),
            (Some(_), None) => ink.muted,
        };
        if colour.0 != want {
            colour.0 = want;
        }
    }
}

/// **Every LED of the pane reads at the size of the text beside it** (#422).
///
/// The words of a `bevy_gooey::led` (its name and its state word) are drawn at the skin's `text_size * text_scale`, while the editor's
/// chips and rows are a type role times the person's text size ([`chrome::font`]); at a `text_scale` of 1.45 the two disagreed and the
/// LED's word stood out. After gooey has styled them, each word is set to the size of its [`PaneLed`]'s role, and the dot to its
/// word's size times [`LED_DOT_SIZES`], so the dot keeps its relation to the word. Each write is compared first, and gooey only
/// restyles on a change, so a settled pane writes nothing.
///
fn size_the_leds(
    leds: Query<(&Children, &PaneLed)>,
    roles: Query<&bevy_gooey::Role>,
    mut fonts: Query<&mut TextFont, With<Text>>,
    mut dots: Query<&mut Node>,
) {
    for (children, PaneLed(role)) in &leds {
        let size = chrome::font(*role).font_size;
        let side = Val::Px(role.px() * chrome::text_scale() * LED_DOT_SIZES);
        for child in children.iter() {
            if let Ok(mut font) = fonts.get_mut(child) {
                if font.font_size != size {
                    font.font_size = size;
                }
            } else if roles.get(child) == Ok(&bevy_gooey::Role::LedDot)
                && let Ok(mut dot) = dots.get_mut(child)
                && (dot.width != side || dot.height != side)
            {
                dot.width = side;
                dot.height = side;
            }
        }
    }
}

/// Redraw the bar after the session list changed.
fn redraw_the_bar(commands: &mut Commands, bar: &Query<Entity, With<TerminalBar>>, terminal: &Terminal) {
    if let Ok(bar) = bar.single() {
        fill_the_bar(commands, bar, terminal);
    }
}

/// **The bar's room, measured** (#423): the bar's laid-out width in logical px, given to the [`Terminal`] so [`bar_plan`] can decide what
/// a narrow bar holds. Taken again only when it moves by [`BAR_ROOM_STEP`], and without marking the terminal changed.
fn measure_the_bar(bars: Query<&ComputedNode, With<TerminalBar>>, mut terminal: ResMut<Terminal>) {
    let Some(bar) = bars.iter().next() else {
        return;
    };
    let room = bar.size().x * bar.inverse_scale_factor();
    if room <= 0.0 {
        return;
    }
    if terminal.bar_room().is_none_or(|known| (known - room).abs() >= BAR_ROOM_STEP) {
        terminal.bypass_change_detection().set_bar_room(room);
    }
}

/// **A click anywhere in the pane puts the keyboard in the shell** — including the bar, because a
/// person who clicked the chip to switch sessions means to type into the one they picked.
///
/// **One click, one answer, decided where it landed** (#317). A `PointerClick` bubbles: it is triggered
/// on the node under the pointer and then on each of its ancestors, and a global observer runs on every
/// hop. These two observers read the hop's own entity, so a click on the terminal's body took the
/// keyboard on its first hop and gave it back on the third, at the frame root above the pane, which is
/// "outside" it — measured: the click left `TerminalFocus` false and the chord was the only way in.
/// Both now act once, on the first hop, and ask where the click *landed*.
///
/// **With find open, the click also says who in the terminal has the keys** (#321): a click in the
/// find box gives them to its query, a click anywhere else in the pane to the shell, and the matches
/// stay lit either way — VS Code's terminal find behaves so.
fn a_click_takes_the_keyboard(
    click: On<bevy::picking::events::PointerClick>,
    ancestors: Query<&ChildOf>,
    roots: Query<(), With<TerminalPaneRoot>>,
    frames: Query<(), With<TerminalFindFrame>>,
    mut focus: ResMut<TerminalFocus>,
    mut find: ResMut<TerminalFind>,
) {
    if click.entity == click.original_event_target() && landed_in_the_pane(click.entity, &ancestors, &roots) {
        focus.take();
        if find.open {
            find.in_box = frames.contains(click.entity) || ancestors.iter_ancestors(click.entity).any(|up| frames.contains(up));
        }
    }
}

/// **A click anywhere outside it gives the keys back**, or the editor would be dead to the keyboard
/// with nothing on screen saying why. Decided on the click's first hop, like
/// [`a_click_takes_the_keyboard`].
fn a_click_elsewhere_gives_it_back(
    click: On<bevy::picking::events::PointerClick>,
    ancestors: Query<&ChildOf>,
    roots: Query<(), With<TerminalPaneRoot>>,
    labels: Query<(), With<TerminalStatusLabel>>,
    mut focus: ResMut<TerminalFocus>,
) {
    if !focus.holds() || click.entity != click.original_event_target() {
        return;
    }
    // The status bar's label raises and lowers the pane itself, and says where the keys go.
    if labels.contains(click.entity) || ancestors.iter_ancestors(click.entity).any(|up| labels.contains(up)) {
        return;
    }
    if !landed_in_the_pane(click.entity, &ancestors, &roots) {
        focus.release();
    }
}

/// Is `entity` the terminal's pane, or inside it?
fn landed_in_the_pane(
    entity: Entity,
    ancestors: &Query<&ChildOf>,
    roots: &Query<(), With<TerminalPaneRoot>>,
) -> bool {
    roots.contains(entity) || ancestors.iter_ancestors(entity).any(|up| roots.contains(up))
}

/// **The keys a terminal owns that the screen's keyboard navigation also reads** (#316): Tab and the
/// arrows move bevy_gooey's focus, Enter and Space press the focused widget, Escape cancels, Page Up and
/// Down page a list, F6 cycles regions, and F10 or an Alt tap open the menu bar. Every one of them means
/// something to a shell (completion, history, a word back with Alt), so while the shell has the
/// keyboard none of them is also the screen's.
const TERMINAL_KEYS: [KeyCode; 19] = [
    KeyCode::ArrowUp,
    KeyCode::ArrowDown,
    KeyCode::ArrowLeft,
    KeyCode::ArrowRight,
    KeyCode::Tab,
    KeyCode::Enter,
    KeyCode::NumpadEnter,
    KeyCode::Space,
    KeyCode::Escape,
    KeyCode::Home,
    KeyCode::End,
    KeyCode::PageUp,
    KeyCode::PageDown,
    KeyCode::Backspace,
    KeyCode::Delete,
    KeyCode::F6,
    KeyCode::F10,
    KeyCode::AltLeft,
    KeyCode::AltRight,
];

/// **A modal dialog takes the keyboard from the shell while it is up** (#446). Settings, the keymap, project configuration and New project are gooey dialogs
/// that close on `Escape` (gooey's `Cancel`), and `Escape` is one of the [`TERMINAL_KEYS`] [`hold_the_keyboard`] takes out of the frame for the shell: with the
/// NightShift's terminal holding the keys, a dialog opened over it (by a chord, say) heard nothing and only its Close button worked. The keys go back to the
/// editor the frame a dialog exists, so its fields and `Escape` are the dialog's; the shell gets them again with the next click in its pane.
fn a_dialog_takes_the_keyboard(mut focus: ResMut<TerminalFocus>, dialogs: Query<(), With<bevy_gooey::Dialog>>) {
    if focus.holds() && !dialogs.is_empty() {
        focus.release();
    }
}

/// **While the shell has the keyboard, the screen does not also act on it** (#316).
///
/// The shell reads `KeyboardInput`; the editor's bindings stand down on `Context::Typing`. bevy_gooey's
/// navigation reads `ButtonInput<KeyCode>`, and Bevy's Tab navigation and a field's typing read keys
/// dispatched to the input focus, and nothing told either of them — so on the projects home an Up
/// then an Enter into the shell landed focus on the home's first button and pressed it. Two halves, in
/// `PreUpdate` between Bevy's input and its focused-input dispatch: the [`TERMINAL_KEYS`] are taken
/// out of this frame's presses (their held state stays, so the shell's modifiers still read), and the
/// input focus is put on the terminal's body, whose observer stops every focused key there. When the
/// keyboard comes back the focus is let go, so the screen starts from nothing focused, as it did.
///
/// **With the find box holding the keys, the input focus is its field rather than the body** (#321),
/// so what is typed is the query; the body's stop is then not in the keys' way.
fn hold_the_keyboard(
    focus: Res<TerminalFocus>,
    find: Res<TerminalFind>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    input_focus: Option<ResMut<bevy::input_focus::InputFocus>>,
    bodies: Query<Entity, With<TerminalBody>>,
    boxes: Query<Entity, With<TerminalFindBox>>,
    // **The field, not its hint**: gooey puts `TextInputOf` on both, and a focused hint takes no keys
    // (measured live: the query typed into the box went nowhere). Bevy's `TextInput` is on the field.
    fields: Query<(Entity, &bevy_gooey::TextInputOf), With<bevy::ui_widgets::TextInput>>,
) {
    let body = bodies.iter().next();
    let field = boxes
        .iter()
        .next()
        .and_then(|row| fields.iter().find(|(_, of)| of.0 == row).map(|(field, _)| field));
    let Some(mut input_focus) = input_focus else {
        return;
    };
    if focus.holds() {
        for key in TERMINAL_KEYS {
            keys.clear_just_pressed(key);
        }
        let target = if find.has_the_keys() { field.or(body) } else { body };
        if let Some(target) = target
            && input_focus.get() != Some(target)
        {
            input_focus.set(target, bevy::input_focus::FocusCause::Navigated);
        }
    } else if input_focus.get().is_some_and(|held| Some(held) == body || Some(held) == field) {
        input_focus.clear();
    }
}

/// **A drag over the body selects**, which is the only way to copy a compiler error out.
///
/// The UI picking backend reports `hit.position` **normalized to the node**, `(-0.5, -0.5)` at its
/// top-left (`bevy_ui-0.20.0/src/picking_backend.rs`'s own header), so a cell is that plus a half,
/// times the node's logical size, divided by the cell size. No second camera projection.
///
/// Three observers rather than one with three event parameters: an `On<E>` observer is registered per
/// event type, and a system taking two of them is a system two fires would each run entirely.
fn a_drag_begins(
    start: On<bevy::picking::events::PointerDragStart>,
    body: Query<(&ComputedNode, &bevy::ui::UiGlobalTransform), With<TerminalBody>>,
    metrics: Res<CellMetrics>,
    mut pane: ResMut<TerminalPane>,
    mut terminal: ResMut<Terminal>,
) {
    if start.button != bevy::picking::pointer::PointerButton::Primary {
        return;
    }
    // A press the program was told about is the program's drag, not the person's selection (#313).
    if pane.mouse_down.is_some() {
        return;
    }
    let Ok((node, _)) = body.get(start.entity) else {
        return;
    };
    let Some(at) = cell_under(&start.hit, node, &metrics) else {
        return;
    };
    pane.drag_from = Some(at);
    if let Some(tab) = terminal.tab_mut() {
        tab.screen_mut().select(at, at);
    }
}

/// Extend the selection while the button is down.
fn a_drag_extends(
    drag: On<bevy::picking::events::PointerDrag>,
    body: Query<(), With<TerminalBody>>,
    metrics: Res<CellMetrics>,
    keys: Res<ButtonInput<KeyCode>>,
    pane: ResMut<TerminalPane>,
    mut terminal: ResMut<Terminal>,
) {
    if body.get(drag.entity).is_err() {
        return;
    }
    // **`PointerDrag` carries no `hit`** — only the start, enter, over and end events do — so the
    // pointer's displacement is `distance`, measured from the drag's start in screen pixels. Adding it
    // to the starting cell is the whole conversion: there is no second camera projection here because
    // the pane is UI, and a UI pixel is a cell once divided by the cell's measured size.
    let scale = metrics.scale();
    let (rows, cols) = (
        (drag.distance.y / (metrics.height() * scale)).round() as i32,
        (drag.distance.x / (metrics.width() * scale)).round() as i32,
    );
    let moved = |from: (usize, usize)| {
        (
            (from.0 as i32 + rows).max(0) as usize,
            (from.1 as i32 + cols).max(0) as usize,
        )
    };
    if let Some(down) = pane.mouse_down {
        if reported_button(drag.button) == down.button {
            report_drag(down, moved(down.from), pane, &keys, &mut terminal);
        }
        return;
    }
    if drag.button != bevy::picking::pointer::PointerButton::Primary {
        return;
    }
    let Some(from) = pane.drag_from else {
        return;
    };
    let to = moved(from);
    if let Some(tab) = terminal.tab_mut() {
        tab.screen_mut().select(from, to);
    }
}

/// **Motion with a reported button held**, under modes 1002 and 1003, once per cell entered.
fn report_drag(
    down: MouseDown,
    at: (usize, usize),
    mut pane: ResMut<TerminalPane>,
    keys: &ButtonInput<KeyCode>,
    terminal: &mut Terminal,
) {
    use crate::terminal::input::{MouseAction, mouse_report};
    if at == down.last {
        return;
    }
    let Some((tracking, encoding)) = terminal.tab().and_then(|tab| tab.screen().mouse_reporting()) else {
        return;
    };
    if !tracking.reports_drag() {
        return;
    }
    let Some(button) = down.button else {
        return;
    };
    if let Some(bytes) = mouse_report(button, MouseAction::Drag, at, mods_held(keys), encoding) {
        terminal.write(&bytes);
    }
    pane.mouse_down = Some(MouseDown { last: at, ..down });
}

/// Whether the platform's primary modifier is held: Cmd on a mac, Ctrl elsewhere (the chord that follows a link).
fn primary_held(keys: &ButtonInput<KeyCode>) -> bool {
    if cfg!(target_os = "macos") {
        keys.any_pressed([KeyCode::SuperLeft, KeyCode::SuperRight])
    } else {
        keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight])
    }
}

/// **Primary+click on an `http://` or `https://` address in the output opens it** (#325), with the
/// platform's opener ([`crate::app_verbs::open_link`], which raises a toast when it cannot). A plain click
/// is untouched: it takes the keyboard, selects by dragging, and reaches a program that asked for the mouse.
fn a_primary_click_opens_a_link(
    press: On<bevy::picking::events::PointerPress>,
    body: Query<&ComputedNode, With<TerminalBody>>,
    metrics: Res<CellMetrics>,
    keys: Res<ButtonInput<KeyCode>>,
    terminal: Res<Terminal>,
    mut commands: Commands,
) {
    if press.button != bevy::picking::pointer::PointerButton::Primary || !primary_held(&keys) {
        return;
    }
    let Ok(node) = body.get(press.entity) else {
        return;
    };
    let Some((row, col)) = cell_under(&press.hit, node, &metrics) else {
        return;
    };
    let Some(url) = terminal.tab().and_then(|tab| tab.screen().link_at(row, col)) else {
        return;
    };
    commands.queue(move |world: &mut World| crate::app_verbs::open_link(world, &url));
}

/// The modifiers held now, in the terminal's terms.
fn mods_held(keys: &ButtonInput<KeyCode>) -> crate::terminal::input::Mods {
    crate::terminal::input::Mods {
        shift: keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]),
        alt: keys.any_pressed([KeyCode::AltLeft, KeyCode::AltRight]),
        ctrl: keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]),
    }
}

/// The terminal's name for a picking button. The right button is not one: it opens the pane's own
/// menu (Copy, Paste, Clear), the editor's one way to those three (assumed decision on #313).
fn reported_button(button: bevy::picking::pointer::PointerButton) -> Option<crate::terminal::input::MouseButton> {
    use crate::terminal::input::MouseButton;
    match button {
        bevy::picking::pointer::PointerButton::Primary => Some(MouseButton::Left),
        bevy::picking::pointer::PointerButton::Middle => Some(MouseButton::Middle),
        bevy::picking::pointer::PointerButton::Secondary => None,
    }
}

/// **A press over the body reaches a program that asked for the mouse** (#313), at the cell under the
/// pointer — unless Shift is held, which keeps the mouse for selecting, as in xterm and VS Code.
fn a_press_reaches_the_program(
    press: On<bevy::picking::events::PointerPress>,
    body: Query<&ComputedNode, With<TerminalBody>>,
    metrics: Res<CellMetrics>,
    keys: Res<ButtonInput<KeyCode>>,
    mut pane: ResMut<TerminalPane>,
    mut terminal: ResMut<Terminal>,
) {
    use crate::terminal::input::{MouseAction, mouse_report};
    let Ok(node) = body.get(press.entity) else {
        return;
    };
    let mods = mods_held(&keys);
    let Some((_, encoding)) = terminal.tab().and_then(|tab| tab.screen().mouse_reporting()) else {
        return;
    };
    if mods.shift {
        return;
    }
    let (Some(button), Some(at)) = (reported_button(press.button), cell_under(&press.hit, node, &metrics)) else {
        return;
    };
    // **Primary+click on a link is the link's, never the program's** (#325): the program keeps the plain click.
    if primary_held(&keys) && terminal.tab().is_some_and(|tab| tab.screen().link_at(at.0, at.1).is_some()) {
        return;
    }
    if let Some(bytes) = mouse_report(button, MouseAction::Press, at, mods, encoding) {
        terminal.write(&bytes);
    }
    pane.mouse_down = Some(MouseDown { button: Some(button), from: at, last: at });
}

/// **The release of a reported press reaches the program**, wherever the pointer is let go: over the
/// body at its cell, elsewhere at the last cell reported, so a program never sees a button stuck down.
fn a_release_reaches_the_program(
    release: On<bevy::picking::events::PointerRelease>,
    body: Query<&ComputedNode, With<TerminalBody>>,
    metrics: Res<CellMetrics>,
    keys: Res<ButtonInput<KeyCode>>,
    mut pane: ResMut<TerminalPane>,
    mut terminal: ResMut<Terminal>,
) {
    use crate::terminal::input::{MouseAction, mouse_report};
    let Some(down) = pane.mouse_down else {
        return;
    };
    if reported_button(release.button) != down.button {
        return;
    }
    pane.mouse_down = None;
    let at = body
        .get(release.entity)
        .ok()
        .and_then(|node| cell_under(&release.hit, node, &metrics))
        .unwrap_or(down.last);
    let (Some(button), Some((_, encoding))) =
        (down.button, terminal.tab().and_then(|tab| tab.screen().mouse_reporting()))
    else {
        return;
    };
    if let Some(bytes) = mouse_report(button, MouseAction::Release, at, mods_held(&keys), encoding) {
        terminal.write(&bytes);
    }
}

/// Forget the anchor, and keep what was selected — the copy is the next thing the person does.
fn a_drag_ends(drag: On<bevy::picking::events::PointerDragEnd>, mut pane: ResMut<TerminalPane>) {
    if drag.button != bevy::picking::pointer::PointerButton::Primary {
        return;
    }
    pane.drag_from = None;
}

/// The cell the pointer is over, from the UI backend's normalized hit position: counted from where the rows
/// begin, past the body's inset (`terminal_view::BODY_INSET`).
fn cell_under(
    hit: &bevy::picking::backend::HitData,
    node: &ComputedNode,
    metrics: &CellMetrics,
) -> Option<(usize, usize)> {
    let normalized = hit.position?;
    let size = node.size() * node.inverse_scale_factor();
    let local = Vec2::new(normalized.x + 0.5, normalized.y + 0.5) * size - crate::terminal_view::body_inset();
    Some((
        (local.y / metrics.height()).max(0.0) as usize,
        (local.x / metrics.width()).max(0.0) as usize,
    ))
}

/// **A wheel notch goes where the screen says** (#312, #313): the pane's scrollback on the normal
/// screen; Up and Down to a full-screen program on the alternate screen, which has no scrollback; a
/// wheel report to a program that asked for the mouse. Three lines a notch, wherever it goes.
fn the_wheel_scrolls_the_scrollback(
    mut scroll: On<bevy::picking::events::PointerScroll>,
    ancestors: Query<&ChildOf>,
    bodies: Query<&ComputedNode, With<TerminalBody>>,
    metrics: Res<CellMetrics>,
    keys: Res<ButtonInput<KeyCode>>,
    mut terminal: ResMut<Terminal>,
) {
    use crate::terminal::input::{
        CursorKey, LINES_PER_NOTCH, MouseAction, MouseButton, Mods, WheelTarget, cursor_key, mouse_report,
    };
    let over = bodies.contains(scroll.entity)
        || ancestors.iter_ancestors(scroll.entity).any(|up| bodies.contains(up));
    if !over {
        return;
    }
    // Consumed: the pane is a terminal, and its scrollback is not the docks'.
    scroll.propagate(false);
    let notches = scroll.y.round() as i32;
    if notches == 0 {
        return;
    }
    let Some(tab) = terminal.tab_mut() else {
        return;
    };
    let up = notches > 0;
    let mut out = Vec::new();
    match tab.screen().wheel_target() {
        WheelTarget::Scrollback => {
            tab.screen_mut().scroll(notches * LINES_PER_NOTCH);
            return;
        }
        WheelTarget::Arrows => {
            let key = if up { CursorKey::Up } else { CursorKey::Down };
            let bytes = cursor_key(key, Mods::default(), tab.screen().app_cursor());
            for _ in 0..notches.abs() * LINES_PER_NOTCH {
                out.extend_from_slice(&bytes);
            }
        }
        WheelTarget::Report(encoding) => {
            let Some(at) = bodies.get(scroll.entity).ok().and_then(|node| cell_under(&scroll.hit, node, &metrics))
            else {
                return;
            };
            let button = if up { MouseButton::WheelUp } else { MouseButton::WheelDown };
            for _ in 0..notches.abs() {
                if let Some(bytes) = mouse_report(button, MouseAction::Press, at, mods_held(&keys), encoding) {
                    out.extend_from_slice(&bytes);
                }
            }
        }
    }
    if !out.is_empty() {
        terminal.write(&out);
    }
}

/// **A pane the person has not sized follows its screen** (#314): it opens at
/// [`TERMINAL_WINDOW_SHARE`] of the screen it is docked in and keeps that share as the window changes,
/// until a drag on the grip gives it a height of the person's own. The screen's own laid-out height is
/// the measure, not the window's: the editor draws into its surface image, and a pane built before a
/// window exists (the headless editor, a door built on the first frame) is corrected here on the first
/// frame its screen has a size.
fn fit_the_pane_to_the_window(
    terminal: Res<Terminal>,
    menus: Query<&ComputedNode, With<crate::menu_bar::EditorMenuBar>>,
    mut panes: Query<(&mut Node, &ChildOf), With<TerminalPaneRoot>>,
    screens: Query<&ComputedNode>,
) {
    if terminal.is_maximised() {
        return;
    }
    // **A drag is the person taking the height** (#492): once they have chosen one, neither the opening share nor the quarter
    // moves it, and the cycle's next turn starts from where they left it.
    if terminal.chosen_height().is_some() {
        return;
    }
    let quarter = terminal.placement() == TerminalPlacement::Quarter;
    let menu_h = menus.iter().next().map_or(0.0, |menu| menu.size().y * menu.inverse_scale_factor());
    for (mut node, parent) in &mut panes {
        let Ok(screen) = screens.get(parent.parent()) else {
            continue;
        };
        let screen_h = screen.size().y * screen.inverse_scale_factor();
        if screen_h <= 0.0 {
            continue;
        }
        let room = (screen_h - menu_h).max(0.0);
        // The quarter is the editor's body, the same room the maximise fills; the opening share is the screen, as it has been
        // since #314, and the dock's own floor keeps a small window's quarter a terminal rather than a sliver.
        let want = if quarter {
            Val::Px((room * TERMINAL_QUARTER_SHARE).max(TERMINAL_MIN_H))
        } else {
            Val::Px(terminal.opening_height(Some(screen_h)))
        };
        if node.height != want {
            node.height = want;
        }
        // **The pane is as wide as its parent, never as wide as its content** (#497). A `Percent` width is
        // the parent's, so a pane whose content is a long unwrapped row cannot be laid out as wide as that
        // row. **It is not the whole fix and the measurement says so**: at text scale 3.0 this width was
        // already `Percent(100.0)` on the pane's first frame and the pane still measured 1569 px in a
        // 1280 px window, because a flex item's *automatic least width* is its content's and that least
        // width reaches the frame root through the pane. The root's `Sizing` stating `min_width: Px(0)` is
        // what stops that (`chrome::spawn_frame`), and
        // `the_pane_is_the_windows_width_everywhere` measures all three scales.
        if node.width != Val::Percent(100.0) {
            node.width = Val::Percent(100.0);
        }
    }
}

/// **The terminal steps aside while the key overlay is up** (#314).
///
/// Holding K lays every key the editor answers to over the controls it drives
/// ([`crate::badges`]), and its legend alone is most of a 720 px window tall. The terminal holds none of
/// those controls, and with it open on every screen the overlay had a third less room: measured, the
/// Meshes door's legend came down 162 px over the shell's prompt and the Anim door's leaders crossed.
/// So for as long as the overlay is up the pane is out of the layout, and it comes back as the person
/// left it when the key goes up. The person's collapse ([`Terminal::is_collapsed`]) is not touched:
/// this is the overlay borrowing the room, not a choice about the terminal.
fn step_aside_for_the_key_overlay(
    terminal: Res<Terminal>,
    overlays: Query<&Node, (With<crate::badges::BadgeLayer>, Without<TerminalPaneRoot>)>,
    mut panes: Query<&mut Node, With<TerminalPaneRoot>>,
) {
    let overlay_up = overlays.iter().any(|node| node.display != Display::None);
    let want = if overlay_up || terminal.is_collapsed() { Display::None } else { Display::Flex };
    for mut node in &mut panes {
        if node.display != want {
            node.display = want;
        }
    }
}

/// **Drag the grip to resize**, clamped to a terminal's range. The height is remembered on the
/// resource, so the next door's pane opens where this one was left.
fn the_grip_resizes(
    drag: On<bevy::picking::events::PointerDrag>,
    grip: Query<(), With<TerminalGrip>>,
    mut panes: Query<(&mut Node, &ComputedNode), With<TerminalPaneRoot>>,
    mut terminal: ResMut<Terminal>,
) {
    if !grip.contains(drag.entity) || drag.delta.y == 0.0 || terminal.is_maximised() {
        return;
    }
    // **From the height it is drawn at**, until the person has chosen one: a pane opens at a share of the
    // window (#314), so a drag that started from the 320 px default would jump it on the first move.
    let drawn = panes.iter().next().map(|(_, computed)| computed.size().y * computed.inverse_scale_factor());
    let from = terminal.chosen_height().or(drawn).unwrap_or_else(|| terminal.height());
    // Dragging **up** makes the pane taller, so the delta is subtracted: y grows downward.
    let want = from - drag.delta.y;
    terminal.set_height(want);
    let height = terminal.height();
    for (mut node, _) in &mut panes {
        node.height = Val::Px(height);
    }
}

/// **A chip selects its session.**
fn a_chip_selects_its_tab(
    mut click: On<bevy::picking::events::PointerClick>,
    chips: Query<&TerminalTabChip>,
    mut terminal: ResMut<Terminal>,
    bar: Query<Entity, With<TerminalBar>>,
    mut commands: Commands,
) {
    let Ok(&TerminalTabChip(index)) = chips.get(click.entity) else {
        return;
    };
    click.propagate(false);
    terminal.select(index);
    redraw_the_bar(&mut commands, &bar, &terminal);
}

/// **`+` starts another shell**, in the open project.
fn a_chip_adds_a_tab(
    mut click: On<bevy::picking::events::PointerClick>,
    plus: Query<(), With<TerminalNewTab>>,
    pane: Res<TerminalPane>,
    mut terminal: ResMut<Terminal>,
    bar: Query<Entity, With<TerminalBar>>,
    mut commands: Commands,
) {
    if !plus.contains(click.entity) {
        return;
    }
    click.propagate(false);
    // **The cells the pane is actually drawing.** A shell started at a guessed 80 x 24 would print a
    // prompt wrapped for a width the pane is not showing and be resized one frame later, which is a
    // visible flash on every `+`. `TerminalPane` is written by the view's own resize system, so this
    // is the measurement the last frame made.
    let (cols, rows) = measured_cells(&pane);
    let cwd = terminal.next_cwd();
    terminal.spawn(cols, rows, &cwd);
    redraw_the_bar(&mut commands, &bar, &terminal);
}

/// **`Agent` starts another agent tab**, in the open project, with its own self test.
#[allow(clippy::too_many_arguments)]
fn a_chip_opens_an_agent(
    mut click: On<bevy::picking::events::PointerClick>,
    chip: Query<(), With<TerminalAgentChip>>,
    pane: Res<TerminalPane>,
    setup: Option<Res<crate::terminal_agent::AgentSetup>>,
    root: Option<Res<crate::project_assets::ProjectRoot>>,
    time: Res<Time<Real>>,
    mut terminal: ResMut<Terminal>,
    bar: Query<Entity, With<TerminalBar>>,
    mut commands: Commands,
) {
    if !chip.contains(click.entity) {
        return;
    }
    click.propagate(false);
    let (cols, rows) = measured_cells(&pane);
    crate::terminal_agent::open_agent_tab(&mut terminal, AgentKind::Project, cols, rows, setup.as_deref(), root.as_deref(), time.elapsed_secs_f64());
    redraw_the_bar(&mut commands, &bar, &terminal);
}

/// **`Invoke Agent` starts the agent**, in the open project, with its own self test: the one way an
/// agent starts that no person pressed for is gone.
#[allow(clippy::too_many_arguments)]
fn the_button_invokes_the_agent(
    mut click: On<bevy::picking::events::PointerClick>,
    button: Query<(), With<TerminalInvokeButton>>,
    pane: Res<TerminalPane>,
    setup: Option<Res<crate::terminal_agent::AgentSetup>>,
    root: Option<Res<crate::project_assets::ProjectRoot>>,
    time: Res<Time<Real>>,
    mut terminal: ResMut<Terminal>,
    bar: Query<Entity, With<TerminalBar>>,
    mut commands: Commands,
) {
    if !button.contains(click.entity) {
        return;
    }
    click.propagate(false);
    let (cols, rows) = measured_cells(&pane);
    crate::terminal_agent::open_agent_tab(&mut terminal, AgentKind::Project, cols, rows, setup.as_deref(), root.as_deref(), time.elapsed_secs_f64());
    redraw_the_bar(&mut commands, &bar, &terminal);
}

/// **Enter presses `Invoke Agent` for a person at the keyboard.** The button is a click target, as the
/// bar's chips are, and the agent used to start by itself: without this a keyboard-only person with an
/// agent chosen could not start it. Only while the terminal holds the keys and invites the agent.
#[allow(clippy::too_many_arguments)]
fn enter_invokes_the_agent(
    mut presses: MessageReader<bevy::input::keyboard::KeyboardInput>,
    focus: Res<TerminalFocus>,
    pane: Res<TerminalPane>,
    setup: Option<Res<crate::terminal_agent::AgentSetup>>,
    root: Option<Res<crate::project_assets::ProjectRoot>>,
    time: Res<Time<Real>>,
    mut terminal: ResMut<Terminal>,
    bar: Query<Entity, With<TerminalBar>>,
    mut commands: Commands,
) {
    // **The key messages, not `ButtonInput`**: while the terminal holds the keyboard `hold_the_keyboard`
    // clears every terminal key's `just_pressed` so the editor never sees it, and the message is what is left.
    let pressed = presses.read().any(|press| {
        press.state == bevy::input::ButtonState::Pressed && !press.repeat && matches!(press.key_code, KeyCode::Enter | KeyCode::NumpadEnter)
    });
    if !pressed || !focus.holds() || !terminal.invites_the_agent() {
        return;
    }
    let (cols, rows) = measured_cells(&pane);
    crate::terminal_agent::open_agent_tab(&mut terminal, AgentKind::Project, cols, rows, setup.as_deref(), root.as_deref(), time.elapsed_secs_f64());
    redraw_the_bar(&mut commands, &bar, &terminal);
}

/// **An agent that was running when Dark Factory last closed is started again, where it left off** (`feedback-1794148425925`).
///
/// Once per project per run: the first frame the terminal would offer `Invoke Agent` (the settings choose omp and nothing
/// is open) at a project whose resume record names an omp session ([`crate::agent_resume`]), the tab opens by itself and
/// types `omp --resume <session>`. A person who pressed `Invoke Agent` or `+` first is not overridden (a tab is open), a
/// record that cannot be read is said in the pane and nothing is started, and the record is struck out when the person ends
/// that agent, or when the resumed omp never connects (`terminal_agent::keep_the_resume_record`), so a session that cannot
/// be resumed is not tried at every launch.
#[allow(clippy::too_many_arguments)]
fn resume_the_agent_that_was_running(
    pane: Res<TerminalPane>,
    setup: Option<Res<crate::terminal_agent::AgentSetup>>,
    root: Option<Res<crate::project_assets::ProjectRoot>>,
    time: Res<Time<Real>>,
    mut terminal: ResMut<Terminal>,
    bar: Query<Entity, With<TerminalBar>>,
    mut commands: Commands,
    mut tried: Local<Option<std::path::PathBuf>>,
) {
    // **It waits for the pane** (#424): the pane starts collapsed, and an agent that woke up behind it would run where nobody can see
    // its state. A raised pane (or a person who raised it before) is when the resume happens.
    if terminal.is_collapsed() || terminal.agent() != crate::settings::TerminalAgent::OhMyPi || !terminal.invites_the_agent() {
        return;
    }
    let (Some(setup), Some(project)) = (setup.as_deref(), root.as_deref().and_then(crate::project_assets::ProjectRoot::current)) else {
        return;
    };
    if tried.as_ref() == Some(&project) {
        return;
    }
    *tried = Some(project.clone());
    match crate::agent_resume::recall(&setup.files, &project, AgentKind::Project) {
        Ok(Some(entry)) if entry.harness == crate::settings::TerminalAgent::OhMyPi.name() => {
            let (cols, rows) = measured_cells(&pane);
            crate::terminal_agent::open_agent_tab_resuming(
                &mut terminal,
                AgentKind::Project,
                cols,
                rows,
                Some(setup),
                root.as_deref(),
                Some(&entry.session),
                time.elapsed_secs_f64(),
            );
            redraw_the_bar(&mut commands, &bar, &terminal);
        }
        Ok(_) => {}
        Err(why) => terminal.refuse(format!("the agent that was running here could not be looked up, so none was resumed: {why}")),
    }
}

/// **The session a relaunch asks the new editor to restore** (`--restore-dev-agent <id>`, inserted by `main`): the editor development
/// agent that asked for the relaunch continues in this editor's terminal. Taken by `restore_the_editor_dev_agent`, once.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct RestoreDevAgent(pub String);

/// **`agent.editor_dev` (`Cmd`/`Ctrl`+`Alt`+`D`) raises the terminal and opens the editor development agent** in the editor's own
/// checkout. The chord is the terminal's own, read here as its others are ([`crate::chords::Fixed::EditorDevAgent`]).
#[allow(clippy::too_many_arguments)]
fn the_chord_opens_the_editor_dev_agent(
    keys: Res<ButtonInput<KeyCode>>,
    mut panes: Query<&mut Node, With<TerminalPaneRoot>>,
    mut focus: ResMut<TerminalFocus>,
    pane: Res<TerminalPane>,
    setup: Option<Res<crate::terminal_agent::AgentSetup>>,
    root: Option<Res<crate::project_assets::ProjectRoot>>,
    time: Res<Time<Real>>,
    mut terminal: ResMut<Terminal>,
    bar: Query<Entity, With<TerminalBar>>,
    mut commands: Commands,
) {
    if !crate::chords::chord(crate::chords::Fixed::EditorDevAgent).just_pressed(&keys) {
        return;
    }
    open_the_editor_dev_agent(&mut panes, &mut focus, &pane, setup.as_deref(), root.as_deref(), time.elapsed_secs_f64(), &mut terminal, None);
    redraw_the_bar(&mut commands, &bar, &terminal);
}

/// **The editor development agent a relaunch carried is opened again**, once the person's settings are in (no shell opens before
/// them): `omp --restore <id>` continues the turn that asked for the relaunch.
#[allow(clippy::too_many_arguments)]
fn restore_the_editor_dev_agent(
    restore: Option<Res<RestoreDevAgent>>,
    mut panes: Query<&mut Node, With<TerminalPaneRoot>>,
    mut focus: ResMut<TerminalFocus>,
    pane: Res<TerminalPane>,
    setup: Option<Res<crate::terminal_agent::AgentSetup>>,
    root: Option<Res<crate::project_assets::ProjectRoot>>,
    time: Res<Time<Real>>,
    mut terminal: ResMut<Terminal>,
    bar: Query<Entity, With<TerminalBar>>,
    mut commands: Commands,
) {
    let Some(restore) = restore else { return };
    if !terminal.settings_taken() || panes.is_empty() {
        return;
    }
    let session = restore.0.clone();
    commands.remove_resource::<RestoreDevAgent>();
    open_the_editor_dev_agent(&mut panes, &mut focus, &pane, setup.as_deref(), root.as_deref(), time.elapsed_secs_f64(), &mut terminal, Some(&session));
    redraw_the_bar(&mut commands, &bar, &terminal);
}

/// Raise the pane, give it the keys, and open the editor development agent's tab (restoring `session` when one is carried).
#[allow(clippy::too_many_arguments)]
fn open_the_editor_dev_agent(
    panes: &mut Query<&mut Node, With<TerminalPaneRoot>>,
    focus: &mut TerminalFocus,
    pane: &TerminalPane,
    setup: Option<&crate::terminal_agent::AgentSetup>,
    root: Option<&crate::project_assets::ProjectRoot>,
    now_s: f64,
    terminal: &mut Terminal,
    session: Option<&str>,
) {
    for mut node in panes.iter_mut() {
        if node.display == Display::None {
            show(&mut node, terminal, false);
        }
    }
    focus.take();
    let (cols, rows) = measured_cells(pane);
    crate::terminal_agent::open_agent_tab_resuming(terminal, AgentKind::EditorDev, cols, rows, setup, root, session, now_s);
}

/// **The status chip brings the self-test card back**, and types the self-test prompt into omp again
/// when its test failed after it connected.
fn a_status_chip_reruns_the_test(
    mut click: On<bevy::picking::events::PointerClick>,
    chip: Query<(), With<TerminalAgentStatusChip>>,
    time: Res<Time<Real>>,
    mut terminal: ResMut<Terminal>,
) {
    if !chip.contains(click.entity) {
        return;
    }
    click.propagate(false);
    let now = time.elapsed_secs_f64();
    let again = terminal.tab_mut().and_then(|tab| tab.agent_mut()).is_some_and(|post| post.rerun(now));
    // The chip is the drawn tab's, and `write` goes to the drawn tab.
    if again {
        terminal.write(format!("{}\r", crate::terminal_agent::SELF_TEST_PROMPT).as_bytes());
    }
}

/// **The cell count a shell is started with.**
///
/// Not simply what the pane last measured, and the difference is a bug that shipped for an afternoon:
/// on the frame `Ctrl+\`` raises the pane, `Display` has just gone from `None` to `Flex`, so the body
/// has **no layout yet** and measures zero — and this clamped that to one cell and started a shell
/// believing it was one column wide. The prompt then drew as a single column of characters, which
/// looked exactly like a terminal that had not started.
///
/// So a measure below a usable minimum is **not** believed: the fallback is 80 x 24, the size every
/// terminal program is written against, and the pane's own resize corrects it on the next frame with a
/// real measurement. That is one honest frame of the wrong width rather than a session that is wrong
/// until something resizes it.
pub(crate) fn measured_cells(pane: &TerminalPane) -> (u16, u16) {
    const USABLE_MIN_COLS: u16 = 20;
    const USABLE_MIN_ROWS: u16 = 5;
    let (cols, rows) = pane.cells();
    if cols < USABLE_MIN_COLS || rows < USABLE_MIN_ROWS {
        return (FALLBACK_COLS, FALLBACK_ROWS);
    }
    (cols, rows)
}

/// The size a shell is started at before the pane has ever been laid out. 80 x 24 is not an arbitrary
/// number: it is the default every terminfo entry declares and every program is tested against.
const FALLBACK_COLS: u16 = 80;
const FALLBACK_ROWS: u16 = 24;

/// **`cd <project>` in the drawn shell**, as a keystroke.
///
/// Quoted with single quotes, which POSIX shells take literally, and with any embedded quote escaped the
/// POSIX way (`'\''`) — a project directory with a space or an apostrophe in it is ordinary, and a chip
/// that broke on one would be a chip nobody trusts. The newline is what runs it.
fn a_chip_moves_the_shell(
    mut click: On<bevy::picking::events::PointerClick>,
    cd: Query<(), With<TerminalCdChip>>,
    mut terminal: ResMut<Terminal>,
    bar: Query<Entity, With<TerminalBar>>,
    mut commands: Commands,
) {
    if !cd.contains(click.entity) {
        return;
    }
    click.propagate(false);
    let root = terminal.next_cwd();
    terminal.write(format!("cd {}\n", crate::terminal_agent::shell_quote(&root.to_string_lossy())).as_bytes());
    // **The pane records what it told the shell**, because only the shell knows where it went.
    if let Some(tab) = terminal.tab_mut() {
        tab.offered_cd(root);
    }
    redraw_the_bar(&mut commands, &bar, &terminal);
}

/// **`×` closes the session being drawn**, and with it the shell (`Drop` hangs the child up).
fn a_chip_closes_a_tab(
    mut click: On<bevy::picking::events::PointerClick>,
    close: Query<(), With<TerminalCloseTab>>,
    mut terminal: ResMut<Terminal>,
    bar: Query<Entity, With<TerminalBar>>,
    mut commands: Commands,
) {
    if !close.contains(click.entity) {
        return;
    }
    click.propagate(false);
    terminal.close_active();
    redraw_the_bar(&mut commands, &bar, &terminal);
}

/// **The caret blinks on real time**, so a paused-but-visible editor still has a live cursor: the
/// shell on the other end is running whether the game clock is or not.
fn blink_the_caret(
    time: Res<Time<Real>>,
    mut pane: ResMut<TerminalPane>,
    settings: Option<Res<crate::settings::EditorSettings>>,
    carets: Query<&mut Node, With<TerminalCaret>>,
) {
    pane.blink = (pane.blink + time.delta_secs()) % CARET_PERIOD;
    // **A caret that does not blink is always lit** (#324, the person's `terminal_cursor_blink`).
    let blinks = settings.as_deref().is_none_or(|settings| settings.terminal_cursor_blink);
    let on = !blinks || pane.blink < CARET_PERIOD * 0.5;
    let want = if on { Display::Flex } else { Display::None };
    for mut node in carets {
        if node.display != want {
            node.display = want;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The cell under the pointer is counted from where the rows begin** (feedback-1794011769640;
    /// oracle `containment`). The body pads its rows [`chrome::MARGIN`] in from the sides and the
    /// terminal's body inset down from the top; the pointer was counted from the body's own corner, so a
    /// press on a character selected the one two cells to its right.
    #[test]
    fn a_press_on_a_character_is_on_that_characters_cell() {
        const SCALE: f32 = 2.4;
        let metrics = CellMetrics::default();
        let size = Vec2::new(1066.0, 200.0);
        let node = ComputedNode { size: size * SCALE, inverse_scale_factor: 1.0 / SCALE, ..default() };
        // Where the rows draw row 2, column 4: past the body's padding, then whole cells.
        let rows_begin = crate::terminal_view::body_inset();
        let (row, column) = (2usize, 4usize);
        let middle = rows_begin + Vec2::new((column as f32 + 0.5) * metrics.width(), (row as f32 + 0.5) * metrics.height());
        let normalized = middle / size - Vec2::splat(0.5);
        let hit = bevy::picking::backend::HitData::new(Entity::PLACEHOLDER, 0.0, Some(normalized.extend(0.0)), None);
        assert_eq!(
            cell_under(&hit, &node, &metrics),
            Some((row, column)),
            "a press in the middle of the character at row {row}, column {column} is on that cell"
        );
    }

    /// **The bar offers `Agent` only when the pane is not already inviting the agent** (feedback-1794088663844;
    /// oracle `spec`). An empty pane with an agent chosen has the `Invoke Agent` button; a chip that starts the same
    /// agent beside it is a second door.
    #[test]
    fn the_agent_chip_is_not_a_second_door_to_the_empty_panes_button() {
        use crate::settings::TerminalAgent;
        let mut terminal = Terminal::default();
        assert!(!offers_another_agent(&terminal), "no settings taken, no agent");
        terminal.set_agent(TerminalAgent::Off, None);
        assert!(!offers_another_agent(&terminal), "no agent chosen: nothing to add");
        terminal.set_agent(TerminalAgent::OhMyPi, None);
        assert!(terminal.invites_the_agent());
        assert!(!offers_another_agent(&terminal), "an empty pane holds the Invoke Agent button: the bar adds no second control for it");
        terminal.spawn(80, 24, std::path::Path::new("/tmp"));
        assert!(!terminal.invites_the_agent());
        assert!(offers_another_agent(&terminal), "with a tab open the chip adds another agent beside it");
    }
}
