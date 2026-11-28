//! **The toolbar** (UI-37, #231): the tool modes, undo and redo, save, and the view and capture
//! helpers, as a row of gooey icon buttons above the docks.
//!
//! **One body per verb, and it is the key's.** Every button carries the id of a registered command
//! (`crate::commands`) and, when it is clicked, hands that id to [`crate::command_run::Chords`] — the
//! same queue the command palette uses. The chord goes down a frame or two later and the editor's own
//! key handler judges it, exactly as if a person had pressed the key. So **a button cannot arm a tool
//! one way and its key another**: there is only one arming path, and it is the keyboard's.
//!
//! That is also why the toolbar holds no state of its own. Which button looks "on" is read from
//! `EditorState::tool` every frame, and a click changes nothing directly; the key press changes the
//! tool, and the buttons follow. The alternative — the button setting `Tool::Move` itself — is the
//! second code path `arm_tool` was written to delete.
//!
//! **Fill is not a tool.** `Tool` has six variants and no `Fill`; `Action::Fill` is a one-shot flood
//! from the cursor, not a mode. A Fill *toggle* would invent a mode to match a checklist line, so the
//! button runs the command `F` runs and refuses when the pointer is not over the map — see
//! `tests/a_tool_arms_the_same_from_a_button_and_a_key.rs`, which measured this.
//!
//! **The bar is the workspace's** (#690, the owner's "per workspace", 2026-11-27). It is drawn on the
//! workspaces that have the map's tools — Map, World and Kit, [`Workspace::has_the_toolbar`] — and on no
//! other, so a workspace without them has no bar and its panels have the 46 px it took. **A control is
//! enabled exactly where its command acts**, read from the binding its key is judged by: World and Kit draw
//! the bar, but the modes, Fill, undo and redo are the Map tab's, so there they are disabled and drawn unlit,
//! and only Save and the snip (live on every tab) can be pressed. Undo, redo and save stay on the bar rather
//! than moving: the menus and the keys reach them on every workspace, as before.
//!
//! The bar's own look — height, gaps, the icon's tint and size — is `bevy_gooey`'s, from the running
//! `GooeySkin`. This module picks *which* commands it shows and marks the armed tool; it draws no
//! colour and sizes no glyph.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, InteractionDisabled, UiGlobalTransform};

use crate::commands::{self, Command};
use crate::editor::{EditorState, Tool};
use crate::icons;
use crate::tiles::Mode;
use crate::workspace::Workspace;

/// The id on a toolbar button, so a click can be resolved back to the command it stands for and a test
/// can find the armed tool's button by name rather than by position.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToolbarCommand(pub &'static str);

/// **The bar itself**: a band of its own straight after the chrome bar, under it and above the docks, as
/// `docs/ui/vision.md` §4 lays the workbench out — on the workspaces that have the map's tools, and no other.
#[derive(Component)]
pub struct Toolbar;

pub struct ToolbarPlugin;

impl Plugin for ToolbarPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<crate::command_run::CommandRunPlugin>() {
            app.add_plugins(crate::command_run::CommandRunPlugin);
        }
        app.add_systems(OnEnter(crate::screen::Screen::Editor), decode_the_sheet)
        .add_systems(
            Update,
            (
                // **Before gooey styles**, so the frame a control is enabled or disabled is the frame it is drawn
                // so, and a bar spawned on this frame has its group's value before its option is lit.
                (the_bar_follows_the_workspace, quiet_what_cannot_act_here, the_group_follows_the_armed_tool)
                    .chain()
                    .after(crate::chrome::FrameSystems)
                    .before(bevy_gooey::GooeySystems::Style),
                mark_the_armed_tool,
                label_the_primary_group,
            )
                .run_if(in_state(crate::screen::Screen::Editor)),
        )
        // After gooey has styled a chip, so the place it was given is the one this replaces.
        .add_systems(
            Update,
            stagger_the_chips.after(bevy_gooey::GooeySystems::Style).run_if(in_state(crate::screen::Screen::Editor)),
        )
        // **The one dispatch.** gooey's `Activate` is a click, `Enter`, `Space` or a pad's south
        // button, so every way a person meets a button ends in the same observer.
        .add_observer(on_toolbar_activate)
        // **And one for a toggle group's choice.** `bevy_gooey` sends `ValueChange<usize>` from the
        // group, which is the index of the option chosen — not the command's id, so this maps it back
        // through [`tools`] rather than trusting the order in two places.
        .add_observer(on_tool_mode_chosen);
    }
}

