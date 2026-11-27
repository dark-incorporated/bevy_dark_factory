//! **One surface, drawn once — the window and an agent see the same pixels.**
//!
//! Everything this editor draws goes into a single `Image`: the world from [`crate::view::MainCamera`],
//! the interface on top of it, and nothing else. The window then shows that image. An agent asking
//! `bevy_debugger/screenshot` reads the same handle, so what it gets back is what the author is
//! looking at — including the panels.
//!
//! # Why this exists
//!
//! It did not, and the cost was the author's screen.
//!
//! The editor used to draw the interface straight to the window and mirror only the *world* to an
//! offscreen square for the agent. Bevy draws a UI tree to **one** camera, so the mirror could never
//! receive a panel — measured on 2026-08-18 against a running editor, same screen, same second: the
//! BRP capture returned one chair on black, and the whole-frame sentinel capture returned the
//! `MESHES AND TILES` panel, the tab strip, the candidate list and the compass. In an editor the
//! interface is most of what there is to look at, so every visual question fell back to
//! `bevy_devshot`, which reads the **window surface** — and macOS keeps that current only while the
//! window is actually on screen (7,188 distinct colours focused, **1** behind something else). So
//! looking at a panel meant taking the display of whoever was at the machine, which is the one thing
//! the deleted `debug_capture` module was written to avoid.
//!
//! Rendering to an image and mirroring it to the window has no such dependency. `chooser.rs` already
//! did exactly this — for sharpness rather than for capture — and this module is that rig lifted to
//! serve both screens, so there is one answer to "how does this application draw" instead of two.
//!
//! # The pointer, and the one conversion
//!
//! Drawing through an image moves every screen coordinate into the image's space, and getting that
//! wrong is silent: a click lands somewhere else and nothing logs. It is one conversion, applied
//! once, and the rest of the crate was already written to receive it.
//!
//! The target is sized in **physical** pixels (see `fit_surface_to_window` for why — it is what
//! makes the type sharp) and carries `scale_factor: 1.0`, so *the image's logical pixels are physical
//! pixels*. [`crate::view::sense_pointer`] therefore stores the pointer already multiplied by the
//! window's scale factor, and then:
//!
//! - **`cursor_ground`/`cursor_ray`** feed `Camera::viewport_to_world`, which measures against
//!   `logical_viewport_rect` (`bevy_camera-0.20.0/src/camera.rs:458`) — the image's, now physical.
//!   Match, with no change at any of `cursor_ground`'s call sites.
//! - **`crate::view::over_ui`** multiplies the pointer by `camera.target_scaling_factor()`, taken
//!   from the same place `bevy_ui`'s backend takes it. Against an image target that factor is
//!   **1.0**, and a pointer already in surface space needs no second conversion. Also unchanged.
//! - **`compose::place_labels`** divides `world_to_viewport` by `Res<UiScale>`, which this module
//!   writes. It follows the surface automatically, which is what reading the resource bought.
//!
//! # UI picking has to be told where the pointer is
//!
//! This is the part that does not follow, and it is load-bearing: every panel carries `Hovered`, and
//! that is the gate stopping a click on a row from *also* dropping a piece on the map behind it.
//!
//! `bevy_ui`'s backend keeps a pointer only for cameras whose normalized `RenderTarget` **equals the
//! pointer's own target** (`bevy_ui-0.20.0/src/picking_backend.rs:129`). A mouse targets the window;
//! a camera drawing to an image never matches it, so the whole interface would stop answering the
//! pointer — with no error anywhere. `retarget_pointer` re-points it, and the same source line
//! says what position to hand over: the backend scales by `camera.target_scaling_factor()`, which is
//! 1.0 here, so the position must already be **physical**.
//!
//! Nothing about that is a second definition of where the cursor is — [`crate::view::Pointer`]
//! remains the one answer, and this writes the same number into the form the backend reads.

use bevy::camera::visibility::RenderLayers;
use bevy::camera::{ImageRenderTarget, RenderTarget};
use bevy::picking::PickingSystems;
use bevy::picking::pointer::PointerLocation;
use bevy::prelude::*;
use bevy::render::render_resource::{
    Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};

/// **The layer the mirror sprite is on, and the surface cameras are not.**
///
/// Without the split the offscreen camera drew the sprite showing its own render target — the same
/// texture as colour attachment and sampled source in one pass — and the frame came back a flat
/// `000000` with nothing in the log at all. `chooser.rs` paid for this once; the constant moves here
/// with the rig.
pub const MIRROR_LAYER: usize = 1;

/// The image every camera in this application draws into, and the one an agent reads.
#[derive(Resource)]
pub struct Surface {
    pub image: Handle<Image>,
}

/// **On every entity the rig owns, so `crate::screen::scene_roots` can leave them alone.**
///
/// The teardown sweeps every parentless entity carrying `Transform` or `Node`, which is exactly what
/// three cameras and a sprite are. Windows and monitors are already excluded there as the things
/// that outlive both screens; the surface is now a second, and for the same reason — it is *how the
/// application draws*, not something a screen made.
#[derive(Component)]
pub struct SurfaceRig;

/// Clears the surface, and draws nothing. Lowest order, always present.
///
/// A pass of its own because the two cameras above it must not clear: the world would wipe a stale
/// interface and the interface would wipe the world. On the menu screen there is no world camera at
/// all, so "let the world clear it" is not a rule that holds on both screens — and one rule that
/// holds everywhere is worth a camera that does nothing else.
#[derive(Component)]
pub struct SurfaceGround;

/// The camera the interface is drawn to — **the default UI camera for the whole application**.
///
/// `IsDefaultUiCamera` is what puts *everyone's* tree on the image rather than only the one file
/// that names a target: Bevy picks the highest-order camera rendering to the primary window when a
/// node names none (`bevy_ui-0.20.0/src/ui_node.rs:3403`), so without this the guide overlay — which
/// spawns its root bare — went to the window camera and appeared in no capture.
#[derive(Component)]
pub struct SurfaceCamera;

/// Shows the surface, and is the only camera pointed at the window.
#[derive(Component)]
pub struct WindowCamera;

/// The sprite carrying the surface into the window.
#[derive(Component)]
pub struct Mirror;

pub struct SurfacePlugin;

