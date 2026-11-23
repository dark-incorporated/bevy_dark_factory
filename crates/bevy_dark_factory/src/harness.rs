//! **The editor, stepped rather than run** — one plugin graph, two entry points.
//!
//! `src/main.rs` builds a window and hands control to winit. This builds the *same* editor with no
//! window, no wgpu device and no audio, and returns it for `app.update()` to step.
//!
//! # Why this exists
//!
//! Because the alternative was worse in a way that took a while to admit. Every system in this crate
//! reads and writes resources, so almost everything about the editor is checkable from a test — but
//! the one question a unit test cannot answer is *"does this app get through its first frame"*, and
//! in Bevy 0.20 that question has teeth: a missing `Res<T>` **panics its system** rather than
//! skipping it, and every run condition is evaluated with no short-circuit (`CLAUDE.md`). Those have
//! broken this editor more than any arithmetic in it.
//!
//! With no way to boot the app in a test, the only way to ask was to run the editor and look at it —
//! which meant driving a real keyboard and a real display on a machine somebody else was working on.
//! It was slow, it was unreliable (focus is stolen by whatever else is running, and a stray click
//! lands as a real edit), and it was rude.
//!
//! # This is not a second code path
//!
//! [`add_editor_plugins`] is the one list of the editor's plugins, in the one order that matters, and
//! `main.rs` calls it too. A harness that assembled its own graph would be testing an editor nobody
//! ships — the failure `sim_harness.rs` avoids by the same means, and the reason its comments insist
//! it is "the *same* plugin graph with the device omitted".
//!
//! # No GPU
//!
//! `WgpuSettings { backends: None }` registers every render type — so material plugins build and
//! `Assets<Mesh>` exists — while creating no adapter, device or queue. `sim_harness.rs` measured the
//! same trick on the game and found it changed no simulation output while dropping the GPU
//! requirement entirely. The editor needs it no less: its systems build UI trees and spawn scenes,
//! and none of that reads pixels back.
//!
//! Rendering is therefore genuinely out of scope here. "Does the highlight land on the right cell"
//! is a question for `descriptor::pick_cell`, which answers it in arithmetic.

use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};

use bevy::prelude::*;

