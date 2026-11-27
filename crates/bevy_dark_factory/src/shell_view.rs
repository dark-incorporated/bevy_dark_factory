//! **Hide the left panel, or everything but the world** (feedback-1794146831022).
//!
//! The note, from the editor: *"We should have a toggle that toggles into "Zen" mode, that hides all the UI and just sees
//! the game. Also, Cmd+B should toggle the left panel, like VSCode."*
//!
//! - **`Cmd+B` shows and hides the left panel** (the INSPECTOR column and its pane): the side bar's chord in VS Code, and in
//!   the same family as `Cmd+J` for the terminal. The viewport takes the room.
//! - **`Shift+Cmd+M` is Zen mode**: the menu bar, the chrome bar, the toolbar, the door's strip, both docks, the terminal, the
//!   status band and the empty-map prompt go, and the viewport is the whole window. The same chord brings them back, and a
//!   toast says so when the mode begins, since nothing on screen could.
//! - **View ▸ Show left panel** and **View ▸ Zen mode** are the same two, as ticks.
//!
//! # The left panel is the shell split's collapsed pane (UI-R06, #336)
//!
//! **Hiding the left panel collapses the shell's gooey split**, as Enter on its divider does, and showing it restores
//! it: the panes slide on gooey's spring ([`bevy_gooey::SplitGlide`]) instead of the viewport jumping a pane's width, and the
//! collapsed pane **peeks** — the pointer resting on the divider floats the pane over the viewport at the width a restore would
//! give it, with the viewport still (gooey's `SplitPeek`, #69). **The shell asks for it with gooey's [`CollapseSplit`]**
//! (G-77, #88), triggered on the split: the one path the divider's own Enter, South button and double press take, so
//! `collapse_the_left_pane` synthesises no key and reaches into nothing of gooey's.
//!
//! **Two numbers are the shell's to hold while it is collapsed.** The pane's floor ([`SplitMin::first`], which keeps a dragged
//! pane wide enough for a panel) is put to nothing so the pane can shut, and given back when a restore has landed; the
//! fraction a restore returns to is raised to that floor first, so the pane opens to the width the panel needs and the
//! glide has somewhere to go that the floor does not cut short.
//!
//! **A shut pane shows no dock.** Once the collapse has landed, nothing is peeking and the pane is as wide as nothing, the left
//! dock is `Display::None` ([`Folded`]): a node that lays out to nothing is how the badges and the census already tell a
//! hidden control from a shown one (`chrome::Control`), and a dock clipped to no width but still laid out would have its
//! badges drawn over the world and its widgets in the Tab order. A peek shows it again for as long as it floats.
//!
//! # How the rest goes
//!
//! **`Display::None`, and the frame is left as it is.** A hidden node lays out to nothing and the viewport node (which the world
//! is drawn through, `surface`) grows by flexbox into what the others leave. What each was before is kept on the node
//! ([`Folded`]) and put back, so a part that was hidden for another reason (the collapsed terminal) stays hidden after Zen.
//! **The view is enforced every frame it is on**: a part another system raises (the terminal's own chord) is folded again the
//! next frame. Zen also folds the split's first pane and its divider, so no sliver of either stands in a bare window.
//!
//! **Session state, not a setting.** A person who left Zen mode on would open the editor to a bare viewport and no menu; the
//! left panel's state is likewise the session's. Neither is written to the user's file.

use bevy::prelude::*;
use bevy_gooey::{CollapseSplit, Role, SplitDivider, SplitFraction, SplitGlide, SplitMin, SplitPeek, SplitSaved};

use crate::chrome::{BottomDock, Frame, FrameSystems, ShellSplit};
use crate::confirm::Confirm;
use crate::empty_state::EmptyMapPrompt;
use crate::keys::{Context, Holder, Live};
use crate::menu_bar::EditorMenuBar;
use crate::screen::Screen;
use crate::terminal_pane::TerminalPaneRoot;
use crate::toolbar::Toolbar;

/// What the person has hidden of the shell this session.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShellView {
    /// The left panel (and its pane) is hidden.
    pub left_hidden: bool,
    /// Zen mode: everything but the viewport is hidden.
    pub zen: bool,
    /// **The bottom dock is asked for** (View ▸ Show reload panel): shown whatever the room, which is
    /// what a person on a short window with the terminal open (the default) has to do to read the reload state.
    pub reload_shown: bool,
}