impl Plugin for SurfacePlugin {
    fn build(&self, app: &mut App) {
        // **Built here, not in `Startup`, and that is not a preference.**
        //
        // `bevy_state` inserts its transition schedule *before* the startup ones —
        // `schedule.insert_startup_before(PreStartup, StateTransition)`
        // (`bevy_state-0.20.0/src/app.rs:336`) — so `OnEnter(Editor)` runs before any `Startup`
        // system, and `build_headless_at` enters the editor on the first transition by design.
        // A surface spawned in `Startup` therefore does not exist when `view::setup` asks for it,
        // and in Bevy 0.20 a missing `Res<T>` panics its system: eighty tests, one message.
        //
        // Doing it at build time is also simply true — the surface is not something a frame makes.
        spawn_surface(app.world_mut());
        app
            // **A known crash lives here, and two fixes have been tried and eliminated. Read this
            // before trying a third.**
            //
            // Booting straight into a door (`bevy_dark_factory . <map>`) dies inside two seconds,
            // reproduced 4/4, while booting through the chooser never does, 0/4:
            //
            // ```text
            // Attachments have differing sizes: the depth attachment's texture view has extent
            // (3396, 1356, 1) but is followed by the color attachment at index 0's texture view
            // which has (3440, 1440, 1)
            // Quitting the application due to Validation RenderError
            // ```
            //
            // Those two extents are the window before and after the compositor finishes sizing it,
            // and `bevy_render`'s default error handler treats a validation error as **fatal**, so
            // the application exits rather than dropping a frame. The menu path survives because by
            // the time the editor's cameras exist the image has settled — which is why this reads as
            // a Map-door bug and is a startup race.
            //
            // **Eliminated 2026-09-03, both measured rather than reasoned about:**
            //
            // 1. *The viewport was stale.* `fit_viewport_to_frame` runs after `UiSystems::Layout`,
            //    which `bevy_ui` chains **after** `CameraUpdateSystems` — so a viewport written here
            //    is unvalidated until the next frame. Clamping it to the image's own extent is a
            //    real fix for a real latent invariant and it is kept below, but the crash is
            //    unchanged: the failing depth extent is the whole previous target, not a dock-hole
            //    rect, so no viewport produced it.
            // 2. *The resize landed after the cameras read it.* Moving this system to `PostUpdate`
            //    `.before(CameraUpdateSystems)` makes the new extent visible to `camera_system` in
            //    the same frame. The crash is byte-identical, so the two cameras are not
            //    disagreeing about *when* they read the target.
            //
            // What is left, and what the next person should measure first: **three cameras share
            // this one image** (ground, world, interface) and only the 3-D one has a depth
            // attachment. `prepare_core_3d_depth_textures` allocates per *target*, so a depth sized
            // for one camera's extracted target info can meet a colour attachment allocated for
            // another's. The world camera is the one spawned late — `view::setup` at
            // `OnEnter(Editor)` — which fits the startup-path evidence exactly.
            // **In `PreUpdate`, not `Update`.** The resize has to be *announced* before
            // `camera_system` reads the message stream in `PostUpdate` — and a message written
            // in `Update` is read in the same frame's `PostUpdate`, while one written in
            // `PreUpdate` is read by a `PostUpdate` that has not read the stream yet. The
            // difference is the whole crash; see the resize's own note.
            .add_systems(PreUpdate, fit_surface_to_window)
            // **After layout, because it reads it.** `PostUpdate` is where `ComputedNode` becomes
            // true for this frame; asking in `Update` would chase the previous one, which is the
            // same one-frame lag `chrome::Follow` exists to name.
            .add_systems(
                PostUpdate,
                fit_viewport_to_frame.after(bevy::ui::UiSystems::Layout),
            )
            // **Between the two picking sets that matter**: `ProcessInput` writes `PointerLocation`
            // in `PreUpdate`, `Backend` reads it in the same schedule
            // (`bevy_picking-0.20.0/src/lib.rs:259`). Anywhere else and the retarget is either
            // overwritten or too late.
            .add_systems(
                PreUpdate,
                retarget_pointer
                    .after(PickingSystems::ProcessInput)
                    .before(PickingSystems::Backend),
            );

        // **Guarded at the registration as well as the definition.** `debugger` is default-on for
        // this editor, but a build without it must still compile — and a `cfg`'d system named in an
        // unguarded `add_systems` is the shape that only fails on the configuration nobody runs.
        // **`PreUpdate`, after the fairy applies its queue and before picking consumes it**
        // (UI-30, #224). `inject_clicks` turns an injected press into a `PointerInput`, and the press
        // it reads arrives as a `MouseButtonInput` **written by `bevy_fairy` in `PreUpdate`**. It used
        // to run in `First`, the schedule *before* `PreUpdate`, so it read the message stream a frame
        // too early, found nothing, and cleared it — which is why a live drag did nothing while a
        // headless one worked: a test writes `MouseButtonInput` itself, before `First` runs, and never
        // sees the gap. Measured live: `[inject] at=Vec2(300.0, 400.0) presses=0` across 86 frames.
        #[cfg(feature = "debugger")]
        app.add_systems(
            PreUpdate,
            inject_clicks
                .after(bevy::input::InputSystems)
                .before(bevy::picking::PickingSystems::ProcessInput),
        );
    }
}

/// A starting size for a run with **no window** — a headless harness, which has none.
///
/// **Window-shaped, deliberately.** It was a 1024 square, and the square told a comfortable lie:
/// with both docks at their fixed widths, a square surface leaves a stage too narrow for the badge
/// legend to stand in at all, so the geometry the headless ratchets policed was one the windowed
/// app never renders — the first real-window capture showed the legend buried under the piece
/// list's boxes while every test was green. This is a 16:9 the docks leave a real stage inside;
/// `bevy_dark_factory::harness::resize_surface` is how a test asks for another shape.
const INITIAL_W: u32 = 1536;
const INITIAL_H: u32 = 864;