/// **The editor's own plugins, in the order that matters.** Called by the binary and by the harness.
///
/// `KeysPlugin` is first because it owns `keys::Live`, which three of the others read as a `Res<_>`.
pub fn add_editor_plugins(app: &mut App) -> &mut App {
    app.add_plugins((
        // **First, because it is how this application draws.** Every camera below targets
        // the surface it owns, and `view::setup` reads `Res<Surface>` on `OnEnter(Editor)`.
        crate::surface::SurfacePlugin,
        // **The widget layer's machinery, and the one key it costs.**
        //
        // `FeathersPlugins` brings `TabNavigationPlugin`, whose handler claims `Tab` for focus
        // traversal — and `keys.rs` bound `Tab` to "next panel". Both would have fired: with nothing
        // focused, `dispatch_focused_input` sends to the PRIMARY WINDOW
        // (`bevy_input_focus-0.20.0/src/lib.rs:394`), which is where that observer lives, so it is
        // not gated on the editor having a focus model. The collision test reads `BINDINGS` and
        // cannot see an upstream observer, and the handler is window-scoped so every headless test
        // stays green — it would have surfaced as an author's `Tab` key doing two things at once.
        //
        // Resolved at the keyboard by **retiring `NextTab`**: it only ever did anything on the Kit
        // door, where `1`/`2`/`3` already jump to those same three panels, so what was given up is a
        // duplicate and the census is one row shorter rather than one key more overloaded. `Tab`
        // now means what it means everywhere else.
        //
        // Leaving the plugin out was the alternative and it is worse than it reads: `acquire_focus`
        // is `pub(crate)` and `click_to_focus` is private, so they cannot be registered by hand —
        // dropping the plugin drops all three permanently, and clicking a widget would never move
        // focus.
        crate::chrome::WidgetsPlugin,
        crate::keys::KeysPlugin,
        crate::chrome::ChromePlugin,
        crate::view::ViewPlugin,
        // **Nested, because `add_plugins` tuples cap at 15** (`all_tuples!(.., 0, 15, ..)`,
        // `bevy_app-0.20.0/src/plugin.rs:186`). Grouped rather than renumbered so adding the next
        // one is a nested pair again and not a re-flatten of the whole list.
        (
            crate::compass::CompassPlugin,
            // The one prompt, shared by every feature that asks a question -- see `confirm`.
            crate::confirm::ConfirmPlugin,
            crate::trusted::TrustPlugin,
            // **Whose hands are on this frame's input** (DL-02, #470): the reset the agent's pointer and chord paths write after.
            crate::hands::HandsPlugin,
            // **The project's agent guide's question** (#348, AG-03): the same modal, a different subject.
            crate::agent_guide::AgentGuidePlugin,
            // The key badges. In the shared list because a headless test drives `K` the same way a
            // person does, and because it binds nothing -- unlike `add_debugger_plugins`.
            crate::badges::BadgePlugin,
            // **The two panels that say how the editor is doing** (#509, #504): the console, a ring the `tracing` layer
            // fills with every `info`, `warn` and `error` this process said; and the frame profile, a bounded ring of the
            // frames it ran. In the shared list because they are about how the editor sees itself, not about debugging it
            // — and here rather than at the top level because that list is at its fifteen (`add_plugins`' cap).
            crate::console::ConsolePlugin,
            crate::profile::ProfilePlugin,
        ),
        crate::editor::EditorPlugin,
        crate::thumbs::ThumbsPlugin,
        crate::tiles::TilesPlugin,
        crate::compose::ComposePlugin,
        crate::anim_tab::AnimTabPlugin,
        crate::label_booth::LabelBoothPlugin,
        crate::labels::LabelsPlugin,
        crate::notice::NoticePlugin,
        // **Nested, because `add_plugins` tuples cap at 15** (`bevy_app-0.20.0/src/plugin.rs:186`,
        // `all_tuples!(impl_plugins_tuples, 0, 15, P, S)`) and `WidgetsPlugin` was the sixteenth.
        // The error names a `Plugins<_>` bound rather than the cap, so this is worth the comment.
        // Still one list, in one order — the nesting is punctuation, not a second code path.
        (
            // **The vocabulary prompt** — the tag block's `+` chip and `Shift+T` both land here.
            // In the shared list because a headless test drives `Shift+T` the same way a person does.
            crate::token_prompt::TokenPromptPlugin,
            // **The after-edit checks owed** for each agent operation, never for a person's.
            crate::after_edit::AfterEditPlugin,
            // **The decision model's status LED and the state its requests write** (DM-03, #454).
            crate::decision_model::DecisionModelPlugin,
            // Two plugins, two jobs: the capture rig is the shared crate, the verbs are ours.
            bevy_devshot::DevShotPlugin,
            crate::devshot::DrivePlugin,
            // **The file watcher** — a change on disk, in the editor, without a restart.
            //
            // In the shared list because it is about how the editor reads its own project, not
            // about debugging: the editor holds a parsed copy of the project's files, and without
            // this a developer who edited one saw nothing happen until they restarted.
            crate::watch::WatchPlugin,
            // **The capture sequencer** -- the editor takes the frames, so frame 0 is the state
            // before the operation and the operation is applied after it.
            crate::sequencer::SequencerPlugin,
            // **The one writer of `PickingSettings::is_enabled`** — both gestures that own the pointer
            // ask it, and neither writes it. See `picking.rs` for the collision this ended.
            crate::picking::PickingPlugin,
            (
                // **A report about what is wrong on screen** (`report.rs`). In the *shared* list and
                // not in `EditorPlugin`, because `Cmd+D` has to work on the Projects home too, where
                // the editor's plugins have not opened a door and none of their systems run.
                crate::report::ReportPlugin,
                // **`Cmd+=` / `Cmd+-` step the whole interface's size** (`feedback-1794783912982`).
                // In the shared list for the report's reason: the chords are about the frame, so they
                // work on the Projects home as well as in the editor.
                crate::scale_chords::ScaleChordsPlugin,
                // **A capture filed as an issue** (#495): the answer of a filing, said in the band.
                crate::report_issue::ReportIssuePlugin,
                // **Composing an issue by hand** (#499): `Cmd+Shift+I`, the six fields, the preview and one
                // filing. In the shared list for the report's reason: a person composes an issue from any
                // screen, and the headless harness drives the dialog the same way a person does.
                crate::issue_compose_panel::IssueComposePlugin,
                // **The write-through** (`--live`): every committed change is saved at once, so a game
                // watching the project reloads it. Inert unless `live::LiveWrite` is inserted.
                crate::live::LivePlugin,
                // **Background jobs a person can see and stop** (UI-22, #223): the record, the band's
                // item, and the popover that lists them. In the shared list because a headless test
                // starts a job and reads the band the same way a person does.
                crate::jobs::JobsPlugin,
                // **The app's own update** (`update.rs`): inert without the `Updater` only `main` inserts
                // inside a `.app`; in the shared list so a headless test can insert one and click it.
                crate::update::UpdatePlugin,
            ),
        ),
    ));
    // **The paint system belongs to the editor too.** `bevy_blueprint::apply_paint` is what gives a
    // painted piece its biased material, and it lives in `BlueprintPlugin`, which the game installs and
    // the editor did not. Without this, a placement's paint was written and read and drawn nowhere.
    app.add_plugins(bevy_blueprint::BlueprintPlugin);
    // **The guide vocabulary belongs in the shared list; the transport does not.**
    //
    // `add_debugger_plugins` sits outside this function because it binds a port and a test process
    // builds several `App`s. `GuidePlugin` binds nothing — it registers one-shot systems answering
    // `bool` under the editor's own names — and the harness is precisely where they are wanted:
    // `every_checkpoint_a_shipped_guide_names_is_registered` boots this app to prove no shipped
    // script names a condition nobody watches.
    //
    // The `#[cfg]` is on the module rather than on a tuple element, so the list above stays one list.
    //
    // **The first-map tour rides on it** (`tour.rs`): it posts its script through the guide's own handler and watches the steps
    // the way a client of the watch stream would, so it is wherever the guide is.
    #[cfg(feature = "debugger")]
    app.add_plugins((crate::guided::GuidePlugin, crate::tour::TourPlugin));
    // **The agent's presentation question** (S1.7). Its own `add_plugins` call rather than a tuple
    // element: the list above is one element away from Bevy's fifteen-plugin cap, and a plugin that
    // owns one question and one system does not need to be in that list.
    app.add_plugins(crate::present::PresentPlugin);
    // **The turn ledger** (DL-01, #460): one record per agent turn, written by the editor. Its own call, like the one above, for the same reason.
    app.add_plugins(crate::turn_ledger::TurnLedgerPlugin);
    // **The terminal** (a shell in the editor's own window). In the shared list rather than the
    // binary's: a headless test presses `Ctrl+\`` and types into a real shell, which is what proves
    // the feature end to end, and a plugin the harness lacks would make that test prove the harness.
    // `terminal_pane::TerminalPanePlugin` and `terminal_view::TerminalViewPlugin` come with it.
    app.add_plugins(crate::terminal::TerminalPlugin);
    app
}