impl ShellView {
    /// Is the left panel's pane out of the layout? Zen takes it with everything else.
    pub fn left_is_hidden(self) -> bool {
        self.left_hidden || self.zen
    }

    /// Are the bands, the toolbar, the right dock and the terminal out of the layout?
    pub fn the_rest_is_hidden(self) -> bool {
        self.zen
    }
}

/// **What a node's `display` was before the shell view folded it**, put back when the view lets go of it.
#[derive(Component, Debug)]
pub struct Folded(pub Display);

/// The View menu's tick for the left panel.
#[derive(Component)]
pub struct LeftPanelItem;

/// The View menu's tick for Zen mode.
#[derive(Component)]
pub struct ZenItem;

/// The View menu's tick for the reload panel: the bottom dock, asked for.
#[derive(Component)]
pub struct ReloadPanelItem;

/// Everything Zen mode folds that has no field in the [`Frame`]: found by what it is.
type ZenParts = Or<(With<EditorMenuBar>, With<Toolbar>, With<TerminalPaneRoot>, With<EmptyMapPrompt>)>;

pub struct ShellViewPlugin;

impl Plugin for ShellViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ShellView>()
            .add_observer(reopen_the_folded_dock)
            .add_observer(the_art_share_flips_the_view)
            .add_systems(
                Update,
                (
                    toggle_on_the_chords,
                    collapse_the_left_pane.after(FrameSystems),
                    give_the_bottom_dock_its_room.after(FrameSystems),
                    fold_the_shell.after(FrameSystems),
                    float_the_art_share,
                    paint_the_art_share,
                    tick_the_menu_rows,
                )
                    .chain()
                    .run_if(in_state(Screen::Editor)),
            );
    }
}

/// **The two chords.** Not while a text field has the keys (a chord typed into a field is the field's), nor over a question.
///
/// **A terminal's hold is not a field's** (#667). With the pane raised at a quarter, `Ctrl+B` and
/// `Shift+Ctrl+M` reached nothing: `TerminalFocus` folded the shell into the same `Context::Typing` a
/// name being typed gives, so both chords were refused and the editor's own answer to "give the art
/// the window" did nothing in the one state a person would reach for it. The two are on
/// [`crate::chords::SKIPPED_BY_THE_SHELL`], the skip list every terminal has: the window reads them
/// on every platform, `Ctrl` or `Cmd` alike, exactly as VS Code's terminal lets `Ctrl+B` through to
/// fold its side bar. Every other chord stays the shell's.
///
/// **Zen mode gives the terminal's keyboard back**, or the keys that move the camera would go on
/// reaching a shell nobody can see.
fn toggle_on_the_chords(
    keys: Res<ButtonInput<KeyCode>>,
    live: Res<Live>,
    confirm: Res<Confirm>,
    bindings: Res<bevy_gooey::BindingSettings>,
    mut focus: ResMut<crate::terminal_pane::TerminalFocus>,
    mut view: ResMut<ShellView>,
    mut commands: Commands,
) {
    // **The terminal's own hold is not a refusal** (#667); a field's is, and `Context::Typing` is
    // both. `Holder::Terminal` is the difference, so the gate asks the holder rather than the folded
    // context.
    let field_has_the_keys = live.0 == Context::Typing && live.2 != Holder::Terminal;
    let left_chord = crate::chords::chord(crate::chords::Fixed::ShellLeftPanel);
    let zen_chord = crate::chords::chord(crate::chords::Fixed::ShellZen);
    let left = left_chord.just_pressed(&keys);
    let zen = zen_chord.just_pressed(&keys);
    if !(left || zen) || field_has_the_keys || confirm.is_open() {
        return;
    }
    if left {
        view.left_hidden = !view.left_hidden;
    }
    if zen {
        look_or_work(&mut view, &mut focus, &bindings, &mut commands);
    }
}