fn spawn_surface(world: &mut World) {
    // `main.rs` and `build_headless_at` both insert `ClearColor` before the editor's plugins; a
    // caller that did not gets black rather than a wrong guess.
    let ground = world
        .get_resource::<ClearColor>()
        .map_or(Color::BLACK, |c| c.0);
    // **The window's physical size, if there is a window.**
    //
    // This is the whole fix for the startup crash, and it is here — at *creation* — rather than in
    // `fit_surface_to_window`, because of where the crash comes from. The world camera is 3-D and
    // so is the only camera with a depth attachment, and `prepare_core_3d_depth_textures` sizes
    // that depth from `camera.computed.target_info.physical_size` — the size of the image **as it
    // was when the camera last refreshed**. A camera refreshes on the image's `AssetEvent`, and
    // that event is flushed in `PostUpdate` *after* `camera_system` has already read the message
    // stream, so a resize is always one frame late at the camera.
    //
    // The frame in between is the one that assembles the pass: the depth texture sized for the
    // old extent, the colour attachment the new one, and wgpu refuses a pass whose attachments
    // differ. It is fatal by default, so the application exits rather than dropping a frame —
    // and it happens on the *first* frame with a window, which is why booting straight into a
    // door dies inside two seconds and booting through the chooser never does.
    //
    // So the image is **created at the window's size** and never has to be resized at startup:
    // there is no resize, so there is no event to be late, so there is no frame in which the two
    // can disagree. `WindowPlugin::build` spawns the primary window, and `DefaultPlugins` is
    // added before the editor's plugins in both entry points, so the window is there to ask.
    //
    // **A headless run has no window** and keeps the placeholder — which is what
    // `harness::resize_surface` then resizes, and what the headless ratchets measure.
    //
    // Read before the `Assets<Image>` borrow below: the query needs the world, and `images` holds
    // it.
    let window_size = {
        let mut q = world.query_filtered::<&Window, With<bevy::window::PrimaryWindow>>();
        q.iter(world).next().map(|w| {
            (
                w.resolution.physical_width().max(1),
                w.resolution.physical_height().max(1),
            )
        })
    };
    let Some(mut images) = world.get_resource_mut::<Assets<Image>>() else {
        // Loud, because the alternative is `view::setup` panicking one schedule later on a
        // parameter whose absence says nothing about the cause. `AssetPlugin` is in `DefaultPlugins`
        // and both entry points add those first, so reaching this means the plugin order moved.
        error!("no `Assets<Image>` when the surface was built — add the editor's plugins after `DefaultPlugins`");
        return;
    };
    let (width, height) = window_size.unwrap_or((INITIAL_W, INITIAL_H));
    let size = Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let mut image = Image {
        texture_descriptor: TextureDescriptor {
            label: Some("bevy_dark_factory surface"),
            size,
            dimension: TextureDimension::D2,
            format: TextureFormat::Bgra8UnormSrgb,
            mip_level_count: 1,
            sample_count: 1,
            // `COPY_SRC` is what lets the frame be read back — without it a capture reports a target
            // it cannot read rather than writing a file.
            usage: TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_DST
                | TextureUsages::COPY_SRC
                | TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        },
        ..default()
    };
    image.resize(size);
    let handle = images.add(image);

    let target = || {
        // `RenderTarget` is its own component in 0.20 — one of `Camera`'s `#[require]`s, not a field
        // on it. **`scale_factor` stays 1.0**: Bevy's own field doc says *"This should almost always
        // be 1.0"* (`bevy_camera-0.20.0/src/camera.rs:991`) and off that path it renders nothing at
        // all. Density is carried by `UiScale` instead — see [`fit_surface_to_window`].
        RenderTarget::Image(ImageRenderTarget {
            handle: handle.clone(),
            scale_factor: 1.0,
        })
    };

    world.spawn((
        SurfaceRig,
        SurfaceGround,
        Camera2d,
        Camera {
            order: ORDER_GROUND,
            clear_color: bevy::camera::ClearColorConfig::Custom(ground),
            ..default()
        },
        target(),
        // Its own empty layer: it exists to clear, and a camera that could see the mirror would be
        // sampling the texture it is writing.
        RenderLayers::none(),
    ));

    world.spawn((
        SurfaceRig,
        SurfaceCamera,
        bevy::ui::IsDefaultUiCamera,
        Camera2d,
        Camera {
            order: ORDER_UI,
            // Never clears: the world has already drawn into this frame.
            clear_color: bevy::camera::ClearColorConfig::None,
            ..default()
        },
        target(),
        // **No layers: this camera draws the interface, and UI is not filtered by layers** (Bevy's
        // UI renderer never reads `RenderLayers`). Without this it sat on layer 0, where gizmos
        // draw by default, and drew the world's bounds box flat over the interface through its 2D
        // projection: a 300 x 38 px rectangle at the window's centre on a 300 x 38 m map, wherever
        // the Map's camera was looking (B-011).
        RenderLayers::none(),
    ));

    world.spawn((
        SurfaceRig,
        WindowCamera,
        Camera2d,
        Camera {
            order: ORDER_WINDOW,
            ..default()
        },
        // Sees the mirror and nothing else. The interface is not on this layer, so it reaches the
        // window only by way of the image — one rendering path, and a capture is the same pixels.
        RenderLayers::layer(MIRROR_LAYER),
    ));

    // Scale set by `fit_surface_to_window`: a `Camera2d`'s default projection makes one world unit
    // one *logical* pixel and the target is sized in *physical* ones, so the sprite draws at
    // `1 / scale_factor` to cover exactly the window it mirrors.
    //
    // **Its size is stated, and restated on every resize** (`fit_surface_to_window`, #307): see
    // [`mirror_size`] for why leaving Bevy to read it off the image drew a black window.
    world.spawn((
        SurfaceRig,
        Mirror,
        Sprite {
            custom_size: Some(mirror_size(size)),
            ..Sprite::from_image(handle.clone())
        },
        RenderLayers::layer(MIRROR_LAYER),
    ));

    // Only the debugger needs telling which image to read; the rig itself is not optional, because
    // it is how this application draws at all.
    #[cfg(feature = "debugger")]
    world.insert_resource(bevy_debugger_bevy::DebugCaptureTarget {
        image: handle.clone(),
    });

    world.insert_resource(Surface { image: handle });
}

/// Clear the frame. Draws nothing.
pub const ORDER_GROUND: isize = -3;
/// **The world**, which `crate::view::setup` gives [`crate::view::MainCamera`]. Named here so the
/// three passes into one image are one ordered list rather than three numbers in three files.
pub const ORDER_WORLD: isize = -2;
/// The interface, over the world.
pub const ORDER_UI: isize = -1;
/// The window, showing the finished surface. Last, so a capture taken this frame shows this frame.
pub const ORDER_WINDOW: isize = 0;