/// **The agent's way in, when the feature is on.**
///
/// Separate from [`add_editor_plugins`] rather than a `#[cfg]` inside it, because that list is *the*
/// plugin graph and both entry points share it — this is a graph the harness must be able to build
/// without, and a conditional inside the shared list would make "the same plugin graph" a claim with
/// an asterisk on it.
///
/// **`DebuggerPlugin` owns `RemotePlugin`.** Its `build` adds `RemotePlugin::default()
/// .with_method_main(..)` to register the two custom methods, and Bevy rejects a duplicate plugin by
/// name — so adding `RemotePlugin` here as well panics the moment the feature is switched on. Only
/// the HTTP transport is ours to add. `docs/bevy_debugger_mcp.md` records this; the game paid for it
/// first.
///
/// The port comes from **`BEVY_BRP_PORT`**, which is the variable `bevy_debugger_mcp`'s own config
/// already reads — so one knob points both ends at the same socket, and running the editor and the
/// game with the debugger on at once is a matter of setting it rather than of learning a second
/// vocabulary.
#[cfg(feature = "debugger")]
pub fn add_debugger_plugins(app: &mut App) -> &mut App {
    let port = brp_port();
    app.add_plugins((
        bevy_debugger_bevy::DebuggerPlugin::default(),
        bevy::remote::http::RemoteHttpPlugin::default().with_port(port),
        // **The capture target is `crate::surface`'s, and it is in the SHARED list, not this one.**
        // There used to be a `DebugCapturePlugin` here owning a second `Camera3d` that mirrored the
        // map into a square image — and it could never see a panel, because Bevy draws a UI tree to
        // one camera. The editor now draws world and interface into one surface and shows it in the
        // window, so the image an agent reads is the image the author is looking at. That belongs
        // in the shared list because it is how the application draws, not a debugging addition.
        //
        // **The agent's door is here, and not in the shared list**, for the same reason the
        // transport is: it registers BRP methods, and a headless test process builds several
        // `App`s and cannot have two things bound to one port. `crate::ops_brp` registers typed
        // commands that dispatch through `crate::ops::dispatch` — the same function the key
        // handlers call — so an agent and a person are doing one thing, not two.
        crate::ops_brp::OpsBridgePlugin,
    ))
}

/// **The port the editor's remote protocol listens on**: `BEVY_BRP_PORT`, or Bevy's default. The one
/// definition both [`add_debugger_plugins`] and an agent tab's MCP config read
/// (`crate::terminal_agent::AgentSetup::port`).
#[cfg(feature = "debugger")]
pub fn brp_port() -> u16 {
    std::env::var("BEVY_BRP_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(bevy::remote::http::DEFAULT_PORT)
}

/// **Resize the headless surface**, for a test that needs a window shape the default is not.
///
/// The windowed app resizes through `fit_surface_to_window`, which needs a `PrimaryWindow`; a
/// headless run has none, so this writes the image the way that system would. Everything follows
/// from the image — the cameras render into it and the UI lays out against it — so the caller only
/// has to step a few frames afterwards for the new geometry to land. Sizes are physical texels; at
/// the harness's `UiScale` of 1.2, divide by 1.2 for logical pixels.
pub fn resize_surface(app: &mut App, width: u32, height: u32) -> Result<(), String> {
    let handle = app
        .world()
        .get_resource::<crate::surface::Surface>()
        .ok_or("no Surface resource — is the editor built?")?
        .image
        .clone();
    let mut images = app
        .world_mut()
        .get_resource_mut::<Assets<Image>>()
        .ok_or("no image assets")?;
    let Some(mut image) = images.get_mut(&handle) else {
        return Err("the surface image is gone".into());
    };
    // **A zero is the caller's bug, not a shape to round up.** Every argument here comes from a test
    // author, and a silently clamped 1 x 864 surface renders a picture no assertion is about — the
    // ratchet then measures it and passes. Two refusals above say so out loud when the surface is
    // missing; this refuses for the same reason.
    if width == 0 || height == 0 {
        return Err(format!(
            "cannot resize the surface to {width} x {height}: a zero {} is not a window shape — pass \
             the physical texels the test means",
            if width == 0 { "width" } else { "height" }
        ));
    }
    image.resize(bevy::render::render_resource::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    });
    Ok(())
}

/// The face this editor ships, for a project that does not carry its own. FiraMono (SIL OFL 1.1; the
/// licence text sits beside it in `assets/fonts/`).
const EMBEDDED_FONT: &[u8] = include_bytes!("../assets/fonts/FiraMono-Regular.ttf");

/// **The shipped face, installed over Bevy's default asset id.**
///
/// Both entry points do this, because it is not cosmetic: the embedded default is 95 codepoints of
/// ASCII, so every `—` in the editor's copy — and in the refusals `blueprint_model` hands back — draws as
/// a tofu box. A harness whose text ran through a different font would measure different layout.
///
/// **A project's own `assets/fonts/FiraMono-Regular.ttf` wins; a project without one gets the copy
/// compiled into this crate.** A game that merely *uses* the editor (Sanctuary) has no reason to
/// carry the editor's font in its own assets, and a project that cannot boot for want of one would
/// make every consumer vendor a file. Any read error other than "not found" is still an error.
pub fn install_font(app: &mut App, root: &Path) -> Result<(), String> {
    install_font_in(app.world_mut(), Some(root))
}