/// **Look, or back to Work** (#669): Zen mode begun or ended, as `shell.zen`'s chord and the status band's
/// `Art` control both ask — one function, so the two doors cannot disagree about what a press does.
///
/// **Look is Zen** (the owner, 2026-11-27: *"two states, Work and Look … Both map onto `ShellView { left_hidden,
/// zen }`"*): both side docks, the chrome bar, the toolbar, the panel tab row and the status band go, and the art
/// gets the window. **Work is the layout the person had**, so `left_hidden` is not touched and a second press
/// gives back exactly what the first took. Look gives the terminal's keyboard back, or the keys that move the
/// camera would go on reaching a shell nobody can see.
fn look_or_work(view: &mut ShellView, focus: &mut crate::terminal_pane::TerminalFocus, bindings: &bevy_gooey::BindingSettings, commands: &mut Commands) {
    view.zen = !view.zen;
    if view.zen {
        focus.release();
        let zen_chord = crate::chords::chord(crate::chords::Fixed::ShellZen);
        commands.spawn(bevy_gooey::toast(format!("Zen mode: {} brings the editor back", bindings.chord(zen_chord))));
    }
}

/// Where the shell's collapse of the left pane stands. The float is the pane's floor, [`SplitMin::first`], as the shell
/// authored it.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
enum Collapse {
    /// The pane is open, or collapsed by someone else.
    #[default]
    Open,
    /// The shell pressed Enter on the divider to collapse, and gooey has not answered yet.
    Asking(f32),
    /// The shell's collapse stands (or is sliding in); the floor is given back at a restore.
    Held(f32),
    /// A restore is under way; the floor is given back when its glide has landed.
    Giving(f32),
}

/// **Ask the shell split to collapse its first pane, or give it back**: gooey's [`CollapseSplit`], the one path the divider's
/// Enter, South button and double press take too, so the slide, the saved fraction, the peek and the one
/// `ValueChange<f32>` are the same whoever asks. Asking for the state the split is in does nothing.
fn ask_the_split(commands: &mut Commands, split: Entity, collapsed: bool) {
    commands.trigger(CollapseSplit { entity: split, collapsed });
}

/// What the shell reads of its split to collapse it.
type ShellSplitRead = (
    Entity,
    &'static mut SplitFraction,
    &'static mut SplitMin,
    &'static ComputedNode,
    &'static Children,
    Has<SplitSaved>,
    Has<SplitGlide>,
);

/// **Collapse the left pane while the view says it is hidden, restore it when it is not** — see the module's header.
///
/// One state per frame: [`Collapse`]. A person who docks the peek, or drags the divider, while the view says hidden has
/// opened the pane by hand, and the view follows them (the tick and the chord agree) rather than collapsing it again.
/// **A split that is not the one it last looked at starts over** (the frame is spawned on each door, with its own pane open
/// and its own floor): the view is session state and stays, so the new split is collapsed to match it, not mistaken for one
/// a person has opened.
#[allow(clippy::too_many_arguments)]
fn collapse_the_left_pane(
    mut view: ResMut<ShellView>,
    workspace: Option<Res<crate::workspace::Workspace>>,
    mut splits: Query<ShellSplitRead, With<ShellSplit>>,
    roles: Query<&Role>,
    dividers: Query<&ComputedNode, With<SplitDivider>>,
    mut state: Local<Collapse>,
    mut looked_at: Local<Option<Entity>>,
    mut commands: Commands,
) {
    let Ok((split, mut fraction, mut min, node, kids, collapsed, gliding)) = splits.single_mut() else { return };
    if *looked_at != Some(split) {
        *looked_at = Some(split);
        *state = Collapse::Open;
    }
    let Some(divider) = kids.iter().find(|kid| matches!(roles.get(*kid), Ok(Role::SplitDivider))) else { return };
    // **Not where the panel takes the centre** (#694): there the first pane is the body itself, and hiding it would leave
    // nothing, so `Cmd+B` waits for a workspace with a viewport to give the room to.
    let want = view.left_is_hidden() && !workspace.is_some_and(|w| w.panel_takes_the_centre());
    match *state {
        Collapse::Open if want => {
            // Laid out, or there is no room to answer for: gooey's collapse reads the same room and does nothing without it.
            let bar = dividers.get(divider).map_or(0.0, |d| d.size().x * d.inverse_scale_factor());
            let extent = node.size().x * node.inverse_scale_factor() - bar;
            if extent <= 0.0 || collapsed {
                return;
            }
            let floor = min.first;
            // What a restore gives back is never under the floor, so the pane opens to the width its panel needs.
            let at_least = (floor / extent).clamp(0.0, 1.0);
            if fraction.0 < at_least {
                fraction.0 = at_least;
            }
            min.first = 0.0;
            ask_the_split(&mut commands, split, true);
            *state = Collapse::Asking(floor);
        }
        Collapse::Open => {}
        Collapse::Asking(floor) => {
            if collapsed {
                *state = Collapse::Held(floor);
            } else {
                // gooey did not take it (no room yet): the floor goes back and the next frame asks again.
                min.first = floor;
                *state = Collapse::Open;
            }
        }
        Collapse::Held(floor) => {
            if collapsed && !want {
                ask_the_split(&mut commands, split, false);
                *state = Collapse::Giving(floor);
            } else if !collapsed && !gliding {
                // Docked by the person: the pane is open and the view says so.
                if want && !view.zen {
                    view.left_hidden = false;
                }
                *state = Collapse::Giving(floor);
            }
        }
        Collapse::Giving(floor) => {
            if collapsed && want {
                // Hidden again before it opened: collapse from where it is drawn.
                ask_the_split(&mut commands, split, true);
                *state = Collapse::Asking(floor);
            } else if !collapsed && !gliding {
                min.first = floor;
                *state = Collapse::Open;
            }
        }
    }
}