/// **The interface's scale for a window of this size**, in one place both the runtime and a test can
/// ask.
///
/// # Density is not size, and this editor was answering only one of them
///
/// `UiScale` is multiplied by the *display's* `scale_factor`, which says how many physical pixels a
/// logical one is worth — it says nothing about how much room there is. On a 3396 px window
/// reporting a factor of 1, the shipped constant left body text at 13 physical pixels and the two
/// Meshes docks using under 14 % of the width. Reported at the keyboard as a readability complaint,
/// measured as `docs/ui_audit.md` F2.
///
/// **The obvious fix — a larger constant — was tried and is wrong**, and the way it announced itself
/// is worth keeping: it is the density a *small* window also gets. At 1280 x 800 two docks of the
/// sheet's `layout.controls_w` scaled by 1.45 leave almost no stage, and
/// `no_badge_cluster_draws_through_another` went red because the badge packer had nowhere to put a
/// legend except on top of a control. A constant cannot answer a question about available room.
///
/// So the base is the density the design is drawn at, and a window wider than `REFERENCE_W` grows
/// it in proportion, up to `MAX_GROWTH`. Below the reference nothing shrinks: the design has a
/// smallest legible size and a cramped window needs the *stage* back, not smaller type.
pub fn ui_scale_for(logical_width: f32, scale_factor: f32) -> f32 {
    ui_scale_for_a_person(logical_width, scale_factor, 1.0)
}

/// **The same fit, times the interface size a person chose** (UI-73, #251).
///
/// The setting is a multiple of the skin's own spacing and target sizes — 100% is the size the editor
/// was designed at — so it multiplies the whole interface's `UiScale` and every panel grows together.
/// `UiScale` reaches every `Val::Px` and every font size, which is exactly what "the interface size"
/// means; the text size is a separate lever (`text_scale`) that moves type alone.
///
/// A person's multiple is clamped where [`crate::settings::SCALE_RANGE`] clamps it, so a hand-edited
/// file cannot ask for an interface two pixels wide or a hundred times larger.
pub fn ui_scale_for_a_person(logical_width: f32, scale_factor: f32, interface_scale: f32) -> f32 {
    let growth = (logical_width / REFERENCE_W).clamp(1.0, MAX_GROWTH);
    let wanted = interface_scale.clamp(crate::settings::SCALE_RANGE.0, crate::settings::SCALE_RANGE.1);
    crate::style::metrics().layout.ui_scale * growth * scale_factor * wanted
}

/// **What the interface size takes of a window, and what the window leaves it** (#537): the person's choice, and the choice as the
/// window can hold it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InterfaceFit {
    /// The interface size the editor is drawn at: the person's choice, or less when the window cannot hold it. At most the choice, at least
    /// [`crate::settings::SCALE_RANGE`]'s floor.
    pub effective: f32,
    /// Whether the window cut the person's choice down. The choice itself is never rewritten: a bigger window gives it back.
    pub limited: bool,
}

/// **The layout the limit is read from, measured** (#537), one row per text size, in layout units (physical pixels over `UiScale`, so they do
/// not move with the interface size): `(text size, chrome, need)`.
///
/// - **chrome**: the height of the frame that is not the body: the bars, the door strip and what stands between them (`frame - body` of the
///   laid-out frame on the Map workspace, with the bottom dock hidden).
/// - **need**: the height of the whole frame at which the tallest panel's scroll viewport is exactly one text row tall: everything above the
///   viewport (the chrome, the panel's margin, title and fixed lines) plus the row, taken from the panel that needs most and the
///   largest reading over the interface sizes 1.0 to 2.5 where the dock was hidden, rounded up, every panel read in the Map's frame.
///
/// Measured by running the census's own floors on the capture's window height (802 logical) at each text size, and checked in
/// `the_layout_table_behind_the_limit_is_what_the_layout_measures` (the census file), which re-measures both columns from the layout and fails when
/// they drift. The text-size rows are a quarter step apart.
/// A row is not linear in the text size (wrapping and snapping to whole pixels: the last quarter step adds 145 units), so the table is not fitted:
/// between two rows the need of the row above and the chrome of the row below are used, the safe side of each.
///
/// **Both columns are the Map's, which has no tab row** (#670): a one-panel workspace folds its tab into the chrome bar, so its frame is
/// [`crate::tiles::TAB_STRIP_H`] shorter above the body than the Kit's, whose three tabs keep their row. [`fit_interface_scale`] adds that
/// row back to both, so the limit holds on the Kit too and the dock comes where the shell's own rule brings it.
pub const LAYOUT_BY_TEXT_SIZE: [(f32, f32, f32); 11] = [
    (0.5, 108.0, 240.0),
    (0.75, 112.0, 267.0),
    (1.0, 118.0, 299.0),
    (1.25, 126.0, 343.0),
    (1.5, 136.0, 376.0),
    (1.75, 145.0, 450.0),
    (2.0, 154.0, 516.0),
    (2.25, 163.0, 582.0),
    (2.5, 173.0, 627.0),
    (2.75, 183.0, 694.0),
    (3.0, 192.0, 839.0),
];

/// One layout unit of slack on every edge read off the table: layout rounds to whole device pixels, and the measured readings of one cell
/// differ by up to this much from one interface size to the next.
const LAYOUT_SLACK: f32 = 1.0;

/// `(chrome, need)` at a text size: the need of the first measured row at or above it, the chrome of the last at or below it. A text size beyond
/// the table's ends is held at the end.
fn layout_at(text_scale: f32) -> (f32, f32) {
    let rows = LAYOUT_BY_TEXT_SIZE;
    let chrome = rows.iter().rev().find(|row| row.0 <= text_scale).map_or(rows[0].1, |row| row.1);
    let need = rows.iter().find(|row| row.0 >= text_scale).map_or(rows[rows.len() - 1].2, |row| row.2);
    (chrome, need)
}