/// How many rows the chips of the bar stand in: two, because a chord is wider than the button it names
/// but narrower than two of them, so alternate buttons' chips can share a row.
const CHIP_ROWS: usize = 2;

/// The air between two chips, in logical px: a chip's own border is one device pixel, and two that touch
/// read as one.
const CHIP_GAP_PX: f32 = 1.0;

/// **Each button's shortcut chip stands over its own button, and none stands on another's** (#291).
///
/// Holding `K` shows a chord on every toolbar button (gooey's `ShortcutBadges`), drawn at the button's
/// right edge. A chord is wider than the 33 px button (`Ctrl+Shift+Z`, `Shift+B`), so every chip drew
/// over its neighbour's and the row read `Xift+B Ctrl+B…`. gooey places a chip from the skin and has no
/// idea its neighbour has one, so the bar's own host stands them: **centred on the button, in one of two
/// rows inside the bar**, a chip going to the row where it needs the least shove right of its centre.
/// Two rows cost no ground — the bar is tall enough for both, and nothing else stands on it — where a
/// row *below* the bar would stand on the door strip and its own chips.
///
/// Gooey writes a chip's place once, when it is styled, so this writes it again only when it differs:
/// a chip already where it belongs costs one comparison, and the system does nothing at all while no
/// chip stands (the query is empty).
fn stagger_the_chips(
    bars: Query<(&ComputedNode, &UiGlobalTransform), With<Toolbar>>,
    mut chips: Query<(Entity, &ChildOf, &ComputedNode, &mut Node), With<bevy_gooey::ShortcutBadge>>,
    buttons: Query<(&ComputedNode, &UiGlobalTransform, &ChildOf), Without<bevy_gooey::ShortcutBadge>>,
    parents: Query<&ChildOf>,
    toolbar: Query<(), With<Toolbar>>,
) {
    let Ok((bar, bar_at)) = bars.single() else {
        return;
    };
    let bar_rect = logical_rect(bar, bar_at);
    // The chips that are the bar's, with where their buttons are: a chip is a child of its button, and a
    // button is the bar's child, or its mode group's.
    let mut mine: Vec<(Entity, Rect, Vec2)> = Vec::new();
    for (chip, owner, node, _) in &chips {
        let Ok((button, button_at, _)) = buttons.get(owner.parent()) else {
            continue;
        };
        let on_the_bar = std::iter::successors(Some(owner.parent()), |e| parents.get(*e).ok().map(|p| p.parent()))
            .take(3)
            .any(|e| toolbar.contains(e));
        if !on_the_bar || node.size() == Vec2::ZERO {
            continue;
        }
        let size = node.size() * node.inverse_scale_factor;
        mine.push((chip, logical_rect(button, button_at), size));
    }
    mine.sort_by(|a, b| a.1.center().x.total_cmp(&b.1.center().x));

    let mut row_right = [f32::MIN; CHIP_ROWS];
    for (chip, button, size) in mine {
        let want_left = button.center().x - size.x * 0.5;
        // The row where the chip lands nearest its button's centre, and the left edge it gets there.
        let (row, left) = (0..CHIP_ROWS)
            .map(|r| (r, want_left.max(row_right[r] + CHIP_GAP_PX)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap_or((0, want_left));
        row_right[row] = left + size.x;
        let rows_height = CHIP_ROWS as f32 * size.y + (CHIP_ROWS - 1) as f32 * CHIP_GAP_PX;
        let top = bar_rect.min.y + (bar_rect.height() - rows_height).max(0.0) * 0.5 + row as f32 * (size.y + CHIP_GAP_PX);
        let Ok((_, _, _, mut node)) = chips.get_mut(chip) else {
            continue;
        };
        let (want_x, want_y) = (Val::Px(left - button.min.x), Val::Px(top - button.min.y));
        if node.left != want_x {
            node.left = want_x;
        }
        if node.top != want_y {
            node.top = want_y;
        }
        if node.right != Val::Auto {
            node.right = Val::Auto;
        }
    }
}

/// A laid-out node's rectangle in logical px.
fn logical_rect(node: &ComputedNode, at: &UiGlobalTransform) -> Rect {
    Rect::from_center_size(at.translation * node.inverse_scale_factor, node.size() * node.inverse_scale_factor)
}

/// **The tool modes' gooey toggle group**, so one option of it is lit at a time and the light is the
/// tool that is armed.
#[derive(Component)]
pub struct ToolModes;

/// **The commands the toolbar shows, in order**, with the tool modes first because they are what a
/// person reaches for while placing: the list is [`icons::TOOLBAR`]'s, so a command on the bar always
/// has a glyph (that parity is `tests/every_toolbar_command_has_an_icon.rs`'s).
pub fn shown() -> Vec<Command> {
    icons::TOOLBAR.iter().filter_map(|id| commands::by_id(id)).collect()
}

/// **The icon sheet, decoded once**, so a bar with no project and no working directory still has its
/// glyphs: the PNG is embedded in the binary ([`icons::ATLAS_PNG`]). The activity bar and the panels' help
/// marks draw from it too, so it is decoded on every workspace, whether or not that workspace draws a bar.
#[derive(Resource)]
pub struct ToolbarAtlas {
    pub image: Handle<Image>,
    pub layout: Handle<TextureAtlasLayout>,
}

/// **Decode the sheet**, on the first frame the editor screen opens: the atlas is the session's
/// (`screen::OWNERSHIP`), so a later door finds it and decodes nothing.
pub fn decode_the_sheet(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
    atlas: Option<Res<ToolbarAtlas>>,
) {
    if atlas.is_some() {
        return;
    }
    let image = match Image::from_buffer(
        icons::ATLAS_PNG,
        bevy::image::ImageType::Extension("png"),
        bevy::image::CompressedImageFormats::NONE,
        true,
        bevy::image::ImageSampler::default(),
        bevy::asset::RenderAssetUsages::default(),
    ) {
        Ok(image) => images.add(image),
        // A sheet that will not decode is a build fault, not a runtime one: the file is in the binary.
        // Say so once rather than spawning eleven blank buttons with no explanation.
        Err(error) => {
            bevy::log::error!("the toolbar's icon sheet does not decode: {error}");
            return;
        }
    };
    let layout = layouts.add(icons::layout());
    commands.insert_resource(ToolbarAtlas { image, layout });
}

/// **The bar follows the workspace** (#690): one bar in the frame while the workspace has the map's tools
/// ([`Workspace::has_the_toolbar`]), and none while it has not, so the docks below take the height it gave up.
///
/// A system rather than `OnEnter`, because "the frame holds the wrong bar for this workspace" is the one condition
/// that covers a new screen and a respawned frame. A bar that is not this frame's is gone with the frame it was in.
///
/// **A switch keeps the bar** (#678, #541): `screen::close_the_door` lifts it off the frame it sweeps, and this puts it
/// back straight after the new frame's chrome bar in the switch's own frame, when the workspace it lands on has the
/// map's tools; one that has not sweeps it. Its buttons are the same on every workspace that draws it: which mode is
/// lit and which button is quiet follow the editor every frame ([`the_group_follows_the_armed_tool`],
/// [`quiet_what_cannot_act_here`]).
fn the_bar_follows_the_workspace(
    mut commands: Commands,
    frame: Option<Res<crate::chrome::Frame>>,
    workspace: Option<Res<Workspace>>,
    atlas: Option<Res<ToolbarAtlas>>,
    skin: Res<bevy_gooey::GooeySkin>,
    bars: Query<(Entity, Option<&ChildOf>), With<Toolbar>>,
) {
    let Some(frame) = frame else {
        return;
    };
    let wanted = workspace.is_some_and(|w| w.has_the_toolbar());
    let mut drawn = false;
    for (bar, parent) in &bars {
        if !wanted || drawn {
            commands.entity(bar).despawn();
            continue;
        }
        match parent.map(ChildOf::parent) {
            Some(parent) if parent == frame.root => drawn = true,
            Some(_) => {
                commands.entity(bar).despawn();
            }
            None => {
                put_after_the_chrome_bar(&mut commands, &frame, bar);
                drawn = true;
            }
        }
    }
    if wanted
        && !drawn
        && let Some(atlas) = atlas
    {
        spawn_the_bar(&mut commands, &frame, &atlas, skin.gap);
    }
}

/// **Spawn the bar** into `frame`: a gooey [`toolbar`](bevy_gooey::toolbar) inserted straight after the chrome
/// bar, so it is the window's shape and nothing a panel adds — the docks get what is left.
fn spawn_the_bar(commands: &mut Commands, frame: &crate::chrome::Frame, atlas: &ToolbarAtlas, gap: f32) {
    let (image, layout) = (atlas.image.clone(), atlas.layout.clone());
    // **The first button stands off the window's edge by a skin gap** (#290). gooey keeps a tooltip
    // `gap` clear of the window, and a box that cannot sit centred under a button at the bar's left end
    // slides along the button's edge; a button nearer the edge than that margin makes every below-the-
    // button place count as clipped, so the box was put *beside* the button, over its neighbours.
    let edge = gap;

    let bar = commands
        .spawn((
            Toolbar,
            bevy_gooey::toolbar(Children::spawn(SpawnWith(move |parent: &mut bevy::ecs::relationship::RelatedSpawner<ChildOf>| {
                // **The tool modes are one gooey toggle group** (UI-37, #231): a group of options of
                // which exactly one is on, which is a tool mode exactly. Its value is written from
                // `EditorState` every frame by [`the_group_follows_the_armed_tool`], and a choice it
                // reports is handed to the same chord queue every other button uses.
                let mut modes = parent.spawn((
                    ToolModes,
                    bevy_gooey::toggle_group(
                        std::iter::once(icons::index(PLACE_ICON).map(|index| {
                            bevy_gooey::GroupOption::icon(
                                bevy_gooey::IconSource::Atlas { image: image.clone(), layout: layout.clone(), index },
                                bevy_gooey::Tooltip::new(PLACE_TITLE),
                            )
                        }))
                        .flatten()
                        .chain(tools().iter().filter_map(|(id, _)| {
                            let command = commands::by_id(id)?;
                            let index = icons::index(command.id)?;
                            Some(bevy_gooey::GroupOption::icon(
                                bevy_gooey::IconSource::Atlas { image: image.clone(), layout: layout.clone(), index },
                                bevy_gooey::Tooltip::new(command.title).with_shortcut(command.shortcut),
                            ))
                        })),
                        0,
                    ),
                ));
                // The group stands off the window's edge by that gap, in logical px (`Space::Px`).
                modes.insert(bevy_gooey::Spacing { margin: crate::chrome::margin_left(bevy_gooey::Space::Px(edge)), ..default() });
                // **The one labelled button that is not a mode stands with the modes**, before the
                // separator: `LABELLED_BUTTON`'s own doc says Fill is "which the brush's own row of
                // tools stands beside". Spawning it in the command loop below put a 46.7-px worded
                // button in a run of 30.8-px pictures — `feedback-1793927698061`, *"Why is the fill
                // button larger here? It looks out of place."* — because that loop is where the
                // pictures live and Fill is not one of them.
                //
                // It stays a one-shot command and not a group option: `toolbar.rs`'s header says Fill
                // is "a one-shot flood from the cursor, not a mode", and a lit Fill would claim a mode
                // the editor does not have.
                if let Some(command) = commands::by_id(LABELLED_BUTTON.0)
                    && let Some(index) = icons::index(command.id)
                {
                    parent.spawn((
                        ToolbarCommand(command.id),
                        bevy_gooey::ButtonVariant::Ghost,
                        bevy_gooey::icon_button(
                            bevy_gooey::IconSource::Atlas { image: image.clone(), layout: layout.clone(), index },
                            bevy_gooey::Tooltip::new(command.title).with_shortcut(command.shortcut),
                        ),
                    ));
                }
                parent.spawn(bevy_gooey::toolbar_separator());
                // **Everything else is a one-shot command**, so it is a button and never a member of a
                // group: a lit "Undo" would be lying about what `Ctrl+Z` did.
                for command in shown() {
                    // The modes are the group above, and Fill is the labelled button above it: this loop
                    // is the pictures, and neither belongs in it.
                    if tools().iter().any(|(id, _)| *id == command.id) || command.id == LABELLED_BUTTON.0 {
                        continue;
                    }
                    let Some(index) = icons::index(command.id) else {
                        continue;
                    };
                    parent.spawn((
                        ToolbarCommand(command.id),
                        bevy_gooey::ButtonVariant::Ghost,
                        bevy_gooey::icon_button(
                            bevy_gooey::IconSource::Atlas { image: image.clone(), layout: layout.clone(), index },
                            bevy_gooey::Tooltip::new(command.title).with_shortcut(command.shortcut),
                        ),
                    ));
                    if command.id == "file.save" {
                        parent.spawn(bevy_gooey::toolbar_separator());
                    }
                }
            }))),
        ))
        .id();
    put_after_the_chrome_bar(commands, frame, bar);
}

/// **Put the bar straight after the chrome bar**, found where it stands when the command runs: the menu bar goes in at
/// index 0 from a system of its own, and either may reach a new frame first, so a fixed index would put this bar above
/// the chrome bar or below it depending on which came first. A new bar and one a switch kept go in the same way.
fn put_after_the_chrome_bar(commands: &mut Commands, frame: &crate::chrome::Frame, bar: Entity) {
    let (root, chrome_bar) = (frame.root, frame.chrome_bar);
    commands.queue(move |world: &mut World| {
        let at = world
            .get::<Children>(root)
            .and_then(|kids| kids.iter().position(|kid| kid == chrome_bar))
            .map_or(1, |chrome| chrome + 1);
        // A frame torn down in the same frame takes the bar with it, rather than leaving it a root of its own.
        match world.get_entity_mut(root) {
            Ok(mut root) => {
                root.insert_children(at, &[bar]);
            }
            Err(_) => {
                world.despawn(bar);
            }
        }
    });
}

/// **A control is enabled exactly where its command acts** (#690). A key's binding names the tab it fires on
/// ([`crate::keys::Context`]), and the bar asks each control the same question of the tab that is open
/// ([`Mode::context`]), so a button never offers what its key would refuse. On the Map everything acts. World
/// and Kit draw the bar, but the modes, Fill, undo and redo are bound on the Map tab: there the mode group, its
/// options and those buttons are `InteractionDisabled`, which gooey draws on the panel's fill and not the
/// accent and which takes no click — **nothing is lit where it cannot act**. Save and the snip fire on every
/// tab and stay live.
///
/// Gated on the tab changing or a bar arriving, so a settled editor costs two reads.
fn quiet_what_cannot_act_here(
    mut commands: Commands,
    mode: Option<Res<Mode>>,
    arrived: Query<(), Added<Toolbar>>,
    buttons: Query<(Entity, &ToolbarCommand, Has<InteractionDisabled>)>,
    groups: Query<(Entity, &Children, Has<InteractionDisabled>), With<ToolModes>>,
    options: Query<(&bevy_gooey::Role, Has<InteractionDisabled>)>,
) {
    let Some(mode) = mode else {
        return;
    };
    if !mode.is_changed() && arrived.is_empty() {
        return;
    }
    let here = mode.context();
    let acts = |id: &str| commands::by_id(id).is_some_and(|command| crate::keys::fires_in(command.context, here));
    let mut follow = |entity: Entity, disabled: bool, live: bool| {
        if live && disabled {
            commands.entity(entity).remove::<InteractionDisabled>();
        } else if !live && !disabled {
            commands.entity(entity).insert(InteractionDisabled);
        }
    };
    for (button, command, disabled) in &buttons {
        follow(button, disabled, acts(command.0));
    }
    // Place has no command of its own: the group acts where its tools do, and every option with it.
    let modes_act = tools().iter().any(|(id, _)| acts(id));
    for (group, kids, disabled) in &groups {
        follow(group, disabled, modes_act);
        for option in kids.iter() {
            if let Ok((bevy_gooey::Role::GroupOption, disabled)) = options.get(option) {
                follow(option, disabled, modes_act);
            }
        }
    }
}

/// **One button pressed.** `bevy_gooey`'s `Activate` fires for a click, `Enter`, `Space` and a pad's
/// south button, so a person meets the button the same way whichever they use, and the chord that
/// goes down is the one the settings gave the command.
pub fn on_toolbar_activate(
    activate: On<bevy_gooey::Activate>,
    buttons: Query<&ToolbarCommand>,
    mut chords: ResMut<crate::command_run::Chords>,
    live: Res<crate::keys::Live>,
    mut state: Option<ResMut<EditorState>>,
) {
    let Ok(button) = buttons.get(activate.entity) else {
        return;
    };
    let Some(command) = commands::by_id(button.0) else {
        return;
    };
    chords.queue(&command, *live);
    if let Some(state) = state.as_deref_mut() {
        // "Asked for", as the palette says: the chord goes down a frame or two from now and the
        // dispatcher judges it then.
        state.status.note(format!("asked for {} from the toolbar", command.title));
    }
}

/// **The armed tool's button is lit**, and no other. Read from `EditorState`, never from the button, so
/// the light follows the key exactly: a click is a chord, the chord arms the tool, and this repaints
/// on the frame the tool changed.
///
/// **Only the tool modes are marked.** Undo, save and a snip are one-shot commands with no "on" state,
/// and a button that lit when pressed and stayed lit would be lying about what `Ctrl+Z` did.
pub fn mark_the_armed_tool(
    state: Res<EditorState>,
    buttons: Query<(&ToolbarCommand, &Children)>,
    mut icons: Query<&mut bevy_gooey::IconTone>,
) {
    if !state.is_changed() {
        return;
    }
    for (button, children) in &buttons {
        let Some(tone) = tools().iter().find(|(id, _)| *id == button.0).map(|(_, tone)| *tone) else {
            continue;
        };
        let on = tone == Some(state.tool);
        let want = if on { bevy_gooey::IconTone::Accent } else { bevy_gooey::IconTone::Text };
        for child in children.iter() {
            if let Ok(mut current) = icons.get_mut(child)
                && *current != want
            {
                *current = want;
            }
        }
    }
}

/// **Which button stands for which tool**, and `None` for a command that is not a mode. Written once so
/// [`mark_the_armed_tool`] and the test that reads the lights agree about it.
pub fn tools() -> [(&'static str, Option<Tool>); 4] {
    [
        ("map.move_mode", Some(Tool::Move)),
        ("map.remove_mode", Some(Tool::Remove)),
        ("map.clone_mode", Some(Tool::Clone)),
        ("map.run_mode", Some(Tool::Run)),
    ]
}

/// The icon of the group's first option, **Place**: the tool every other one goes back to. It is the
/// tool the editor is in when none is armed, so it has no command of its own and no key — pressing the
/// armed tool's key again, or Esc, is the way back, and this option asks for the same.
const PLACE_ICON: &str = "map.place_mode";

/// What the Place option is called when the pointer rests on it.
const PLACE_TITLE: &str = "Place pieces";

/// How many options come before the tool commands in the mode group: Place.
const PLACE_OPTIONS: usize = 1;

/// The group option that stands for `tool`, or `None` for a tool the group has no option for (a
/// note). Place is option 0; the tool commands follow in [`tools`]'s order.
pub fn group_index(tool: Tool) -> Option<usize> {
    if tool == Tool::Place {
        return Some(0);
    }
    tools().iter().position(|(_, mode)| *mode == Some(tool)).map(|at| at + PLACE_OPTIONS)
}

/// **The command whose key puts `tool` down**, because every tool's key is a toggle: pressed while its
/// tool is armed, it returns to placing. `None` for Place, which is already where that leads.
fn leaving_command(tool: Tool) -> Option<&'static str> {
    tools()
        .iter()
        .find(|(_, mode)| *mode == Some(tool))
        .map(|(id, _)| *id)
        .or((tool == Tool::Note).then_some("map.note"))
}

/// The word under a tool's icon.
fn tool_label(tool: Tool) -> &'static str {
    match tool {
        Tool::Place => "Place",
        Tool::Move => "Move",
        Tool::Remove => "Erase",
        Tool::Clone => "Clone",
        Tool::Run => "Run",
        Tool::Note => "Note",
    }
}

/// The command whose button carries a word under its icon besides the tool modes: Fill, which the
/// brush's own row of tools stands beside.
const LABELLED_BUTTON: (&str, &str) = ("map.fill", "Fill");

/// On a button or option that already has its word under the icon, so the system that adds it runs
/// once per control.
#[derive(Component)]
struct Labelled;

/// **The primary group says what it is in words** (#302): Place, Move, Erase, Clone, Run and Fill each
/// carry their name under the icon, because a 16 px picture is a guess until it has been learned. The
/// rest of the bar (undo, save, view) are the pictures everyone already knows and keep only a tooltip.
///
/// gooey's icon button has no slot for a word, so the control's node turns to a column and a text child
/// goes under the icon. The skin styles the text by its role (a group option's like a tab's, so it
/// takes the accent's ink when the option is lit, and a button's a control's: a `ButtonText` is what the
/// dialogs and the pages call a button, and a toolbar's picture is not one of those); the control, its tooltip and its chip stay what they
/// were, so the tooltip still sits on the button.
fn label_the_primary_group(
    mut commands: Commands,
    groups: Query<&Children, With<ToolModes>>,
    buttons: Query<(Entity, &ToolbarCommand), Without<Labelled>>,
    done: Query<(), With<Labelled>>,
    mut nodes: Query<&mut Node>,
) {
    let mut label = |entity: Entity, words: &'static str, role: bevy_gooey::Role| {
        if let Ok(mut node) = nodes.get_mut(entity) {
            node.flex_direction = FlexDirection::Column;
        }
        commands.entity(entity).insert(Labelled).with_child((
            Text::new(words),
            role,
            bevy::text::TextLayout::new(bevy::text::Justify::Center, bevy::text::LineBreak::NoWrap),
            bevy::picking::Pickable::IGNORE,
        ));
    };
    let words: Vec<&'static str> = std::iter::once(tool_label(Tool::Place))
        .chain(tools().iter().filter_map(|(_, mode)| mode.map(tool_label)))
        .collect();
    for children in &groups {
        for (option, words) in children.iter().zip(words.iter().copied()) {
            if done.get(option).is_err() {
                label(option, words, bevy_gooey::Role::TabText);
            }
        }
    }
    for (button, command) in &buttons {
        if command.0 == LABELLED_BUTTON.0 {
            label(button, LABELLED_BUTTON.1, bevy_gooey::Role::ControlText);
        }
    }
}

