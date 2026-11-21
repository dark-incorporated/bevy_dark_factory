//! **The first-map tour** (#253): four steps on a real map — pick a piece, place it, turn it, save — offered once and always
//! available from Help.
//!
//! # It is the agent's guide, not a second one
//!
//! `guided.rs` forbids a second way to load a script: an agent posts one over BRP, so the editor's own "start the tour" posts
//! `guides/first_map.json` through the **same handler** ([`bevy_debugger_bevy::handle_guide`]) and the card is the same overlay
//! (`GuideOverlayPlugin`). Nothing here knows what a step looks like; the steps' conditions are checkpoints registered in
//! `guided.rs` like every other.
//!
//! # Who watches the checkpoints
//!
//! `bevy_fairy/guide+watch` is the watching half and it is a BRP *watching* method: nothing runs it unless a client is attached.
//! An agent is that client; a person following the tour is not, so while a tour runs [`drive_the_tour`] runs the same
//! [`bevy_debugger_bevy::watch_guide`] every frame, and the step advances when its condition arrives. It runs only while a
//! [`TourRunning`] stands, so an agent's own script is never read by a second consumer.
//!
//! # Pulled, never pushed
//!
//! The editor offers the tour **once** ([`offer_the_tour`], `Recent::tour_offered`), as a toast the person can ignore. It stays
//! at Help → Take the tour. A person who chose *Take the tour* on the home's Learn page ([`crate::launcher::TourRequested`]) has
//! already said yes: the tour starts when the editor opens.
//!
//! The overlay's words are drawn in Bevy's default font, which has 95 characters: the script is plain ASCII.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy_debugger_bevy::{Guide, handle_guide, watch_guide};
use bevy_gooey::{ToastAction, toast, toast_alert};
use serde_json::Value;

use crate::editor::EditorState;
use crate::keys::ControlId;
use crate::launcher::TourRequested;
use crate::project::{OpenMap, Project};
use crate::screen::Screen;
use crate::workspace::Workspace;

/// The script the tour posts: the file an agent could post, compiled in so an app bundle has it.
pub const SCRIPT: &str = include_str!("../guides/first_map.json");

/// What the toast says.
pub const OFFER: &str = "New here? Take the 2-minute tour";

/// The toast's buttons, by what they say and the id each sends.
pub const START_LABEL: &str = "Start";
pub const NOT_NOW_LABEL: &str = "No thanks";
const START: &str = "tour-start";
const NOT_NOW: &str = "tour-no";

/// **The map as it stood when the tour began**, so a step can ask what the person did and not what was already there: the
/// example map arrives with thirteen pieces placed and a turn of one of them is not a turn made in the tour.
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct Tour {
    /// How many placements the map had.
    pub placements: usize,
    /// Each placement's yaw, in the map's order.
    pub yaws: Vec<f32>,
    /// The yaw the piece in hand was at.
    pub brush_yaw: f32,
}

impl Tour {
    /// The baseline of `open` with `state`'s piece in hand.
    pub fn of(open: &OpenMap, state: &EditorState) -> Tour {
        Tour {
            placements: open.map.placements.len(),
            yaws: open.map.placements.iter().map(|p| p.yaw).collect(),
            brush_yaw: state.brush_yaw,
        }
    }

    /// Has a piece been placed since?
    pub fn placed(&self, open: &OpenMap) -> bool {
        open.map.placements.len() > self.placements
    }

    /// **Has anything been turned since**: the piece in hand, one that was down, or one that was placed after.
    pub fn turned(&self, open: &OpenMap, state: &EditorState) -> bool {
        let placement_turned = |(at, placed): (usize, &blueprint_model::map::Placed)| match self.yaws.get(at) {
            Some(was) => (*was - placed.yaw).abs() > f32::EPSILON,
            // A piece placed in the tour starts upright unless it was turned in hand.
            None => placed.yaw.abs() > f32::EPSILON,
        };
        (state.brush_yaw - self.brush_yaw).abs() > f32::EPSILON || open.map.placements.iter().enumerate().any(placement_turned)
    }
}