/// **The interface size a window can hold** (#537): the person's choice, cut down where the window's height leaves a panel no room to be read.
///
/// The frame is `logical_height / (ui_scale * growth * size)` layout units tall, `ui_scale` being the style sheet's `metrics.layout.ui_scale`. A panel is readable when that is at least its *need*
/// ([`LAYOUT_BY_TEXT_SIZE`]); below it the chrome and the panel's own heading fill the window and its list has no row. The bottom dock is the
/// second edge: the shell shows it once the body keeps the sheet's `metrics.layout.workbench_floor_h` beside it, and where a panel needs more than that
/// (large text) the dock takes the room the panel's list needed, so the heights from there to `need` plus the dock's row are not held either (and
/// at the largest text sizes nothing below `need` plus the dock's row is), and a choice that falls in them is given the size at the far end.
///
/// **Never below the setting's own floor.** A window too short for even that is not helped (a reading of the table: at text 300% under about 460
/// logical px tall); `limited` stays false for a person who asked for the floor.
pub fn fit_interface_scale(logical_width: f32, logical_height: f32, text_scale: f32, interface_scale: f32) -> InterfaceFit {
    let (floor, ceiling) = crate::settings::SCALE_RANGE;
    let wanted = interface_scale.clamp(floor, ceiling);
    let growth = (logical_width / REFERENCE_W).clamp(1.0, MAX_GROWTH);
    let layout = &crate::style::metrics().layout;
    let per_size = layout.ui_scale * growth;
    // **The table's frame plus the workspace strip's row** ([`LAYOUT_BY_TEXT_SIZE`], #670): the Map's frame has none, the Kit's has one, and the
    // shell reads its room as if the row were there on every workspace (`shell_view::give_the_bottom_dock_its_room`), so with it the panel is
    // readable on the Kit and the dock comes where the shell brings it.
    let (chrome, need) = layout_at(text_scale);
    let (chrome, need) = (chrome + crate::tiles::TAB_STRIP_H, need + crate::tiles::TAB_STRIP_H);
    let frame_at = |size: f32| logical_height / (per_size * size);
    let size_at = |frame: f32| logical_height / (per_size * frame);
    let frame = frame_at(wanted);
    // The heights a panel's list can be read at: from `need` up to where the dock is shown, and from `need` plus the dock's row up. Where a panel
    // needs more than the dock's threshold (large text) the first range is empty.
    let dock_comes = chrome + layout.workbench_floor_h + layout.bottom_dock_h - LAYOUT_SLACK;
    let readable_with_the_dock = need + layout.bottom_dock_h + LAYOUT_SLACK;
    let held = if (frame >= need && frame < dock_comes) || frame >= readable_with_the_dock {
        frame
    } else if frame < need && need < dock_comes {
        need
    } else {
        readable_with_the_dock
    };
    let effective = if held == frame { wanted } else { size_at(held).clamp(floor, wanted) };
    InterfaceFit { effective, limited: effective < wanted - f32::EPSILON }
}

/// **`UiScale`, for a person's choices in a window of this size**: the fit of [`ui_scale_for_a_person`] at the interface size the window can
/// hold ([`fit_interface_scale`]), with what was held. The one place a window limits the choice; the fit system and the settings screen's note
/// both read it.
pub fn ui_scale_in_a_window(logical_width: f32, logical_height: f32, scale_factor: f32, text_scale: f32, interface_scale: f32) -> (f32, InterfaceFit) {
    let fit = fit_interface_scale(logical_width, logical_height, text_scale, interface_scale);
    (ui_scale_for_a_person(logical_width, scale_factor, fit.effective), fit)
}

/// The width the editor's pixel constants were chosen against — a 16:10 laptop's logical width, and
/// the size below which growth stops.
const REFERENCE_W: f32 = 1600.0;

/// **How far the interface may grow on a large display.** A cap rather than a straight ratio,
/// because a dock is a work surface and not a column of prose: past about a quarter again, a wider
/// monitor should be buying *viewport*, which is the thing an editor actually runs out of.
const MAX_GROWTH: f32 = 1.25;