/// **The bottom dock: asked for, or given the room** (UI-61, #247). The one writer of its `display`.
///
/// A 148-px row under the body is taken from the body, and at the editor's reference window (1280 x
/// 720, terminal open) that left the inspector 24 px of visible height and the Kit tabs' badge legends
/// under the dock. So, unless the person has asked for it (View ▸ Show reload panel, [`ShellView::reload_shown`]),
/// the dock is a row only when the workbench keeps the sheet's `workbench_floor_h` beside it: `Display::None`
/// otherwise, which lays out to nothing and leaves no text for the census to find drawn outside a
/// clip. **The room is the body plus the dock's own row, whether it is shown or not**, so showing it
/// does not change the answer the next frame (no flicker at the threshold).
///
/// **And less the workspace strip's row where the strip is not a row** (#670). A workspace with one
/// panel folds its tab into the chrome bar and the body has the 35 px ([`crate::tiles::TAB_STRIP_H`])
/// its row took; the room is read as if the row were there, so the window at which the dock comes is
/// the one it was on every workspace (1280 x 720 still has none) and the body keeps the row's height.
///
/// **A dock with nothing to report folds to its own tab bar** (#662). A project nobody has connected a
/// game to draws one dim line, and a 148-px strip for that line took 17.8% of a 1600 x 1000 window from
/// the art. When [`crate::editor::ReloadNews`] says the panel has no news, the strip's height is
/// [`LayoutMetrics::bottom_dock_tab_h`] and its pages are out of the layout — the tab bar stays drawn,
/// so the strip is still there to be opened, and the room it gives back is the body's. The
/// news is `refresh_reload_list`'s answer, which is the same pass that builds the lines, so the strip
/// and its content cannot disagree about whether there is anything to say.
///
/// **Zen mode hides it, and the frame Zen ends on does not read the room.** [`fold_the_shell`] does not
/// touch the dock, so there is no stored display to put back (that put back a stale `Flex` for one frame
/// when the window had shrunk in Zen); and on that frame the layout is still Zen's, whose body is the
/// whole window, so a dock nobody asked for stays hidden until the next frame has measured the real one.
#[allow(clippy::too_many_arguments)]
fn give_the_bottom_dock_its_room(
    view: Res<ShellView>,
    news: Option<Res<crate::editor::ReloadNews>>,
    frame: Option<Res<Frame>>,
    laid: Query<&ComputedNode>,
    mut docks: Query<&mut Node, With<BottomDock>>,
    mut sizing: Query<&mut bevy_gooey::Sizing>,
    roles: Query<(&Role, &ChildOf)>,
    // **The reload panel's page, by its own marker** (`ui::register_the_shell_panels` puts it there): the
    // one node this fold may take out of the layout.
    panels: Query<(Entity, &ChildOf), With<crate::editor::ReloadPanel>>,
    mut pages: Query<&mut Node, Without<BottomDock>>,
    mut zen_last_frame: Local<bool>,
) {
    let Some(frame) = frame else { return };
    let Ok(mut dock) = docks.get_mut(frame.bottom) else { return };
    let just_left_zen = std::mem::replace(&mut *zen_last_frame, view.the_rest_is_hidden()) && !view.the_rest_is_hidden();
    let layout = crate::style::metrics().layout;
    // **The panel has news, or the person asked for it.**
    let reports = news.as_deref().is_none_or(crate::editor::ReloadNews::has_news) || view.reload_shown;
    let full = layout.bottom_dock_h;
    let folded = layout.bottom_dock_tab_h;
    let display = if view.the_rest_is_hidden() {
        Display::None
    } else if view.reload_shown {
        Display::Flex
    } else if just_left_zen {
        Display::None
    } else {
        // **One floor, whatever the strip's height**: the room rule is a fact about the window and the
        // workbench, not about what the dock is showing. `the_bottom_dock_gives_way_in_the_reference_
        // window` pins that at 1280x720 there is no dock row at all — folded or not — and three
        // assertions in `the_reload_panel_is_a_gooey_dock_panel` pin the same floor from three sides.
        // A 29-px tab bar that slipped in under it would be a row those tests say is not there.
        let Ok(body) = laid.get(frame.body) else { return };
        let interface_px = |node: &ComputedNode| node.size().y * node.inverse_scale_factor();
        let own_row = if dock.display == Display::None { 0.0 } else { laid.get(frame.bottom).map_or(0.0, interface_px) };
        let folded_row = (crate::tiles::TAB_STRIP_H - laid.get(frame.door_strip).map_or(0.0, interface_px)).max(0.0);
        if interface_px(body) + own_row - folded_row >= layout.workbench_floor_h + layout.bottom_dock_h { Display::Flex } else { Display::None }
    };
    if dock.display != display {
        dock.display = display;
    }
    // **The height, and the page it leaves no room for** (#662). The dock's `Sizing` is the one writer
    // of its height, and gooey follows it.
    //
    // **The page is hidden through its own panel marker, not the dock's `TabPages`**: a bottom dock
    // holding a second panel (`a_dock_tab_reordered_glides_to_its_place` lays a `STATUS` one beside
    // `RELOAD`) has one `TabPages` node for both, and hiding that would lay both tabs' pages out to
    // nothing and put the two tabs at one point. The reload panel is what has nothing to report, so it
    // is what goes; another panel in the same strip is drawn as it always was.
    let want_h = if display == Display::Flex && !reports { folded } else { full };
    if let Ok(mut size) = sizing.get_mut(frame.bottom)
        && size.height != Some(bevy_gooey::Extent::Px(want_h))
    {
        size.height = Some(bevy_gooey::Extent::Px(want_h));
    }
    let hide_the_page = display == Display::Flex && !reports && only_the_reload_panel(&roles, frame.bottom);
    for (page, child_of) in &panels {
        if !under(child_of.parent(), frame.bottom, &roles) {
            continue;
        }
        let want = if hide_the_page { Display::None } else { Display::Flex };
        if let Ok(mut node) = pages.get_mut(page)
            && node.display != want
        {
            node.display = want;
        }
    }
}