/// **A tour is to start in the first frame the editor is up.** Entering the editor seats the first piece in hand and the like; a tour
/// started inside that would be started on a state it is about to change, so it waits one frame ([`start_the_due_tour`]).
#[derive(Resource, Debug, Clone, Copy)]
struct TourDue;

/// **A tour is running**: [`drive_the_tour`] watches its steps while this stands.
#[derive(Resource, Debug, Clone, Copy)]
pub struct TourRunning;

pub struct TourPlugin;

impl Plugin for TourPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PointedAt>()
            .add_observer(on_the_offer_answered)
            .add_systems(OnEnter(Screen::Editor), (offer_the_tour, spawn_the_pointer.after(crate::chrome::FrameSystems)))
            .add_systems(OnExit(Screen::Editor), end_the_tour)
            // **The pointer runs with the tour**, chained after the watch so it sees the step the watch left the card on:
            // a step that has just advanced is pointed at in the frame it arrives, not one frame late.
            .add_systems(
                Update,
                (
                    start_the_due_tour.run_if(resource_exists::<TourDue>),
                    drive_the_tour.run_if(resource_exists::<TourRunning>),
                    point_at_the_step.run_if(resource_exists::<TourRunning>),
                )
                    .chain()
                    .run_if(in_state(Screen::Editor)),
            );
    }
}

/// **Why the tour cannot start here**, or `None`. It is on a map, with a piece to pick.
pub fn why_not(world: &World) -> Option<&'static str> {
    if world.get_resource::<State<Screen>>().is_none_or(|s| *s.get() != Screen::Editor) {
        return Some("The tour is on a map: open a project first.");
    }
    if world.get_resource::<Workspace>().is_none_or(|d| *d != Workspace::Map) || world.get_resource::<OpenMap>().is_none() {
        return Some("The tour is on a map: switch to the Map first.");
    }
    if world.get_resource::<Project>().is_none_or(|p| p.library.descriptors.is_empty()) {
        return Some("This project has no pieces yet: add some meshes, then take the tour.");
    }
    None
}

/// **Start the tour**: nothing in hand, the map's baseline taken, and the script posted through the agent's handler.
pub fn start(world: &mut World) -> Result<(), String> {
    if let Some(why) = why_not(world) {
        return Err(why.to_owned());
    }
    let script: Value = serde_json::from_str(SCRIPT).map_err(|e| format!("the tour's script does not read: {e}"))?;
    // Nothing in hand, so step one is a thing the person does.
    world.resource_mut::<EditorState>().brush = None;
    let tour = Tour::of(world.resource::<OpenMap>(), world.resource::<EditorState>());
    world.insert_resource(tour);
    // **Which control each step is about**, read out of the same script: the tour's own join from a step to the node
    // `badges.rs` can find. Empty is a real state (a script with no `controls`), in which the pointer stays hidden.
    let controls = ScriptControls::of(&script);
    match world.run_system_cached_with(handle_guide, Some(script)) {
        Ok(Ok(_)) => {
            world.insert_resource(controls);
            world.insert_resource(TourRunning);
            Ok(())
        }
        Ok(Err(e)) => Err(format!("the tour was not posted: {}", e.message)),
        Err(e) => Err(format!("the tour was not posted: {e}")),
    }
}

/// Start the tour, or say why not in a toast.
pub fn start_or_say_why(world: &mut World) {
    if let Err(why) = start(world) {
        world.spawn(toast_alert(why).sticky());
    }
}

/// The frame after the editor was entered: start the tour that was asked for.
fn start_the_due_tour(world: &mut World) {
    world.remove_resource::<TourDue>();
    start_or_say_why(world);
}