/// **Fit the surface to the window, and carry the display's density in `UiScale`.**
///
/// The target is sized in **physical** pixels, and that is what makes the type sharp. Reported at the
/// keyboard: *"why isn't the text sharper? that feels like text rendered at a lower resolution and
/// then zoomed in on."* It was exactly that — the interface renders to an image and the window shows
/// that image, so the image *is* the resolution the interface is rasterised at, and it was sized in
/// logical pixels. On a 2x display every glyph edge was an interpolation between texels that were
/// never rendered.
///
/// Taking the size from the window's own `physical_*` rather than multiplying the logical size by
/// the scale factor keeps the two exactly equal, with no rounding to disagree about.
///
/// `UiScale` multiplies every `Val::Px` and every font size, so scaling it by the window's factor
/// lays the same design out twice as large in a twice-as-large target — and rasterises every glyph
/// at that size, which is the whole point. The sprite then halves it back for the window.
fn fit_surface_to_window(
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    surface: Option<Res<Surface>>,
    mut mirror: Query<(&mut Transform, &mut Sprite), With<Mirror>>,
    mut ui_scale: ResMut<UiScale>,
    settings: Option<Res<crate::settings::EditorSettings>>,
    mut images: ResMut<Assets<Image>>,
    mut events: MessageWriter<bevy::asset::AssetEvent<Image>>,
) {
    // **The interface's scale is the window's business, not the surface's**, and it is answered
    // first. A harness **does** have a `Surface` — `harness::resize_surface` makes one — so what is
    // missing there is the *mirror* and the window's physical size, not the surface. Splitting the two
    // is what lets a test of the interface-size setting read `UiScale` at a size it stated, without a
    // render target behind it.
    let Some(window) = windows.single().ok() else {
        // No window is a headless run with no size: nothing is fitted and nothing is scaled.
        return;
    };
    let sf = window.scale_factor().max(1.0);
    // **The person's interface size multiplies the fit** (UI-73, #251): the setting is the whole
    // interface's scale, so 125% enlarges the panels and the type together, in the units `UiScale`
    // already scales.
    let interface_scale = settings.as_deref().map_or(1.0, |s| s.interface_scale);
    let text_scale = settings.as_deref().map_or(1.0, |s| s.text_scale);
    // **Limited to what the window can hold** (#537), live: this runs every frame, so a resize moves it, and the stored choice is read, never written.
    let (want_ui, _) = ui_scale_in_a_window(window.resolution.width(), window.resolution.height(), sf, text_scale, interface_scale);
    if ui_scale.0 != want_ui {
        ui_scale.0 = want_ui;
    }

    // The mirror and the target: only a surface has them.
    let (Some(surface), Ok((mut mirror, mut mirror_sprite))) = (surface, mirror.single_mut()) else {
        return;
    };
    let (iw, ih) = (
        window.resolution.physical_width().max(1),
        window.resolution.physical_height().max(1),
    );

    // A `1554`-texel image drawn at `0.5` covers `777` logical pixels — the whole window, one texel
    // per physical pixel, which is what sharp means here.
    let want = Vec3::new(1.0 / sf, 1.0 / sf, 1.0);
    if mirror.scale != want {
        mirror.scale = want;
    }

    // `get_mut` marks the asset modified, so ask before writing — per the standing rule that no
    // system writes unconditionally every frame.
    let already = images.get(&surface.image).map(|i| {
        (
            i.texture_descriptor.size.width,
            i.texture_descriptor.size.height,
        )
    });
    if already != Some((iw, ih))
        && let Some(mut image) = images.get_mut(&surface.image)
    {
        let size = Extent3d {
            width: iw,
            height: ih,
            depth_or_array_layers: 1,
        };
        image.resize(size);
        // The mirror's size is restated with the image's, in the same frame: see [`mirror_size`].
        mirror_sprite.custom_size = Some(mirror_size(size));
        // **Announce the resize, in the frame the camera reads it.**
        //
        // This is the whole fix, and it is here for a reason that took reading the engine to see.
        //
        // The world camera is 3-D, and so is the only camera with a **depth attachment**.
        // `prepare_core_3d_depth_textures` sizes that depth from
        // `camera.computed.target_info.physical_size` — the image's size **as it was when the
        // camera last refreshed**. A camera refreshes on the image's `AssetEvent::Modified`, and
        // `camera_system` reads that from the **message** stream.
        //
        // The message is written by `Assets::asset_events`, which is scheduled in `PostUpdate` **in
        // `AssetEventSystems`** — and `camera_system` is scheduled `.before(AssetEventSystems)`. So
        // within one `PostUpdate` the camera reads the stream **before** the flush writes to it, and
        // learns about the resize **one frame late**. The frame in between is the one that
        // assembles the pass: a depth texture sized for the old extent and a colour attachment for
        // the new one. wgpu refuses a pass whose attachments differ, and `bevy_render`'s default
        // error handler treats that as fatal — so the application exits rather than dropping a
        // frame. That is the crash the ui_audit records, and it is why it bit the **startup**
        // path: the surface is resized on the first frame that has a window, and the world camera
        // has just been spawned.
        //
        // **So the editor writes the event itself**, in `PreUpdate` — before `camera_system` reads
        // the stream in `PostUpdate`. The camera then refreshes its `target_info` in the *same*
        // frame the image changed size, and the pass is assembled with one size.
        //
        // The event is written **by hand** because `Assets::asset_events` is the only writer of
        // that stream and it is scheduled after the reader. `AssetEvent` is a `Message`, so an
        // application may write one; and the editor is an application.
        events.write(bevy::asset::AssetEvent::Modified {
            id: surface.image.id(),
        });
    }
}

/// **The mirror's size, stated rather than read off the image** (#307): the surface's own extent, one
/// world unit per texel, which `fit_surface_to_window`'s `1 / scale_factor` then fits to the window.
///
/// Reading it off the image is the same number, and it drew a black window. Since 0.20 Bevy draws a
/// `Sprite` as a `Mesh2d` with a `SpriteMeshMaterial` it caches by the sprite's fields
/// (`bevy_sprite_render-0.20.0/src/sprite_mesh/mod.rs`, `SpriteMeshMaterialBucketKey`). An image resized
/// in place on frame 2 or 3 of a run left the sprite sampling a texture that is never drawn into, for
/// good: measured 2026-11-04 on a bare Bevy rig under Xvfb (an image camera, a window camera on layer 1,
/// this sprite), the window held 1 colour after a resize on frame 2 or 3 and 2 after one on frame 1, 5,
/// 10 or 30; touching the material asset did not bring it back. The editor's surface is resized in
/// exactly those frames, when the window learns its size (mac_air: 2176x1408, then 2176x1406), so every
/// 0.2.x release opened black. A size that changes with the image moves the sprite to a new material
/// in the frame the image changes, and the same rig then drew at every frame tried (1 to 5).
fn mirror_size(size: Extent3d) -> Vec2 {
    Vec2::new(size.width as f32, size.height as f32)
}