/// **The group's value is the armed tool**, written from `EditorState` every time the tool changes and
/// never from a click. A group that held its own choice would drift the moment a key changed the mode:
/// `B` would arm Move and leave Remove lit.
pub fn the_group_follows_the_armed_tool(
    state: Res<EditorState>,
    arrived: Query<(), Added<ToolModes>>,
    mut groups: Query<&mut bevy_gooey::ToggleGroupValue, With<ToolModes>>,
) {
    // **And on the frame a bar arrives** (#690): the bar is spawned on each workspace that has one, its group
    // starts on option 0, and the tool it should light may have been armed long before.
    if !state.is_changed() && arrived.is_empty() {
        return;
    }
    let Some(at) = group_index(state.tool) else {
        return;
    };
    for mut value in &mut groups {
        if value.0 != at {
            value.0 = at;
        }
    }
}

/// **A choice in the group arms that tool**, through the same chord the key sends: the group's index is
/// mapped back to a command id, and the id goes into the queue. So a click on a group option, a click on
/// a toolbar button and a key press are one path, and the tool changes where the key handler says so.
pub fn on_tool_mode_chosen(
    change: On<bevy_gooey::ValueChange<usize>>,
    groups: Query<(), With<ToolModes>>,
    mut chords: ResMut<crate::command_run::Chords>,
    live: Res<crate::keys::Live>,
    mut state: Option<ResMut<EditorState>>,
) {
    if groups.get(change.source).is_err() {
        return;
    }
    // Option 0 is Place, which is no command: the way to it is the armed tool's own key again.
    let id = match change.value.checked_sub(PLACE_OPTIONS) {
        Some(at) => tools().get(at).map(|(id, _)| *id),
        None => state.as_deref().and_then(|state| leaving_command(state.tool)),
    };
    let Some(command) = id.and_then(commands::by_id) else {
        return;
    };
    chords.queue(&command, *live);
    if let Some(state) = state.as_deref_mut() {
        state.status.note(format!("asked for {} from the toolbar", command.title));
    }
}