/// **Watch the running tour's step**, as a client of `guide+watch` would. The script finishing, or failing to be watched at all,
/// ends the run; the card hides itself once the last step's confirmation has been shown.
fn drive_the_tour(world: &mut World) {
    match world.run_system_cached_with(watch_guide, None) {
        Ok(Ok(Some(answer))) if answer.get("done").and_then(Value::as_bool) == Some(true) => finish(world),
        Ok(Ok(_)) => {}
        Ok(Err(e)) => {
            warn!("the tour cannot be watched: {}", e.message);
            end(world);
        }
        Err(e) => {
            warn!("the tour cannot be watched: {e}");
            end(world);
        }
    }
}

/// The script is done: stop watching, and let the card show its last confirmation and go.
fn finish(world: &mut World) {
    world.remove_resource::<ScriptControls>();
    world.remove_resource::<PointedAt>();
    world.remove_resource::<TourRunning>();
    world.remove_resource::<Tour>();
}

/// The tour is over before its script is: take the card down.
fn end(world: &mut World) {
    if let Some(mut guide) = world.get_resource_mut::<Guide>() {
        guide.clear();
    }
    finish(world);
    // The pointer goes with the card: a layer pointing at a step nobody is on is a mark with no meaning, and the
    // `without_tour_running` branch of [`point_at_the_step`] is what hides it on the next frame.
}

/// A tour belongs to the map it was started on: closing the project takes it down.
fn end_the_tour(world: &mut World) {
    if world.contains_resource::<TourRunning>() {
        end(world);
    }
}

/// **On entering the editor: start the tour a person asked for, or offer it once.**
fn offer_the_tour(
    mut commands: Commands,
    recents: Option<Res<crate::recent::RecentProjects>>,
    requested: Option<Res<TourRequested>>,
    project: Option<Res<Project>>,
) {
    let requested = requested.is_some();
    if requested {
        commands.remove_resource::<TourRequested>();
    }
    let Some(file) = recents.as_deref().map(|r| r.0.clone()) else {
        if requested {
            commands.insert_resource(TourDue);
        }
        return;
    };
    let offered = match crate::recent::Recent::load(&file) {
        Ok(recent) => recent.tour_offered,
        Err(e) => {
            warn!("the tour is not offered: {e}");
            return;
        }
    };
    if requested {
        commands.insert_resource(TourDue);
    } else if !offered && project.is_some_and(|p| !p.library.descriptors.is_empty()) {
        commands.spawn(toast(OFFER).action(START_LABEL, START).action(NOT_NOW_LABEL, NOT_NOW));
    } else {
        return;
    }
    // Said once: whichever way it is answered, or ignored.
    if !offered
        && let Err(e) = crate::recent::mark_tour_offered(&file)
    {
        warn!("the tour's offer was not remembered: {e}");
    }
}

/// The offer's **Start**.
fn on_the_offer_answered(action: On<ToastAction>, mut commands: Commands) {
    if action.id == START {
        commands.queue(start_or_say_why);
    }
}

// ── The pointer: where the step's control is ────────────────────────────────────────────────────────

/// **Which control a step is about**, by the step's own label, read out of the script the tour posted.
///
/// `guides/*.json` carries `"controls": {"Pick a piece": "Palette"}` beside its `steps`, so a step points at
/// the control `badges.rs` can find and the badge and the pointer ask the same question with the same answer.
/// The field lives at the top of the script rather than on a `Step` because `bevy_debugger_bevy::Step` is
/// `deny_unknown_fields` and the card is the third-party crate's surface (`guided.rs:44`); the tour joins the
/// two by label, which is the one key both sides already carry.
#[derive(Resource, Debug, Clone, Default)]
pub struct ScriptControls {
    by_label: std::collections::HashMap<String, String>,
}