/// **Does the bottom dock hold the reload panel and nothing else?** — the condition the fold needs
/// before it may take the panel's page out of the layout. A dock with a second panel in its group is
/// somebody else's strip, and its height is not this fold's to spend.
fn only_the_reload_panel(roles: &Query<(&Role, &ChildOf)>, dock: Entity) -> bool {
    let mut pages = 0usize;
    for (role, child_of) in roles.iter() {
        if *role == Role::TabPages && under(child_of.parent(), dock, roles) {
            pages += 1;
        }
    }
    // **One `TabPages` and one `TabList`, and the tabs are the reload panel's alone.** Counted from the
    // tabs rather than from the registry, because what a person sees is the tabs.
    let tabs = roles.iter().filter(|(role, child_of)| **role == Role::Tab && under(child_of.parent(), dock, roles)).count();
    pages == 1 && tabs <= 1
}

/// Is `entity` the bottom dock, or inside it? Walked upward so a change to what gooey puts between
/// them cannot break the fold.
fn under(entity: Entity, root: Entity, parents: &Query<(&Role, &ChildOf)>) -> bool {
    let mut at = Some(entity);
    while let Some(entity) = at {
        if entity == root {
            return true;
        }
        at = parents.get(entity).ok().map(|(_, child_of)| child_of.parent());
    }
    false
}