/// **Give the map camera the frame's hole, so the viewport is a region rather than the window.**
///
/// Panels used to float over a camera that owned the whole window, which is why the world ran under
/// them and why "is the pointer over UI" had to be asked at all before a click could mean anything
/// in the world. With a docked frame the interface has its own ground and the world has its own
/// rectangle, and this is the line between them: `chrome::ViewportSlot`'s computed rect, handed to
/// the camera as `Camera::viewport` in physical pixels.
///
/// `ComputedNode` and `UiGlobalTransform` are already in the surface's space — the interface is laid
/// out at `UiScale = EDITOR_UI_SCALE * scale_factor` into a physical-sized target — so no conversion
/// happens here, which is the whole reason [`fit_surface_to_window`] carries the density in `UiScale`
/// rather than in the target's own `scale_factor`.
///
/// `Changed<ComputedNode>`-gated and compares before writing, per the standing rule: `Camera` is
/// change-detected and the render world reads it.
///
/// # The viewport is clamped to the image, and that is a crash fix
///
/// Reproduced 2026-09-03, twice out of two, by booting straight into a door
/// (`bevy_dark_factory . <map>`): the application died inside two seconds with
///
/// ```text
/// Attachments have differing sizes: the depth attachment's texture view has extent
/// (3396, 1356, 1) but is followed by the color attachment at index 0's texture view
/// which has (3440, 1440, 1)
/// Quitting the application due to Validation RenderError
/// ```
///
/// Those two numbers are the window before and after the compositor finished sizing it. The frame
/// ordering is the whole story, and it is not obvious from either system on its own:
/// `CameraUpdateSystems` is chained **before** `UiSystems::Layout` in `PostUpdate`
/// (`bevy_ui-0.20.0/src/lib.rs:160`), so `camera_system` — which is what validates a viewport
/// against its target — has already run by the time this writes one. A viewport written here is
/// therefore unchecked until the *next* frame, and on a frame where the surface also grew, the pass
/// is assembled from a rect measured against one size and a target that is another.
///
/// Booting through the chooser never hit it, because by the time the editor's cameras exist the menu
/// has been on screen for many frames and the image has settled — which is exactly why this looked
/// like a Map-door bug rather than a startup race.
///
/// So the rect is clamped against the image's own current extent before it is handed over, and a
/// rect that cannot be made to fit is **skipped** rather than shrunk to something nobody laid out:
/// one frame of a slightly stale viewport is invisible, and the alternative is a dead application.
fn fit_viewport_to_frame(
    slot: Query<(&ComputedNode, &UiGlobalTransform), With<crate::chrome::ViewportSlot>>,
    surface: Option<Res<Surface>>,
    images: Res<Assets<Image>>,
    mut camera: Query<&mut Camera, With<crate::view::MainCamera>>,
) {
    let (Ok((node, tf)), Some(surface), Ok(mut camera)) =
        (slot.single(), surface, camera.single_mut())
    else {
        return;
    };
    let Some(target) = images.get(&surface.image).map(|i| {
        UVec2::new(
            i.texture_descriptor.size.width,
            i.texture_descriptor.size.height,
        )
    }) else {
        return;
    };
    let size = node.size();
    // A zero rect is the frame before its first layout, and a camera given a zero viewport renders
    // nothing at all — which looks exactly like a broken scene and says nothing about why.
    if size.x < 1.0 || size.y < 1.0 {
        return;
    }
    let min = tf.translation - size / 2.0;
    let position = min.max(Vec2::ZERO).as_uvec2();
    // The clamp, and the reason the whole function takes `Assets<Image>`.
    if position.x >= target.x || position.y >= target.y {
        return;
    }
    let physical_size = size.as_uvec2().min(target - position);
    if physical_size.x < 1 || physical_size.y < 1 {
        return;
    }
    let want = bevy::camera::Viewport {
        physical_position: position,
        physical_size,
        ..default()
    };
    // `Viewport` has no `PartialEq` in 0.20, so the two fields that matter are compared by hand
    // rather than the whole struct — the point is only to not mark `Camera` changed every frame.
    let same = camera.viewport.as_ref().is_some_and(|v| {
        v.physical_position == want.physical_position && v.physical_size == want.physical_size
    });
    if !same {
        camera.viewport = Some(want);
    }
}

/// **An injected click has to become a picking event, or the interface is look-but-do-not-touch.**
///
/// `retarget_pointer` gets an agent's cursor as far as `Hovered` — a row lights, a tab lights — and
/// then nothing happens when the button goes down. The reason is one layer further in:
/// `bevy_picking`'s `mouse_pick_events` reads **`WindowEvent`** messages, and takes a press's
/// position from the real cursor it has been tracking (`bevy_picking-0.20.0/src/input.rs:161`,
/// `position: *cursor_last`). The debugger writes `MouseButtonInput` messages, which `InputPlugin`
/// folds into `ButtonInput` — so the button is pressed as far as every key handler in this editor is
/// concerned, and invisible to every observer.
///
/// That gap is why the tab strip could be *hovered* over BRP and not *clicked*, and it would have
/// been read as "the observer is wrong" by whoever looked next.
///
/// So when — and only when — an agent has taken the cursor, the button drives picking too:
/// `PointerAction::Press`/`Release` at [`crate::view::Pointer`]'s position on the surface. **The
/// gate is the injected cursor**, not a flag of its own, because that is already the crate's rule
/// for who owns the pointer (`bevy_debugger_bevy::cursor_position`: *"Precedence, not fallback"*).
/// With no injected cursor this writes nothing and the real mouse takes its ordinary path, so a
/// person's click is never doubled.
///
/// `First`, after `PickingSystems::Input`, because `PointerInput` is *consumed* in `PreUpdate` by
/// `ProcessInput` — emitted any later and it would be read a frame after the press.
#[cfg(feature = "debugger")]
fn inject_clicks(
    surface: Option<Res<Surface>>,
    injected: Option<Res<bevy_debugger_bevy::DebugCursor>>,
    pointer: Option<Res<crate::view::Pointer>>,
    mut presses: MessageReader<bevy::input::mouse::MouseButtonInput>,
    mut out: MessageWriter<bevy::picking::pointer::PointerInput>,
    // **The hands on this frame's input** (DL-02, #470): a button edge forwarded here is an agent's press, and nothing else can reach this system.
    mut hands: Option<ResMut<crate::hands::Hands>>,
    // The position this system last saw, so a moved cursor writes a `Move` and a still one does not.
    mut last: Local<Option<Vec2>>,
) {
    use bevy::input::ButtonState;
    use bevy::picking::pointer::{Location, PointerAction, PointerButton, PointerId, PointerInput};

    let (Some(surface), Some(injected), Some(pointer)) = (surface, injected, pointer) else {
        return;
    };
    // No injected cursor means the real mouse owns the pointer, and `mouse_pick_events` is already
    // doing this job with the position winit gave it.
    if injected.0.is_none() {
        presses.clear();
        return;
    }
    let Some(at) = pointer.0 else {
        presses.clear();
        return;
    };
    let location = Location {
        target: bevy::camera::NormalizedRenderTarget::Image(ImageRenderTarget {
            handle: surface.image.clone(),
            scale_factor: 1.0,
        }),
        position: at,
    };
    // **The move, which Press and Release alone cannot make** (UI-30, #224).
    //
    // Measured on the fixture project this pass: after an injected press on the split's divider,
    // `PointerState` held `pressing = 1, dragging = 0` — the press reached the picker and a drag never
    // began. `bevy_picking` builds `PointerDragStart`/`PointerDrag`/`PointerDragEnd` out of
    // `PointerAction::Move` **with a non-zero delta** (`bevy_picking-0.20.0/src/events.rs:1167`), and
    // `Move` is emitted only by `mouse_pick_events` from `WindowEvent::CursorMoved` — which an
    // injected cursor never produces. So an agent could click a row and never drag a divider.
    //
    // **Only on a real move.** The delta is the difference from the position last seen, so a still
    // cursor writes nothing, and `bevy_picking` ignores a zero delta anyway. Comparing before writing
    // is the standing rule `retarget_pointer` follows for the same reason: `PointerLocation` is
    // change-detected.
    if let Some(previous) = *last {
        let delta = at - previous;
        if delta != Vec2::ZERO {
            out.write(PointerInput::new(
                PointerId::Mouse,
                location.clone(),
                PointerAction::Move { delta },
            ));
        }
    }
    *last = Some(at);
    for press in presses.read() {
        let button = match press.button {
            bevy::input::mouse::MouseButton::Left => PointerButton::Primary,
            bevy::input::mouse::MouseButton::Right => PointerButton::Secondary,
            bevy::input::mouse::MouseButton::Middle => PointerButton::Middle,
            _ => continue,
        };
        let action = match press.state {
            ButtonState::Pressed => PointerAction::Press(button),
            ButtonState::Released => PointerAction::Release(button),
        };
        // **An agent's press** (DL-02, #470): this system runs only while `DebugCursor` is out, so every edge it forwards is one the injected cursor
        // made and a person's never reaches here. `retarget_pointer` and `mouse_pick_events` own the person's own path.
        if let Some(hands) = hands.as_mut() {
            hands.say_an_agent();
        }
        out.write(PointerInput::new(PointerId::Mouse, location.clone(), action));
    }
}