impl ScriptControls {
    /// The controls a script names, resolved from its top-level `controls` object.
    pub fn of(script: &Value) -> Self {
        let by_label = script
            .get("controls")
            .and_then(Value::as_object)
            .map(|m| {
                m.iter()
                    .filter_map(|(label, id)| id.as_str().map(|id| (label.clone(), id.to_owned())))
                    .collect()
            })
            .unwrap_or_default();
        ScriptControls { by_label }
    }

    /// The control a step labelled `label` is about, if it is about one.
    pub fn control(&self, label: &str) -> Option<ControlId> {
        let name = self.by_label.get(label)?;
        ControlId::named(name)
    }

    /// The control's name as the guide spells it, for the problem line when it cannot be resolved.
    pub fn name(&self, label: &str) -> Option<&str> {
        self.by_label.get(label).map(String::as_str)
    }

    pub fn is_empty(&self) -> bool {
        self.by_label.is_empty()
    }
}

/// **The id a guide names**, for the corpus tests and the pointer alike: one spelling, one place.
pub fn control_of(name: &str) -> Option<ControlId> {
    ControlId::named(name)
}

/// **Which side of a target the arrow sits on**, and so which way it points.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ArrowSide {
    /// Under the target, pointing up — the ordinary case.
    Below,
    /// Above the target, pointing down — for a target with no room below it, where an arrow under it would be clipped.
    Above,
}

impl ArrowSide {
    /// The chevron a [`ArrowSide`] draws. Both are in the shipped face `assets/fonts/FiraMono-Regular.ttf`
    /// (`docs/design/icon-vocabulary.md` holds the measurement that made the fold's marks chevrons).
    pub fn glyph(self) -> char {
        match self {
            ArrowSide::Below => '\u{25b2}',
            ArrowSide::Above => '\u{25bc}',
        }
    }
}

/// How many logical pixels the arrow stands off its target's edge.
const ARROW_GAP: f32 = 4.0;

/// **Where the arrow goes for a target**, given the window and the arrow's own size.
///
/// A target with its own height to spare under the window's foot gets the arrow below it; one within that of the
/// bottom gets it above and pointing down — the case `renzora_tutorial/src/highlight.rs:1-20` names (the status-bar
/// menu). `arrow` is the arrow's own height, so "within its own height of the window's bottom" is the test.
pub fn arrow_side(target: Rect, window: Rect, arrow: f32) -> ArrowSide {
    if target.max.y + ARROW_GAP + arrow <= window.max.y {
        ArrowSide::Below
    } else {
        ArrowSide::Above
    }
}

/// **The tour's pointer layer**: a click-through box over the step's control and an arrow beside it.
#[derive(Component)]
pub struct TourPointerLayer;

/// The outline node, positioned each frame from [`crate::badges::control_rect`].
#[derive(Component)]
struct TourOutline;

/// The arrow's node and its glyph.
#[derive(Component)]
struct TourArrow;

/// **The layer that points at the step's control**, spawned once and hidden until a step names one.
fn spawn_the_pointer(mut commands: Commands, frame: Res<crate::chrome::Frame>) {
    let ink = crate::chrome::ink();
    let layer = commands
        .spawn((
            TourPointerLayer,
            // Covers the window so a child may be placed anywhere in it. `cover()` is `placed(..)`, which already carries
            // `Pickable::IGNORE` — the click-through property, so the press the step asks for reaches the control under
            // the pointer. Adding it again would be a bundle with one component twice, which bevy panics at spawn over.
            bevy_gooey::cover(),
            bevy_gooey::Shown(false),
            // Above the panels and below a modal question, the tier the badges hold: the pointer reads over the interface
            // it points into, and a dialog that has taken the keyboard still outranks it.
            GlobalZIndex(crate::chrome::BADGE_Z),
        ))
        .id();
    commands.entity(layer).with_children(|p| {
        // **The outline.** A box laid over the control's rect, `placed(..)` (and so `Pickable::IGNORE`) so the click the
        // step asks for falls through it: the whole reason the box is click-through, `highlight.rs`'s `PASS_THROUGH`.
        p.spawn((
            TourOutline,
            bevy_gooey::placed(UiRect::all(Val::Auto)),
            bevy_gooey::HostGeometry,
            bevy_gooey::BorderPx(bevy_gooey::Sides::all(OUTLINE_PX)),
            BorderColor::all(ink.accent),
            BackgroundColor(Color::NONE),
            Visibility::Hidden,
        ));
        // **The arrow**, a glyph on a placed node — the `Text` is on the node itself, so the system writes the glyph where
        // it writes the position.
        p.spawn((
            TourArrow,
            bevy_gooey::placed(UiRect::all(Val::Auto)),
            bevy_gooey::HostGeometry,
            Visibility::Hidden,
            Text::new(ArrowSide::Below.glyph().to_string()),
            TextColor(ink.accent),
            crate::chrome::typed(crate::chrome::text::BODY),
        ));
    });
    commands.entity(frame.root).add_child(layer);
}