/// [`install_font`] on a running world, for a project opened while the app runs ([`crate::project_assets`]).
/// With no project (`None`) the face compiled into this crate is installed.
///
/// # A face over a registered face
///
/// Bevy's text pipeline (`bevy_text::load_font_assets_into_font_collection`) registers a font asset with the shaper the first
/// time it sees the asset's id and gives it the alias that handles resolve through; it remembers the ids it registered. An asset
/// put over an id it already registered arrives with no alias and is never registered, so every `Text` on the default handle then
/// draws nothing — the editor opened from the home after the home itself was drawn.
///
/// The pipeline does notice a **removed** asset: it clears its collection, registers every remaining face again and marks every
/// `TextFont` changed. So a second face is a removal now and the new face a frame later (`PendingFont`), once a frame has shown the
/// pipeline the removal (`mark_the_removal_seen`, `seat_the_pending_font`); for that frame the editor's text has no face. A
/// face with the same bytes as the one installed changes nothing, so the common case — two projects with no font of their own —
/// never blanks.
pub fn install_font_in(world: &mut World, root: Option<&Path>) -> Result<(), String> {
    let bytes = match root {
        Some(root) => {
            let path = root.join("assets/fonts/FiraMono-Regular.ttf");
            match std::fs::read(&path) {
                Ok(bytes) => bytes,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => EMBEDDED_FONT.to_vec(),
                Err(e) => return Err(format!("cannot read {}: {e}", path.display())),
            }
        }
        None => EMBEDDED_FONT.to_vec(),
    };
    let replacing = world.contains_resource::<EditorFont>();
    let mut fonts = world.resource_mut::<Assets<Font>>();
    if replacing {
        if fonts.get(AssetId::default()).is_some_and(|installed| installed.data.as_ref() == bytes.as_slice()) {
            return Ok(());
        }
        // Removed now, put back by `seat_the_pending_font` after the pipeline has seen it gone.
        fonts.remove(AssetId::default());
        world.insert_resource(PendingFont { bytes, removal_seen: false });
        return Ok(());
    }
    fonts.insert(AssetId::default(), Font::from_bytes(bytes)).map_err(|e| format!("cannot install the editor font: {e}"))?;
    world.insert_resource(EditorFont);
    Ok(())
}

/// The editor's face has been installed over the default font asset.
#[derive(Resource)]
struct EditorFont;

/// **A face waiting to replace the one removed**: [`install_font_in`] took the installed face away, and the text pipeline has
/// to see it gone before the new one can take its id.
#[derive(Resource)]
pub(crate) struct PendingFont {
    bytes: Vec<u8>,
    /// A frame has run the text pipeline since the removal.
    removal_seen: bool,
}

/// The end of a frame: the text pipeline (in `PostUpdate`) has run since a pending face's removal.
pub(crate) fn mark_the_removal_seen(pending: Option<ResMut<PendingFont>>) {
    if let Some(mut pending) = pending {
        pending.removal_seen = true;
    }
}

/// The start of the next frame, before the pipeline runs again: the pending face takes the default id, which the pipeline then
/// registers as a new face with an alias of its own.
pub(crate) fn seat_the_pending_font(mut commands: Commands, pending: Option<ResMut<PendingFont>>, mut fonts: ResMut<Assets<Font>>) {
    let Some(mut pending) = pending.filter(|p| p.removal_seen) else { return };
    let face = Font::from_bytes(std::mem::take(&mut pending.bytes));
    if let Err(e) = fonts.insert(AssetId::default(), face) {
        error!("cannot install the project's font: {e}");
    }
    commands.remove_resource::<PendingFont>();
}

/// **Stands a stepped editor on the project page of `root`**, as opening a project from the home does: the home's plugins are added when
/// they are not, the asset root and the agent bridge's root point at the project, and the screen is the page. For tests, which drive the
/// page by its widgets and by [`crate::project_page::PageAction`].
pub fn stand_on_the_project_page(app: &mut App, root: &Path) {
    if !app.is_plugin_added::<crate::launcher::LauncherPlugin>() {
        app.add_plugins(crate::launcher::LauncherPlugin);
    }
    app.world().resource::<crate::project_assets::ProjectRoot>().set(Some(root.to_path_buf()));
    app.insert_resource(crate::project_assets::PointedAt { root: root.to_path_buf() });
    app.world_mut().resource_mut::<NextState<crate::screen::Screen>>().set(crate::screen::Screen::Project);
    for _ in 0..4 {
        app.update();
    }
}

/// **A stepped editor**: the real plugins, no window, no device, no sound.
///
/// `root` is the project directory — the same argument the binary takes, and the asset root, because
/// a descriptor's `mesh` path is relative to the project.
pub fn build_headless(root: &Path, map: &str, kit: Option<&str>) -> Result<App, String> {
    build_headless_at(root, map, kit, crate::tiles::Mode::default())
}

/// **The same editor, opened on a named panel.**
///
/// The caller names the **panel** it wants and the door follows (`Workspace::showing`), because every
/// `Mode` belongs to exactly one door — asking for both would be two facts that can disagree. A test
/// that wants the Tiles panel gets the Kit door with Tiles showing.
pub fn build_headless_at(
    root: &Path,
    map: &str,
    kit: Option<&str>,
    mode: crate::tiles::Mode,
) -> Result<App, String> {
    build_stepped_at(root, map, kit, mode, Device::None)
}

/// **What renderer a stepped editor runs with.**
///
/// **One editor, two devices.** [`Device::None`] registers every render type and creates no adapter, which is
/// what every other test in this crate wants: `Assets<Mesh>`, material plugins and the UI all build, nothing
/// reads a pixel, and a test process needs no GPU. [`Device::Real`] asks wgpu for an adapter and a device, so
/// the surfaces are actually rasterised — which is the only way a test can assert *a colour at a pixel*, and
/// therefore the only way #472's cut can be proved rather than described.
///
/// **A real device is opt-in per test and skipped when there is none**, because a machine without one must not
/// fail a suite it cannot answer for: see [`real_device_available`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Device {
    None,
    Real,
}