/// **A click on the dock's tab opens the strip** (#662): the tab bar is what a folded dock keeps, so it
/// is the door back to the full panel. Clicking a tab is a person asking for that panel, which is what
/// `reload_shown` means — so the View menu's tick and the click say the same thing.
///
/// The bottom dock holds one panel, so any click in its tab strip is the RELOAD tab's. A click in the
/// panel's own body is not this observer's: the strip is folded then, and there is no body to click.
fn reopen_the_folded_dock(
    mut click: On<bevy::picking::events::PointerClick>,
    frame: Option<Res<Frame>>,
    roles: Query<(&Role, &ChildOf)>,
    mut view: ResMut<ShellView>,
) {
    let Some(frame) = frame else { return };
    if view.reload_shown || !under(click.entity, frame.bottom, &roles) {
        return;
    }
    // **The tab strip, not the page**: a click that landed inside a panel's body is the panel's own
    // control, and folding never leaves one laid out.
    let in_the_strip = roles.get(click.entity).is_ok_and(|(role, _)| *role == Role::TabList)
        || roles
            .get(click.entity)
            .is_ok_and(|(_, child_of)| roles.get(child_of.parent()).is_ok_and(|(role, _)| *role == Role::TabList));
    if in_the_strip {
        click.propagate(false);
        view.reload_shown = true;
    }
}

/// **Fold what the view hides, and unfold what it has let go of.** Every frame the view is on, the same answer, so a part
/// another system raised is folded again; a settled shell costs one comparison per part.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn fold_the_shell(
    view: Res<ShellView>,
    workspace: Option<Res<crate::workspace::Workspace>>,
    frame: Option<Res<Frame>>,
    parts: Query<Entity, ZenParts>,
    splits: Query<(&Children, Has<SplitSaved>, Has<SplitGlide>, &SplitPeek), With<ShellSplit>>,
    roles: Query<&Role>,
    folded: Query<(Entity, &Folded)>,
    mut nodes: Query<&mut Node>,
    mut commands: Commands,
) {
    let Some(frame) = frame else { return };
    let centred = workspace.is_some_and(|w| w.panel_takes_the_centre());
    let mut fold: Vec<Entity> = Vec::new();
    if view.the_rest_is_hidden() {
        fold.extend(parts.iter());
        fold.extend([frame.chrome_bar, frame.door_strip, frame.status, frame.right]);
    }
    for (kids, collapsed, gliding, peek) in &splits {
        // The children come `[pane, divider, pane]`, and the second pane holds the viewport.
        let own = |kid: &Entity| roles.get(*kid).is_ok_and(|role| matches!(role, Role::SplitPane | Role::SplitDivider));
        if centred {
            // **A panel that takes the centre (#694)**: the divider and the viewport's pane go, and the first pane is the
            // body. Zen leaves that pane, because on this workspace the panel is what there is to see.
            fold.extend(kids.iter().filter(own).skip(1));
        } else if view.zen {
            // **The first pane and the divider only**.
            fold.extend(kids.iter().filter(own).take(2));
        }
        // **A shut pane shows no dock**: collapsed, landed, and nothing floating over the viewport.
        if view.left_is_hidden() && collapsed && !gliding && !peek.is_open() {
            fold.push(frame.left);
        }
    }

    for entity in &fold {
        let Ok(mut node) = nodes.get_mut(*entity) else { continue };
        if node.display != Display::None {
            if !folded.contains(*entity) {
                commands.entity(*entity).insert(Folded(node.display));
            }
            node.display = Display::None;
        }
    }
    for (entity, before) in &folded {
        if fold.contains(&entity) {
            continue;
        }
        if let Ok(mut node) = nodes.get_mut(entity) {
            node.display = before.0;
        }
        commands.entity(entity).remove::<Folded>();
    }
}

/// **The ticks say what the view is**, whichever way it changed: a chord, or the row.
fn tick_the_menu_rows(
    view: Res<ShellView>,
    left: Query<(Entity, Has<bevy_gooey::Checked>), With<LeftPanelItem>>,
    zen: Query<(Entity, Has<bevy_gooey::Checked>), With<ZenItem>>,
    reload: Query<(Entity, Has<bevy_gooey::Checked>), With<ReloadPanelItem>>,
    mut commands: Commands,
) {
    let mut tick = |item: Entity, ticked: bool, want: bool| match (want, ticked) {
        (true, false) => {
            commands.entity(item).insert(bevy_gooey::Checked);
        }
        (false, true) => {
            commands.entity(item).remove::<bevy_gooey::Checked>();
        }
        _ => {}
    };
    for (item, ticked) in &left {
        tick(item, ticked, !view.left_hidden);
    }
    for (item, ticked) in &zen {
        tick(item, ticked, view.zen);
    }
    for (item, ticked) in &reload {
        tick(item, ticked, view.reload_shown);
    }
}