/// How thick the outline is, in logical pixels. One hairline, the way a badge's box is edged.
const OUTLINE_PX: f32 = 2.0;

/// **Point at the control the current step is about**, or say why it cannot.**
///
/// Reads the step's label, joins it to a [`ControlId`] through [`ScriptControls`], resolves the rect through the badge
/// module's own locator ([`crate::badges::rect_of`] — the one answer to *"where is this control"*, reused rather than
/// restated), and writes the outline and arrow from it. A step that names a control which is not on screen **raises a
/// problem** naming the step and the control rather than drawing nothing — `renzora_tutorial`'s silent miss is the
/// failure this is designed against, and it is raised once per step rather than once a frame.
/// **What a step reads** — its resources, bundled so `point_at_the_step` is under clippy's limit.
#[derive(bevy::ecs::system::SystemParam)]
struct StepReads<'w, 's> {
    guide: Option<Res<'w, Guide>>,
    controls: Option<Res<'w, ScriptControls>>,
    frame: Option<Res<'w, crate::chrome::Frame>>,
    ui_scale: Res<'w, UiScale>,
    _marker: std::marker::PhantomData<&'s ()>,
}

/// A node and its children, shown by the tour's pointer layer.
type LayerQuery<'w, 's> = Query<'w, 's, (&'static mut bevy_gooey::Shown, &'static Children),
    With<TourPointerLayer>>;