/// **How many times a test app asks the GPU for a device** before a refusal is reported (#581). The memory that refuses one
/// belongs to other processes, and their work finishes, so a refusal says "ask again", not "this tree is broken".
pub const DEVICE_TRIES: u32 = 3;

/// **How long the harness waits between one device request and the next**, seconds: two waits, so the three requests span ten
/// seconds. A GPU out of memory is a moment, not a state.
pub const DEVICE_BACKOFF_S: u64 = 5;

/// **How many device requests the harness has made in this process**, retries included — a count a test reads to prove the retry
/// happened rather than guessing that it did.
static DEVICE_REQUESTS: AtomicU32 = AtomicU32::new(0);

/// **How many of the next device requests this process is told to refuse**, the test-only switch #581 asks for: a test proves the
/// retry without a GPU that is really out of memory.
static SIMULATED_REFUSALS: AtomicU32 = AtomicU32::new(0);

/// **The two device switches and the request count are process-wide**, so every caller that asks for a device holds this
/// lock: under `cargo test`, where a binary's tests share a process, a simulated refusal cannot be spent by another test's
/// `real_device_available`. (Nextest gives each test its own process, which is what the gate runs, so there the lock is a
/// belt as well as braces.) The callers are `real_device_available`, `build_stepped_at(Device::Real)` and the four tests of
/// the switch; `on_a_device` itself does not take it, because the switch tests hold it across the set-then-build pair.
static DEVICE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// How many device requests this process has made, retries included.
pub fn device_requests() -> u32 {
    DEVICE_REQUESTS.load(Ordering::SeqCst)
}

/// **Refuse the next `times` device requests**, the way a GPU out of memory refuses one (#581). A test sets it, builds, and sets it
/// back to `0`; anything else in the same process that asks for a device in between is refused too.
pub fn refuse_the_next_devices(times: u32) {
    SIMULATED_REFUSALS.store(times, Ordering::SeqCst);
}

/// **The reason a device could not be had, as one sentence naming the GPU**, so a reader of the gate log does not chase a regression.
fn a_refused_device(payload: &str) -> String {
    format!("the GPU was out of memory and refused a device ({payload}); {DEVICE_TRIES} requests, {DEVICE_BACKOFF_S} s apart, were all refused")
}

/// Whether a panic payload is wgpu refusing the device for memory, which is the one refusal worth asking again about.
fn refused_for_memory(payload: &(dyn std::any::Any + Send)) -> bool {
    let text = payload.downcast_ref::<&str>().map(|s| (*s).to_owned()).or_else(|| payload.downcast_ref::<String>().cloned()).unwrap_or_default();
    text.contains("OutOfMemory")
}

/// **The one place a test app asks the GPU for a device** (#581).
///
/// wgpu refuses a device with `RequestDeviceError { inner: Core(Device(OutOfMemory)) }` when the card is short of memory, and
/// `bevy_render` panics on that `Result` — so a busy host reads as a broken commit. This asks again, [`DEVICE_TRIES`] times
/// [`DEVICE_BACKOFF_S`] seconds apart, and reports a refusal as a sentence naming the GPU. A panic that is not an out-of-memory
/// refusal is resumed, because a bug must not be asked about three times.
fn on_a_device<T>(tries: u32, backoff: std::time::Duration, build: impl Fn() -> Result<T, String>) -> Result<T, String> {
    let mut refusal = String::new();
    for attempt in 1..=tries.max(1) {
        DEVICE_REQUESTS.fetch_add(1, Ordering::SeqCst);
        // The test-only switch is spent before the request, so a simulation refuses exactly the next `times` requests.
        let simulated = SIMULATED_REFUSALS.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |left| left.checked_sub(1)).is_ok();
        if !simulated {
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(&build)) {
                Ok(Ok(built)) => return Ok(built),
                // Not the device: an answer about the project, which no amount of waiting improves.
                Ok(Err(why)) => return Err(why),
                Err(payload) if refused_for_memory(payload.as_ref()) => {
                    refusal = payload.downcast_ref::<&str>().map(|s| (*s).to_owned()).or_else(|| payload.downcast_ref::<String>().cloned()).unwrap_or_default()
                }
                Err(payload) => std::panic::resume_unwind(payload),
            }
        }
        if attempt < tries {
            std::thread::sleep(backoff);
        }
    }
    Err(a_refused_device(&refusal))
}

/// **Whether this machine can rasterise at all**, so a pixel test skips rather than lies.
///
/// It asks wgpu for an adapter the same way the app will, so a host with no driver, no `/dev/dri` and no
/// llvmpipe is told so before a test builds a whole editor around a frame it can never render.
pub fn real_device_available() -> bool {
    // **The question is asked of the app itself**, by building one on the real device and reading back whether
    // the renderer produced a `RenderDevice`. That is the same code path a pixel test then uses, so a host that
    // answers "yes" here is one where the picture really is rendered — and a host with no driver answers "no"
    // rather than panicking inside `RenderPlugin`.
    let root = std::env::temp_dir().join(format!("blueprint_device_probe_{}", std::process::id()));
    if std::fs::create_dir_all(root.join("assets/blueprint")).is_err() {
        return false;
    }
    let _one_device_at_a_time = DEVICE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let asked = on_a_device(DEVICE_TRIES, std::time::Duration::from_secs(DEVICE_BACKOFF_S), || {
        let mut app = App::new();
        app.add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: None,
                    exit_condition: bevy::window::ExitCondition::DontExit,
                    close_when_requested: false,
                    ..default()
                })
                .set(bevy::render::RenderPlugin {
                    render_creation: bevy::render::settings::RenderCreation::Automatic(Box::new(
                        bevy::render::settings::WgpuSettings {
                            backends: Some(bevy::render::settings::Backends::all()),
                            ..default()
                        },
                    )),
                    ..default()
                })
                .disable::<bevy::winit::WinitPlugin>()
                .disable::<bevy::log::LogPlugin>()
                .disable::<bevy::audio::AudioPlugin>(),
        );
        Ok(app.world().get_resource::<bevy::render::renderer::RenderDevice>().is_some())
    });
    let _ = std::fs::remove_dir_all(&root);
    match asked {
        Ok(ready) => ready,
        Err(refusal) => {
            // **Said out loud with the GPU named** (#581), so the gate log carries the reason a pixel test skipped.
            eprintln!("SKIPPED: {refusal}");
            false
        }
    }
}