/// **Tell the picking backend the pointer is on the surface, not on the window.**
///
/// See the module note: the backend keeps a pointer only for cameras whose render target equals the
/// pointer's own, so without this every `Hovered` in the editor reads false and a click on a panel
/// row falls through to the map behind it.
///
/// **The position is read from [`crate::view::Pointer`] rather than scaled in place**, and that is
/// not a style choice — it is the only form that is correct. `ProcessInput` rewrites
/// `PointerLocation` when the mouse moves and leaves it alone when it does not, so a system that
/// multiplied the field it found would scale a fresh value once and a stale one again every frame it
/// sat still. Reading the resource that already decided where the pointer is keeps one answer to
/// that question — the rule `view::Pointer`'s own doc states — and it is already in surface space.
///
/// It also buys something the window path could never give: an **injected** cursor now reaches
/// `Hovered`. `sense_pointer` deliberately never writes the window's own cursor, because that would
/// move the physical mouse, so until now an agent's pointer was invisible to picking.
///
/// One frame of latency, because `sense_pointer` runs after `PreUpdate` and this runs in it. At
/// 60 Hz that is below the threshold of a hover reading late, and the alternative is a second place
/// deciding where the cursor is.
fn retarget_pointer(
    surface: Option<Res<Surface>>,
    pointer: Option<Res<crate::view::Pointer>>,
    window: Query<(), With<bevy::window::PrimaryWindow>>,
    mut pointers: Query<&mut PointerLocation>,
) {
    let (Some(surface), Some(pointer)) = (surface, pointer) else {
        return;
    };
    // **With no window at all, nothing here knows where the pointer is, so it leaves it where picking's
    // own input put it** (UI-06, #214). `view::sense_pointer` answers `None` for "no window" as much as
    // for "off the window", and clearing on the first would erase every `PointerInput` a headless test
    // app writes (`bevy_gooey::testing::click` puts the pointer on the surface's image) before picking
    // reads it. A window that has the cursor off it still clears, below.
    //
    // **`window.is_empty()` alone, not `&& pointer.0.is_none()`** (UI-37, #231). A headless app has no
    // window but the editor's own `view::Pointer` *does* hold a position — `sense_pointer` reads the
    // cursor through `bevy_fairy`, not through the window — so the old guard fell through and this
    // system re-pointed a gooey test's click from the interface camera onto the **viewport image**.
    // Picking then hit the map, the click never reached the button, and the failure looked like a
    // toolbar that ignored clicks: measured as an observer that ran 0 times on a live 36 x 29 option.
    if window.is_empty() {
        return;
    }
    let onto = bevy::camera::NormalizedRenderTarget::Image(ImageRenderTarget {
        handle: surface.image.clone(),
        scale_factor: 1.0,
    });

    for mut slot in &mut pointers {
        match pointer.0 {
            // Off the window is not a position, and the backend's own reading of `None` is "this
            // pointer hits nothing" — which is the honest answer rather than a stale last hover.
            None => {
                if slot.location.is_some() {
                    slot.location = None;
                }
            }
            Some(at) => {
                // Compare before writing: `PointerLocation` is change-detected and the hovermap is
                // rebuilt off it, so a blind write every frame would make `Changed` meaningless for
                // everything downstream — the standing constraint `chrome::Follow` records.
                let want = bevy::picking::pointer::Location {
                    target: onto.clone(),
                    position: at,
                };
                if slot.location.as_ref() != Some(&want) {
                    slot.location = Some(want);
                }
            }
        }
    }
}

#[cfg(test)]
mod scale_tests {
    use super::{ui_scale_for, MAX_GROWTH, REFERENCE_W};
    /// The interface scale the design is drawn at: the sheet's `metrics.layout.ui_scale`.
    fn base() -> f32 {
        crate::style::metrics().layout.ui_scale
    }

    /// **A small window keeps the density the design was drawn at**, which is the half a larger
    /// constant broke. 1280 x 800 is the size every headless test lays out at, and the badge packer
    /// measures its stage in exactly those pixels.
    #[test]
    fn a_small_window_is_not_scaled_up() {
        assert_eq!(ui_scale_for(1280.0, 1.0), base());
        assert_eq!(ui_scale_for(REFERENCE_W, 1.0), base());
    }

    /// **A large one grows, and stops.**
    #[test]
    fn a_large_window_grows_to_the_cap() {
        let wide = ui_scale_for(3396.0, 1.0);
        assert_eq!(wide, base() * MAX_GROWTH);
        assert!(
            wide > ui_scale_for(REFERENCE_W, 1.0),
            "a window twice the reference width reads at a larger size, or F2 is not fixed"
        );
        // Halfway up the ramp, so the clamp is not the only thing being tested.
        assert!((ui_scale_for(1800.0, 1.0) - base() * 1.125).abs() < 1e-5);
    }

    /// **Density still multiplies on top**, because a 2x display is a separate question from a wide
    /// one and the target is sized in physical pixels either way.
    #[test]
    fn the_display_factor_still_multiplies() {
        assert_eq!(ui_scale_for(1280.0, 2.0), base() * 2.0);
    }
}