/// A node's placement and visibility — the outline and the arrow share the shape.
type PlaceQuery<'w, 's, F> = Query<'w, 's, (&'static mut Node, &'static mut Visibility), F>;
/// The arrow also carries its own text.
type ArrowQuery<'w, 's> = Query<'w, 's,
    (&'static mut Node, &'static mut Visibility, &'static mut Text),
    (With<TourArrow>, Without<TourOutline>)>;
/// Where every drawn box stands, which is what the tour aims at.
type BoxQuery<'w, 's> = Query<'w, 's, (&'static ComputedNode, &'static UiGlobalTransform)>;
/// A control that is on screen, with the box it occupies.
type ScreenQuery<'w, 's> = Query<'w, 's,
    (Entity, &'static crate::chrome::Control, &'static ComputedNode, &'static UiGlobalTransform)>;

/// **What a step draws into** — the five queries, named rather than six complex types inline.
#[derive(bevy::ecs::system::SystemParam)]
struct StepQueries<'w, 's> {
    layers: LayerQuery<'w, 's>,
    outline: PlaceQuery<'w, 's, (With<TourOutline>, Without<TourArrow>)>,
    arrow: ArrowQuery<'w, 's>,
    boxes: BoxQuery<'w, 's>,
    controls_on_screen: ScreenQuery<'w, 's>,
}

fn point_at_the_step(
    StepReads { guide, controls, frame, ui_scale, .. }: StepReads<'_, '_>,
    mut said: Option<ResMut<EditorState>>,
    mut complained: ResMut<PointedAt>,
    drawn: StepQueries<'_, '_>,
) {
    let StepQueries { mut layers, mut outline, mut arrow, boxes, controls_on_screen } = drawn;
    let (Some(guide), Some(controls)) = (guide, controls) else {
        return;
    };
    let Some(window) = frame.as_deref().and_then(|f| {
        boxes.get(f.root).ok().and_then(|(n, tf)| {
            let size = n.size();
            (size != Vec2::ZERO).then(|| Rect::from_center_size(tf.translation, size))
        })
    }) else {
        return;
    };
    let scale = if ui_scale.0 > 0.0 { ui_scale.0 } else { 1.0 };

    // What the step is about, if it names a control.
    let label = guide.current().map(|s| s.label.clone());
    let want = label
        .as_deref()
        .and_then(|l| controls.control(l).map(|id| (l.to_owned(), id)));
    let Some((label, id)) = want else {
        complained.0 = None;
        for (mut shown, _) in &mut layers {
            shown.0 = false;
        }
        return;
    };

    let Some(rect) = crate::badges::rect_of(id, &controls_on_screen) else {
        for (mut shown, _) in &mut layers {
            shown.0 = false;
        }
        // **Once per step, not once a frame.** A problem repeats fold in `Status` (`xN`), and a step that is
        // pointed at nothing for a minute would show a minute of repeats beside a guide fault that happened once.
        if complained.0.as_deref() != Some(label.as_str()) {
            let name = controls.name(&label).unwrap_or("that control");
            if let Some(state) = said.as_deref_mut() {
                state.status.problem(format!(
                    "The tour's step `{label}` points at {name}, which is not on screen. That is a fault \
                     in the guide, so the step cannot be followed as written"
                ));
            }
            complained.0 = Some(label.clone());
        }
        return;
    };

    let side = arrow_side(rect, window, ARROW_SIZE * scale);
    for (mut shown, children) in &mut layers {
        if !shown.0 {
            shown.0 = true;
        }
        for child in children.iter() {
            if let Ok((mut node, mut vis)) = outline.get_mut(child) {
                let (l, t) = (Val::Px(rect.min.x / scale), Val::Px(rect.min.y / scale));
                let (w, h) = (Val::Px(rect.width() / scale), Val::Px(rect.height() / scale));
                if node.left != l {
                    node.left = l;
                }
                if node.top != t {
                    node.top = t;
                }
                if node.width != w {
                    node.width = w;
                }
                if node.height != h {
                    node.height = h;
                }
                if *vis != Visibility::Inherited {
                    *vis = Visibility::Inherited;
                }
            }
            if let Ok((mut node, mut vis, mut text)) = arrow.get_mut(child) {
                let (x, y) = match side {
                    ArrowSide::Below => (
                        rect.center().x - ARROW_SIZE * scale * 0.5,
                        rect.max.y + ARROW_GAP * scale,
                    ),
                    ArrowSide::Above => (
                        rect.center().x - ARROW_SIZE * scale * 0.5,
                        rect.min.y - ARROW_GAP * scale - ARROW_SIZE * scale,
                    ),
                };
                let (l, t) = (Val::Px(x / scale), Val::Px(y / scale));
                if node.left != l {
                    node.left = l;
                }
                if node.top != t {
                    node.top = t;
                }
                if *vis != Visibility::Inherited {
                    *vis = Visibility::Inherited;
                }
                // The glyph turns with the side: down when the arrow stands above its target, up when below.
                let want = side.glyph().to_string();
                if text.0 != want {
                    text.0 = want;
                }
            }
        }
    }
}

/// **The step the pointer has already complained about**, so a guide fault is said once rather than once a frame.
#[derive(Resource, Debug, Default)]
struct PointedAt(Option<String>);

/// The arrow's own box, in logical pixels — one glyph on a line.
const ARROW_SIZE: f32 = 16.0;