/// **View ▸ Show left panel**: the tick is the panel shown.
pub fn toggle_the_left_panel(change: On<bevy_gooey::ValueChange<bool>>, mut view: ResMut<ShellView>) {
    view.left_hidden = !change.value;
}

/// **View ▸ Show reload panel**: the tick is the bottom dock asked for.
pub fn toggle_the_reload_panel(change: On<bevy_gooey::ValueChange<bool>>, mut view: ResMut<ShellView>) {
    view.reload_shown = change.value;
}

/// **View ▸ Zen mode.**
pub fn toggle_zen(change: On<bevy_gooey::ValueChange<bool>>, mut view: ResMut<ShellView>) {
    view.zen = change.value;
}

// ── the status band's `Art` control (#669) ─────────────────────────────────────────────────────

/// **The status band's `Art 32%` control** (#669): its label is the art's share of the window, and a click
/// is [`look_or_work`] — the same press as `shell.zen`'s chord, so `blueprint/run_command { id: "shell.zen" }`
/// reaches it.
///
/// The owner's decision (2026-11-27): *"two states, Work and Look. A status-band control labelled with the game
/// art's share of the window (e.g. `Art 32%`); click cycles Work (today's layout) ↔ Look (both side docks folded
/// and the chrome bar, toolbar, panel tab row and status band hidden except this control, so the art gets the
/// window)."* The label is the measurement, so a person can see what a press bought; it is
/// [`crate::ops_brp::measure_rects`] and [`crate::ops_brp::art_share`], the functions `blueprint/state`'s
/// `rects.art_share` is made by, so the band and the state report one number.
///
/// **In Look the band is folded with the rest, and the control is not**: [`float_the_art_share`] lifts it out of
/// the band onto the window's own root, floating at the bottom-right corner over the art, and puts it back where
/// it stood when Work returns. It takes no room in either place but the band's, so Look's art is the whole window
/// and the way back is still on screen.
#[derive(Component)]
pub struct ArtShare;

/// **What the control says**: `Art` and the share as a whole percent, `Art 32%`; `Art` alone before the first
/// layout has measured anything, never a made-up `0%`.
pub fn art_share_words(share: Option<f32>) -> String {
    share.map_or_else(|| "Art".to_owned(), |share| format!("Art {:.0}%", share * 100.0))
}

/// **What the next press does**, for the control's tooltip and its accessible name.
fn next_press_words(view: ShellView) -> &'static str {
    if view.the_rest_is_hidden() { "Work: give the editor back" } else { "Look: give the art the window" }
}

/// **The control, spawned at the status band's end** by `chrome::spawn_status_band`, after the hint: the band is
/// where the editor says what the whole shell is doing, and the hint's key badge settles to the hint's left, so
/// nothing is drawn over it. A chip, as the band's `Terminal` label is: one click is its gesture.
pub fn spawn_the_art_share(band: &mut ChildSpawnerCommands) {
    let ink = crate::chrome::ink();
    crate::chrome::chip(band, ArtShare, &art_share_words(None), crate::chrome::text::CONTROL, ink.text, ink.bar_bg, ink.panel_edge)
        .insert((Name::new("art share"), bevy_gooey::Tooltip::new(next_press_words(ShellView::default()))));
}

/// **The label follows the measurement, and the hover and the accessible name follow the view.** Measured only
/// when the viewport or the window was laid out anew, the view changed or the control is new, so a settled shell
/// costs two change checks; written only on a change.
#[allow(clippy::type_complexity)]
fn paint_the_art_share(
    view: Res<ShellView>,
    frame: Option<Res<Frame>>,
    laid: Query<Ref<ComputedNode>>,
    rects: Query<crate::ops_brp::RectNode>,
    mut controls: Query<(Entity, Ref<ArtShare>, &Children, &mut bevy_gooey::Tooltip)>,
    mut texts: Query<&mut Text>,
    mut commands: Commands,
) {
    let Some(frame) = frame else { return };
    let moved = [frame.viewport, frame.root].into_iter().any(|node| laid.get(node).is_ok_and(|node| node.is_changed()));
    let fresh = controls.iter().any(|(_, control, _, _)| control.is_added());
    if !(moved || fresh || view.is_changed()) {
        return;
    }
    let (viewport, window) = crate::ops_brp::measure_rects(rects.iter());
    let words = art_share_words(crate::ops_brp::art_share(viewport, window));
    let next = next_press_words(*view);
    let tip = format!("{next} — {}", crate::terminal_pane::chord_words(&crate::chords::chord(crate::chords::Fixed::ShellZen)));
    for (control, _, kids, mut tooltip) in &mut controls {
        let Some(words_at) = kids.iter().find(|kid| texts.contains(*kid)) else { continue };
        let Ok(mut text) = texts.get_mut(words_at) else { continue };
        let (reworded, renamed) = (text.0 != words, tooltip.text != tip);
        if reworded {
            text.0.clone_from(&words);
        }
        if renamed {
            tooltip.text.clone_from(&tip);
        }
        if reworded || renamed {
            commands.entity(control).insert(AccessibleLabel::new(format!("{words} of the window — {next}")));
        }
    }
}