/// **The stepped editor, on a chosen device.** [`build_headless_at`] is this with [`Device::None`].
///
/// **A real device comes through [`on_a_device`]** (#581): the only place this crate asks the GPU for one, and the only place a
/// refusal is retried rather than reported as a failing test.
pub fn build_stepped_at(
    root: &Path,
    map: &str,
    kit: Option<&str>,
    mode: crate::tiles::Mode,
    device: Device,
) -> Result<App, String> {
    match device {
        Device::None => build_stepped(root, map, kit, mode, Device::None),
        Device::Real => {
            // **A real device is asked for under the same lock as `real_device_available`**, so a simulated refusal a test
            // sets is never spent by a build in another test of this process (2026-11-23).
            let _one_device_at_a_time = DEVICE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            on_a_device(DEVICE_TRIES, std::time::Duration::from_secs(DEVICE_BACKOFF_S), || {
                build_stepped(root, map, kit, mode, Device::Real)
            })
        }
    }
}

fn build_stepped(
    root: &Path,
    map: &str,
    kit: Option<&str>,
    mode: crate::tiles::Mode,
    device: Device,
) -> Result<App, String> {
    let workspace = crate::workspace::Workspace::showing(mode);
    let mut project = crate::project::Project::open(root, kit)?;
    // **The map is its own resource**, because four of the five doors do not have one — see
    // `project::OpenMap`. The headless harness stands up the Maps door, which does.
    let open_map = crate::project::OpenMap::open(&project, map)?;
    // **And the project remembers the map's name**, so a switching from the Kit door back to the Map
    // door knows which map to open. See `Project::open_map_name`.
    project.open_map_name = Some(open_map.map.name.clone());
    let mut app = App::new();
    // **No motion in a headless editor.** A test steps frames, not seconds: gooey's dialogs, menus and
    // colours move on the real clock, so a dialog that has been asked to close would still be on screen a
    // few frames later and every colour would be a frame behind its skin. Off lands every motion in the
    // frame it is seen (the setting the player has as Reduced, but stronger: fades land too). A test of
    // the motion itself puts `MotionSettings::default()` back and steps a manual clock.
    app.insert_resource(bevy_gooey::MotionSettings::off());

    // **The injected-pointer resource, without the plugin that owns it.**
    //
    // `view::sense_pointer` reads `DebugCursor` to let an agent aim the cursor, and in Bevy 0.20 a
    // missing `Res<T>` panics its system rather than skipping it. `add_debugger_plugins` stays out of
    // here on purpose — it binds a port, and a test process builds several `App`s — so the resource
    // has to be provided on its own. Empty, so `sense_pointer` reads the real window exactly as it
    // does for a person.
    //
    // This went red the moment the `debugger` feature became default, which is the feature working:
    // the panic named the system and the parameter.
    #[cfg(feature = "debugger")]
    app.init_resource::<bevy_debugger_bevy::DebugCursor>();

    // Absolute, because the project is the asset root and a relative root would resolve against
    // `CARGO_MANIFEST_DIR` (the CRATE dir under `cargo test -p`), not the workspace.
    let root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    // **The project as the asset root, through the same reader the binary uses**, so a test that opens a
    // second project reads the second project's meshes (`crate::project_assets`).
    crate::project_assets::install(&mut app, Some(root.clone()));
    app.add_plugins(
        DefaultPlugins
            // No window, and nothing that would make the app want to exit when it has none.
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: bevy::window::ExitCondition::DontExit,
                close_when_requested: false,
                ..default()
            })
            // Every render type, no device. See the module note.
            .set(bevy::render::RenderPlugin {
                render_creation: bevy::render::settings::RenderCreation::Automatic(Box::new(
                    bevy::render::settings::WgpuSettings {
                        // **`None` registers every render type and creates no device**; `all()` asks for a real
                        // one, which is what a pixel test needs and every other test must not pay for.
                        backends: match device {
                            Device::None => None,
                            Device::Real => Some(bevy::render::settings::Backends::all()),
                        },
                        ..default()
                    },
                )),
                ..default()
            })
            // Winit would take the thread; the point here is to step frames by hand.
            .disable::<bevy::winit::WinitPlugin>()
            // The tracing subscriber is process-global and a test binary may build several apps, so
            // the first would win and the rest would log an error per build. `sim_harness.rs` makes
            // the same call for the same reason: an `App` in a multi-`App` process must not own
            // process-global state.
            .disable::<bevy::log::LogPlugin>()
            // No sound device: `AudioPlugin` opens a real output stream and a mixer thread per app,
            // for audio no test listens to.
            .disable::<bevy::audio::AudioPlugin>(),
    )
    .insert_resource(project)
    .insert_resource(open_map)
    // **Before the plugins**, so `TilesPlugin`'s `init_resource` leaves both alone. The panel is
    // derived rather than passed: a workspace opens on its first panel (`Workspace::opens_on`), so
    // there is no second place to state where the Kit workspace starts.
    .insert_resource(workspace)
    .insert_resource(mode)
    // **Straight into the workspace.** This entry point IS a workspace — the launcher is the other
    // screen, and it is what `Screen::default()` gives. Inserting the state rather than initialising it runs
    // `OnEnter(Editor)` on the first transition, which is where every former `Startup` spawn lives.
    .insert_state(crate::screen::Screen::Editor)
    .insert_resource(ClearColor(crate::chrome::ink().void))
    // The same knob the binary sets, by the same name — a literal here would be the second
    // definition the constant exists to prevent, and headless is where a drift goes unseen.
    .insert_resource(UiScale(crate::style::metrics().layout.ui_scale));

    // **Despawning a light or a mesh must not panic in here.** `backends: None` registers every
    // render type but skips the render world — and with it `SyncWorldPlugin`, whose
    // `PendingSyncEntity` resource the `on_remove` hooks of every synced component (`PointLight`,
    // `Mesh3d`, …) unconditionally reach for. The hooks ARE registered, so the first test that
    // retired a staged figure panicked inside `sync_component.rs`. Adding the plugin gives the
    // hooks their ledger; nothing consumes the records without a render world, which over a
    // bounded test run is a small vec nobody reads — the honest cost of stepping frames deviceless.
    // Bevy 0.20 moved the plugin into `bevy_extract` and made it generic over the sub-app label.
    type SyncRender = bevy::extract::sync_world::SyncWorldPlugin<bevy::render::RenderApp>;
    if !app.is_plugin_added::<SyncRender>() {
        app.add_plugins(SyncRender::default());
    }

    // **A stepped editor waits for the project index's first listing** when it starts following the project, so its first frame
    // reads a finished listing, as it did when the listing was a walk on the frame. The binary does not wait.
    app.insert_resource(crate::project_index::WaitsForFirstListing);
    add_editor_plugins(&mut app);
    // **The screen plugin, so a test can change the door.**
    //
    // The harness stands up a door by inserting the state directly, and it did **not** register
    // `ScreenPlugin` — so `OnExit(Editor)` never ran and `close_the_door` was unreachable. That was
    // found on 2026-10-21, when the door switch's own test went green while the door's working
    // state **survived** the switching: the teardown had not run at all.
    //
    // **Registering it is safe for the other tests**, because `ScreenPlugin` only adds `OnExit` and
    // `OnEnter` schedules for `Screen`, and the harness's state is `Screen::Editor` — so `OnEnter`
    // runs on the first frame and `OnExit` only runs when a test asks to leave. The harness's own
    // `insert_state(Screen::Editor)` is what `OnEnter(Editor)` is for, and it is the *same* set of
    // spawns the binary's first frame runs.
    //
    // **Guarded, because a test may add it itself** — `leaving_a_door_survives_the_frames_after_it`
    // does, and Bevy refuses a plugin added twice.
    if !app.is_plugin_added::<crate::screen::ScreenPlugin>() {
        app.add_plugins(crate::screen::ScreenPlugin);
    }
    // **A headless run opens nothing, and asks nobody.**
    //
    // Both are the seams the features already have — [`crate::app_verbs::LinksOff`] records an
    // address instead of launching the system's opener, and [`crate::wish_view::WishSearchOff`]
    // answers a search without a network — and a test process has no business doing either for real:
    // a suite that launched browsers would be rude, and one that asked GitHub would fail on an
    // aeroplane and spend a rate limit per run. **Inserted rather than required of every test**, so
    // the honest default is the safe one and a test that wants a different answer says so
    // (`insert_resource` over these is the supported way).
    app.init_resource::<crate::app_verbs::LinksOff>();
    app.init_resource::<crate::wish_view::WishSearchOff>();
    install_font(&mut app, &root)?;
    Ok(app)
}