/// **A click on the control is Look, or Work** ([`look_or_work`]), as the chord is.
fn the_art_share_flips_the_view(
    mut click: On<bevy::picking::events::PointerClick>,
    controls: Query<(), With<ArtShare>>,
    mut view: ResMut<ShellView>,
    mut focus: ResMut<crate::terminal_pane::TerminalFocus>,
    bindings: Res<bevy_gooey::BindingSettings>,
    mut commands: Commands,
) {
    if !controls.contains(click.entity) {
        return;
    }
    click.propagate(false);
    look_or_work(&mut view, &mut focus, &bindings, &mut commands);
}

/// **In Look the control floats on the window's root, in Work it stands in the band where it was** (#669).
///
/// Look folds the status band with the rest ([`fold_the_shell`]), and a folded node's children are not drawn, so
/// the control is moved, not left behind: onto [`Frame::root`], `Absolute` at the bottom-right corner a step in,
/// which takes no room from the art. Work puts it back at the place in the band it was taken from, so the band
/// and the rects are what they were before the first press.
fn float_the_art_share(
    view: Res<ShellView>,
    frame: Option<Res<Frame>>,
    mut controls: Query<(Entity, &ChildOf, &mut Node), With<ArtShare>>,
    children: Query<&Children>,
    mut home: Local<usize>,
    mut commands: Commands,
) {
    let Some(frame) = frame else { return };
    let look = view.the_rest_is_hidden();
    let inset = Val::Px(crate::chrome::space(crate::chrome::STEP_PAD));
    let in_band = |control: Entity| children.get(frame.status).ok().and_then(|kids| kids.iter().position(|kid| kid == control));
    for (control, parent, mut node) in &mut controls {
        let floating = parent.parent() == frame.root;
        if look && !floating {
            if let Some(at) = in_band(control) {
                *home = at;
            }
            commands.entity(frame.root).add_child(control);
            // PLACES-ITSELF-OK: not a panel; the one control left in Look (#669) floats in the window's corner so the way back to Work stays reachable while the band it lives in is hidden.
            node.position_type = PositionType::Absolute;
            node.right = inset;
            node.bottom = inset;
        } else if !look && floating {
            let at = (*home).min(children.get(frame.status).map_or(0, |kids| kids.len()));
            commands.entity(frame.status).insert_children(at, &[control]);
            node.position_type = PositionType::Relative;
            node.right = Val::Auto;
            node.bottom = Val::Auto;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Zen takes the left panel with it, and the left panel alone takes nothing else.**
    #[test]
    fn the_view_says_what_is_hidden() {
        for left_hidden in [false, true] {
            for zen in [false, true] {
                let view = ShellView { left_hidden, zen, ..ShellView::default() };
                assert_eq!(view.left_is_hidden(), left_hidden || zen, "{view:?}");
                assert_eq!(view.the_rest_is_hidden(), zen, "{view:?}");
            }
        }
    }

    /// **The two chords are different keys and neither is the other's with a modifier added**: a press of one never reads as the
    /// other (`Shortcut::just_pressed` asks for exactly its modifiers).
    #[test]
    fn the_chords_do_not_overlap() {
        use crate::chords::{Fixed, default_of};
        assert_ne!(default_of(Fixed::ShellLeftPanel), default_of(Fixed::ShellZen));
        assert_ne!(default_of(Fixed::ShellLeftPanel).key, default_of(Fixed::ShellZen).key);
    }
}