/// **The addresses a stepped editor opened**, in the order it opened them, for the tests that prove a
/// button opens the right page (#354).
///
/// A headless run records them ([`crate::app_verbs::LinksOff`]) rather than launching a browser, so
/// this is the record a person's press leaves behind — not a separate hook a test could pass by.
pub fn opened_links(app: &App) -> Vec<String> {
    app.world().get_resource::<crate::app_verbs::LinksOff>().map(|links| links.0.clone()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;

    use super::*;

    /// A font file for a project in a scratch folder: the embedded face and `extra` trailing bytes, so two projects' faces differ
    /// and both are still the face.
    fn project_with_a_font(tag: &str, extra: usize) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("blueprint_font_{tag}_{}", std::process::id()));
        let fonts = root.join("assets/fonts");
        std::fs::create_dir_all(&fonts).unwrap_or_else(|e| panic!("{e}"));
        let mut bytes = EMBEDDED_FONT.to_vec();
        bytes.extend(std::iter::repeat_n(0u8, extra));
        std::fs::write(fonts.join("FiraMono-Regular.ttf"), bytes).unwrap_or_else(|e| panic!("{e}"));
        root
    }

    fn installed_len(world: &World) -> Option<usize> {
        world.resource::<Assets<Font>>().get(AssetId::default()).map(|f| f.data.len())
    }

    /// **Another project's face replaces the installed one in two steps**: the removal, which the text pipeline has to see, then the
    /// new face. Put over the registered id in one go it arrives with no alias and every `Text` draws nothing.
    #[test]
    fn a_second_face_is_a_removal_and_then_the_new_face() {
        let (one, two) = (project_with_a_font("one", 0), project_with_a_font("two", 8));
        let mut world = World::new();
        world.init_resource::<Assets<Font>>();
        install_font_in(&mut world, Some(&one)).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(installed_len(&world), Some(EMBEDDED_FONT.len()), "the first project's face is installed");

        install_font_in(&mut world, Some(&two)).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(installed_len(&world), None, "the registered face is removed, not written over");

        // Before the pipeline has seen the removal nothing is put back.
        world.run_system_once(seat_the_pending_font).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(installed_len(&world), None, "a face seated in the frame of the removal would arrive unregistered");
        world.run_system_once(mark_the_removal_seen).unwrap_or_else(|e| panic!("{e}"));
        world.run_system_once(seat_the_pending_font).unwrap_or_else(|e| panic!("{e}"));
        world.flush();
        assert_eq!(installed_len(&world), Some(EMBEDDED_FONT.len() + 8), "the second project's face, once the removal was seen");
        assert!(!world.contains_resource::<PendingFont>());
        let _ = std::fs::remove_dir_all(one);
        let _ = std::fs::remove_dir_all(two);
    }

    /// **The same face again changes nothing**, so two projects that carry no font of their own never blank the text.
    #[test]
    fn the_same_face_again_leaves_the_registered_face_alone() {
        let mut world = World::new();
        world.init_resource::<Assets<Font>>();
        install_font_in(&mut world, None).unwrap_or_else(|e| panic!("{e}"));
        world.resource_mut::<Assets<Font>>().get_mut(AssetId::default()).unwrap_or_else(|| panic!("the face is installed")).alias = "registered".to_owned();

        install_font_in(&mut world, Some(Path::new("/nonexistent/project"))).unwrap_or_else(|e| panic!("{e}"));

        let alias = world.resource::<Assets<Font>>().get(AssetId::default()).map(|f| f.alias.clone());
        assert_eq!(alias.as_deref(), Some("registered"), "the registered face was touched");
        assert!(!world.contains_resource::<PendingFont>());
    }

    /// **A refusal for memory is asked again about and then succeeds** (#581), and the request count proves the retry ran rather
    /// than that the first try happened to work.
    #[test]
    fn an_out_of_memory_refusal_is_asked_again_and_the_second_ask_is_taken() {
        let _one_at_a_time = DEVICE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        refuse_the_next_devices(0);
        let before = device_requests();
        let asks = std::cell::Cell::new(0u32);
        let built = on_a_device(DEVICE_TRIES, std::time::Duration::ZERO, || {
            asks.set(asks.get() + 1);
            if asks.get() == 1 {
                panic!("called `Result::unwrap()` on an `Err` value: RequestDeviceError {{ inner: Core(Device(OutOfMemory)) }}");
            }
            Ok("the editor")
        });
        assert_eq!(built.as_deref(), Ok("the editor"), "the second ask is the one that is taken");
        assert_eq!(asks.get(), 2, "the first ask was refused and asked again");
        assert_eq!(device_requests(), before + 2, "both requests are counted, so the retry is measured and not believed");
    }

    /// **With every ask refused, the retry is off and the same simulation fails** — the other half of the proof, and the refusal is a
    /// sentence naming the GPU rather than a panic a reader would chase as a regression.
    #[test]
    fn with_the_retry_off_the_same_simulation_fails_naming_the_gpu() {
        let _one_at_a_time = DEVICE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        refuse_the_next_devices(0);
        let asks = std::cell::Cell::new(0u32);
        let refused = on_a_device(1, std::time::Duration::ZERO, || -> Result<&str, String> {
            asks.set(asks.get() + 1);
            panic!("called `Result::unwrap()` on an `Err` value: RequestDeviceError {{ inner: Core(Device(OutOfMemory)) }}")
        });
        assert_eq!(asks.get(), 1, "one try, because that is what the retry being off means");
        let why = refused.err().unwrap_or_default();
        assert!(why.starts_with("the GPU was out of memory and refused a device"), "the refusal names the GPU: {why:?}");
        assert!(why.contains("OutOfMemory"), "and it keeps wgpu's own words: {why:?}");
    }

    /// **A panic that is not about memory is resumed**, because a bug must not be asked about three times.
    #[test]
    fn a_panic_that_is_not_about_memory_is_not_retried() {
        let _one_at_a_time = DEVICE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        refuse_the_next_devices(0);
        let caught = std::panic::catch_unwind(|| on_a_device(DEVICE_TRIES, std::time::Duration::ZERO, || -> Result<(), String> { panic!("a missing `Res<T>` panics its system") }));
        assert!(caught.is_err(), "an unrelated panic is resumed rather than retried and reported as a GPU refusal");
    }

    /// **The simulated refusal is spent exactly as told**, so a test's switch cannot leak into another test's build.
    #[test]
    fn the_simulated_refusal_is_spent_one_request_at_a_time() {
        let _one_at_a_time = DEVICE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // **The simulation must survive the process**: the build under test is inside the lock, so another test's device
        // request (`real_device_available`) cannot spend the refusals (the 2026-11-23 gate: `Ok(1)`, another test had taken
        // them). The lock covers every ask in this process, which is why the four tests of this switch take it.
        refuse_the_next_devices(2);
        let before = device_requests();
        let asks = std::cell::Cell::new(0u32);
        let built = on_a_device(DEVICE_TRIES, std::time::Duration::ZERO, || {
            asks.set(asks.get() + 1);
            Ok(asks.get())
        });
        // **Three requests, one build.** A refusal never reaches the build, so the build is called once and the
        // number it returns is 1; the retry is proved by the request counter, not by that number (the assertion
        // here used to read `Ok(3)`, which counted the refusals as builds).
        assert_eq!(built, Ok(1), "the third request is the one that builds");
        assert_eq!(asks.get(), 1, "a refused request never reaches the build");
        assert_eq!(device_requests(), before + 3, "three requests were made and counted, so the retry is measured");
        refuse_the_next_devices(0);
    }
}
