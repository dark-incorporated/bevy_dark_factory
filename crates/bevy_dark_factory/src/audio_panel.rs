//! **The audio panel** (S-01): a project's sound as the crate's own data, with its mix drawn live and every cue
//! auditionable.
//!
//! The file is `<project>/family/bevy_siren.yaml` — `bevy_siren`'s own `SirenSettings`, `CueTable` and `MusicScore`,
//! read and written through their own `serde` derives. There is no second schema: the editor writes the crate's
//! types, so the file a person edits here is the file a game ships.
//!
//! # What it draws
//!
//! - A row per bus: its level from [`bevy_siren::SirenMix::db`], the duck on it from `ducked`, and its live voices from `voices`.
//!   The same numbers a game's own meter reads, because it is the same resource.
//! - Every cue by name, the chosen one marked, and an **Audition** button on it that writes
//!   `Play::cue` ([`bevy_siren::Play`]) — the door a game uses. Up and Down walk the list and the marked row *is* what Audition plays, so
//!   choosing and playing are one fact. The log then shows `Played` or `Refused`, so a cue that cannot play says
//!   what it is missing rather than doing nothing.
//! - The tail of [`bevy_siren::SirenLog`], so "why is the music quiet" has an answer on the screen.
//!
//! # A file that does not parse
//!
//! Is a problem on the status line, naming the file and what is wrong with it — never a silently defaulted mix. The
//! editor keeps the last good settings in the world while the file is broken, which is what the family layer's hot
//! reload does for every crate.

use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use blueprint_model::family::settings_dir;

use crate::editor::EditorState;
use crate::project::Project;
use crate::world_lists::NO_CUES;

/// The crate's settings file, under the project's `family/`.
pub const FILE: &str = "bevy_siren.yaml";

/// The panel's own state: what was read, what is selected, and whether it is open.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct AudioPanel {
    /// Whether the panel is open. A panel, not a viewport overlay: it costs nothing when closed.
    pub open: bool,
    /// The bus list from the open project's file, in the file's order.
    pub buses: Vec<String>,
    /// The cue names, in the file's order: the audition target list.
    pub cues: Vec<String>,
    /// The project root the last read came from, so a different project re-reads.
    pub root: Option<std::path::PathBuf>,
    /// The bytes last read, so a frame that changed nothing costs nothing and a hand edit is picked up once.
    pub text: String,
    /// **Why the file is not the crate's tables**, when it is not: the sentence
    /// [`world_lists`](crate::world_lists) draws as the cue list's one row. It lives here because this is the
    /// reader that parses the file as `SirenSettings` and `CueTable`; the list's own plain-value reader cannot
    /// say why a file that is not a table at all did not read, and two readers would be two wordings for one
    /// failure. `None` when the file read, and when there is no file at all (a project may ship no sound yet).
    pub unread: Option<String>,
}

/// Audition the selected cue, the top of the panel's one row of buttons.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct AuditionButton;

/// The AUDIO section of the controls panel: the heading, **Audition**, then the three lists the module
/// note says it draws (#557) — one row per bus, every cue with the selected one marked, and the tail of
/// the crate's log. Each list is an empty column here and filled by a system in [`AudioPanelPlugin`], so
/// what a person reads is this frame's mix and log, not a snapshot of the frame the panel was built.
pub fn spawn_section(s: &mut ChildSpawnerCommands) {
    use crate::chrome::section;
    section(s, "AUDIO");
    s.spawn((bevy_gooey::button("Audition cue"), AuditionButton));
    s.spawn((BusList, list_column()));
    // **The cue column is `world_lists`' own list** (#392, #600), not a column of this panel: its reader
    // names the cues from the project's file in *both* builds, marks the one `world_lists::SelectedCue`
    // holds — the same value Audition reads — and says a file that does not read as a row of its own.
    // One writer per list is the rule the rest of this file follows, so the panel draws the bus mix and
    // the log and leaves the cue names to the list that owns them.
    crate::world_lists::spawn_cue_column(s);
    crate::world_lists::spawn_cue_uses(s);
    s.spawn((LogTail, list_column()));
}

/// A panel list's column: gooey's, one row tight under the next.
fn list_column() -> impl Bundle {
    (bevy_gooey::column(0.0), crate::chrome::Gap::Tight.class())
}

/// **One row per bus**, from this frame's [`bevy_siren::SirenMix`]: the list [`bus_rows`] fills.
#[derive(Component, Clone, Copy)]
pub struct BusList;

/// **The tail of [`bevy_siren::SirenLog`]**: the last few decisions, newest first, so an audition's
/// `Played`/`Refused` is on screen and not only on the status line.
#[derive(Component, Clone, Copy)]
pub struct LogTail;

/// How many log entries the tail shows. Three fits under the cue list without pushing the panel past its
/// first screen, and a `Refused` and the `Stopped` that preceded it are what a person asks about.
pub const LOG_TAIL_ROWS: usize = 3;

/// What the cue list marks the chosen cue with, the Sound workspace's cue list's own mark.
const CUE_MARK: &str = crate::world_lists::CUE_MARK;

/// **The sound file, followed through the project index** (#501): what [`read_audio`] keeps so it reads the file when the
/// index says it moved and at no other time, the same subscription the world lists keep for their files.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct SoundFile<'w, 's> {
    index: Option<Res<'w, crate::project_index::ProjectIndex>>,
    follow: Local<'s, crate::world_lists::Follow>,
}

/// **Read the project's sound** when the project opens and when the project index says its file moved — once per
/// change, never per frame and never on a clock. A file that does not read is a problem naming the field, and the
/// previous settings stay in force.
///
/// A missing file is not a problem: a project may ship no sound yet, and the crate's defaults are a real mix. The
/// panel still names the buses and the cues from the crate's own tables, so it has rows to draw.
// A Bevy system's parameters are what it reads and writes; the crate's three stores, the index and the status line are each
// one of them.
#[allow(clippy::too_many_arguments)]
pub(crate) fn read_audio(
    mut panel: ResMut<AudioPanel>,
    project: Option<Res<Project>>,
    mut settings: ResMut<bevy_siren::SirenSettings>,
    mut table: ResMut<bevy_siren::CueTable>,
    // **The handles Audition needs** (#555). `Play::cue` refuses with `MissingSound` unless the
    // `SirenSounds` map already holds a handle for the key, and nothing in this editor ever loaded
    // one — so the button could only ever refuse, and the panel's own note ("a cue that cannot play
    // says what it is missing") was the only thing it ever said.
    mut sounds: ResMut<bevy_siren::SirenSounds>,
    server: Option<Res<AssetServer>>,
    mut state: ResMut<EditorState>,
    mut sound_file: SoundFile,
) {
    let Some(project) = project else {
        return;
    };
    let reopened = panel.root.as_deref() != Some(project.root.as_path());
    let SoundFile { index, follow } = &mut sound_file;
    let moved = index
        .as_deref()
        .and_then(|index| follow.moved(index, Some(&project), project.is_changed(), |p| Some(settings_dir(&p.root).join(FILE))));
    // **A project's mix is in force from the frame it opens**, before the index has listed anything; after that the file
    // is read only when the index says it moved (#501). A file the index cannot follow (one the project's `.gitignore`
    // hides) is said in the cue list's row and not read, as the build without the crate says it.
    match moved {
        Some(Err(why)) if !reopened => {
            panel.unread = Some(why);
            return;
        }
        None if !reopened => return,
        _ => {}
    }
    panel.root = Some(project.root.clone());
    let file = settings_dir(&project.root).join(FILE);
    // **Every read of the project's sound is counted** (`world_lists::reads_on_this_thread`), because this is the one
    // reader of `family/bevy_siren.yaml` in a `bevy_siren` build: the cue list takes its names from what this read, so
    // "one changed file is one read" is a fact about this line and a test has to be able to see it.
    crate::world_lists::counted_read();
    let Ok(text) = std::fs::read_to_string(&file) else {
        if reopened {
            panel.buses = settings
                .bus_names()
                .iter()
                .map(|b| (*b).to_owned())
                .collect();
            panel.cues = table.names().map(str::to_owned).collect();
            panel.text.clear();
        }
        return;
    };
    if !reopened && panel.text == text {
        return;
    }
    panel.text.clone_from(&text);
    match read_file(&text) {
        Ok((read, table_read)) => {
            // **The cues are judged against this file's own mix table** (#554), not against the crate's
            // four `DEFAULT_BUSES`. `CueTable::problems` answers that second question, and a project that
            // declares `SFX` was told every cue on it named an unknown bus while the crate would play them;
            // a typo on a bus the file *does* declare was the one fault it could not see, because the crate's
            // four always answered. `problems_with` is the crate's own accessor for "against the mix table's
            // list", and [`bevy_siren::SirenSettings::bus_names`] is the same list the mix and the crate's
            // `UnknownBus` check read — so the panel cannot disagree with the mix it draws.
            let buses = read.bus_names();
            for problem in read
                .problems()
                .into_iter()
                .chain(table_read.problems_with(&buses).into_iter().map(|p| p.to_string()))
            {
                state.status.problem(format!("family/{FILE}: {problem}"));
            }
            panel.buses = read.bus_names().iter().map(|b| (*b).to_owned()).collect();
            panel.cues = table_read.names().map(str::to_owned).collect();
            panel.unread = None;
            *settings = read;
            *table = table_read;
            // **Ask the asset server for the project's sounds** (#555): `audio/<key>`, which the
            // server reads from the project's asset root as `<project>/assets/audio/<key>` — where a
            // game puts them and where `bevy_siren`'s own quick start reads from. A handle is
            // held whether or not the file is there, so a project with no `audio/` folder plays a
            // voice that waits for an asset which will never arrive; the panel names that file from
            // `SirenSounds::failed` when the cue is auditioned, which is the one place a person is
            // told which file to deliver (`SirenSounds::missing` is now the narrower case: no handle
            // at all, which is a build whose read had no asset server).
            if let Some(server) = server.as_deref() {
                sounds.load_table(server, &table, "audio");
            }
        }
        Err(problem) => {
            // **Said once.** The cue list draws `panel.unread` as its one row, where a person is looking at
            // the cues; raising the status line as well said the same sentence twice (measured: three "did not
            // read" nodes where the test counts one).
            panel.unread = Some(problem);
        }
    }
}

/// One file's worth of the crate's own types: the mix table and the cue table, which is what an audio panel edits.
///
/// The file is one YAML mapping holding the two (`settings:`, `cues:`), so a project ships one sound file and the editor has one file to write. A
/// score is a third section and is optional: a project with no adaptive music leaves it out.
pub fn read_file(text: &str) -> Result<(bevy_siren::SirenSettings, bevy_siren::CueTable), String> {
    #[derive(serde::Deserialize)]
    struct File {
        #[serde(default)]
        settings: bevy_siren::SirenSettings,
        #[serde(default)]
        cues: bevy_siren::CueTable,
    }
    let file: File = blueprint_model::yaml::from_str(text, &format!("family/{FILE}"))
        .map_err(|p| format!("family/{FILE} did not read: {}", p.line()))?;
    Ok((file.settings, file.cues))
}

/// Writes the mix table and the cue table back as the same file, so an edit here is an edit a game ships.
pub fn write_file(
    settings: &bevy_siren::SirenSettings,
    table: &bevy_siren::CueTable,
) -> Result<String, String> {
    #[derive(serde::Serialize)]
    struct File<'a> {
        settings: &'a bevy_siren::SirenSettings,
        cues: &'a bevy_siren::CueTable,
    }
    blueprint_model::yaml::to_string(&File {
        settings,
        cues: table,
    })
    .map_err(|p| format!("family/{FILE} does not write: {}", p.line()))
}

/// **The crate's answers an audition asks for**, grouped so [`on_audition`] stays inside clippy's
/// seven-argument limit without an `#[allow]` — the same grouping `Columns` uses for the same reason.
///
/// **Each is an `Option`, because `on_audition` is a global observer**: it runs for every `Activate` in the editor, on
/// every screen, and a bare `Res` would panic it wherever the crate's stores are not in the world (a door that does not
/// build them, a screen before the editor). The panic would come from a click that has nothing to do with sound, so the
/// observer first checks that the click was Audition's, then says what is missing (`no_observer_hides_a_resource_in_a_bundle`).
#[derive(bevy::ecs::system::SystemParam)]
pub struct Sounds<'w> {
    sounds: Option<Res<'w, bevy_siren::SirenSounds>>,
    table: Option<Res<'w, bevy_siren::CueTable>>,
    server: Option<Res<'w, AssetServer>>,
}

/// **Audition** plays the selected cue through the crate's own door, and the panel's log shows what happened.
pub fn on_audition(
    activate: On<Activate>,
    buttons: Query<(), With<AuditionButton>>,
    selected: Option<Res<crate::world_lists::SelectedCue>>,
    mut plays: MessageWriter<bevy_siren::Play>,
    heard: Sounds,
    mut state: ResMut<EditorState>,
) {
    if buttons.get(activate.entity).is_err() {
        return;
    }
    let (Some(sounds), Some(table), Some(server)) = (heard.sounds.as_deref(), heard.table.as_deref(), heard.server.as_deref())
    else {
        state
            .status
            .problem("Audition needs the sound crate's stores and the asset server, and this editor has not built them".to_owned());
        return;
    };
    let Some(cue) = selected.as_deref().and_then(|s| s.0.clone()) else {
        state
            .status
            .problem("no cue is selected to audition".to_owned());
        return;
    };
    // **What the cue cannot sound, said before it is asked to** (#555): `Play::cue` answers
    // `Refused { MissingSound }` and the log explains it a frame later, but the button's own answer
    // should be the file a person has to deliver. A project with no `audio/` folder is the ordinary
    // case, and "no file `assets/audio/step_01` in this project" is a thing an author can act on.
    //
    // **Two ways a key cannot sound, and the panel says both** (#555's second half). `missing` is a
    // key with no handle at all — a read that had no asset server; `failed` is a handle whose file
    // the server could not read, which is *every* cue of a project with an empty `audio/` folder now
    // that `read_audio` asks for all of them. The crate asks "is there a handle", not "did the file
    // load", so the play below still reaches `Played`; these two are what make the failure visible,
    // and both mean the same next step, so they are one list in the cue's own order and both name
    // the file.
    let wanted: Vec<String> = table
        .get(&cue)
        .map(|cue| cue.sounds.iter().map(|slot| slot.sound.clone()).collect())
        .unwrap_or_default();
    let missing = sounds.missing(table);
    let failed = sounds.failed(server, table);
    let unloaded: Vec<String> = wanted
        .into_iter()
        .filter(|key| missing.contains(key) || failed.contains(key))
        .collect();
    plays.write(bevy_siren::Play::cue(cue.clone()));
    if unloaded.is_empty() {
        state.status.note(format!("auditioning `{cue}`"));
    } else {
        // **The path a person must deliver, relative to the project** — not `audio/<key>`, which is
        // the *asset-server*-relative spelling `read_audio` hands to `load_table`: the server is
        // rooted at `<project>/assets` (`project_assets.rs`), so the file on disk is
        // `assets/audio/<key>`.
        state.status.problem(format!(
            "`{cue}` has no sound: {}. Put {} at {} in the project's `assets/audio/` folder",
            unloaded
                .iter()
                .map(|key| format!("`{key}`"))
                .collect::<Vec<_>>()
                .join(", "),
            if unloaded.len() == 1 { "it" } else { "them" },
            unloaded
                .iter()
                .map(|key| format!("`assets/audio/{key}`"))
                .collect::<Vec<_>>()
                .join(", "),
        ));
    }
}

/// The rows the panel draws: one line per bus, from this frame's mix.
pub fn bus_rows(mix: &bevy_siren::SirenMix, buses: &[String]) -> Vec<String> {
    buses
        .iter()
        .map(|bus| {
            let db = mix.bus_db(bus);
            let duck = mix.ducked.get(bus).copied().unwrap_or(0.0);
            let duck = if duck < -0.05 {
                format!("  duck {duck:+.1} dB")
            } else {
                String::new()
            };
            format!(
                "{bus:<10} {db:+6.1} dB{duck}  {} voices",
                mix.voices_on(bus)
            )
        })
        .collect()
}

/// **The cue list's rows**: every cue in the file's order, the chosen one marked with [`crate::world_lists::CUE_MARK`]. The marked
/// row is the same [`world_lists::SelectedCue`](crate::world_lists::SelectedCue) **Audition** reads, so what a
/// person reads as chosen is what plays. The `world_lists` reader owns the list that shows these; this function
/// stays because the panel's own note and a test name it.
pub fn cue_rows(panel: &AudioPanel, selected: Option<&str>) -> Vec<String> {
    if panel.cues.is_empty() {
        // **An empty project says so**, the way the log does. A blank column reads as a list that failed to fill,
        // and a list with no rows is never "filled" (see `fill`), so it would be rebuilt every frame.
        return vec![NO_CUES.to_owned()];
    }
    panel
        .cues
        .iter()
        .map(|name| if selected == Some(name.as_str()) { format!("{CUE_MARK}{name}") } else { name.clone() })
        .collect()
}

/// **The log tail**: the newest [`LOG_TAIL_ROWS`] of the crate's own [`bevy_siren::SirenLog`], each in the
/// crate's [`Entry::line`](bevy_siren::Entry::line) — the sentence a panel shows verbatim. An empty log says so,
/// so the section never reads as a list that failed to fill.
pub fn log_rows(log: &bevy_siren::SirenLog) -> Vec<String> {
    let rows: Vec<String> = log.recent(LOG_TAIL_ROWS).map(bevy_siren::Entry::line).collect();
    if rows.is_empty() { vec!["nothing has played yet".to_owned()] } else { rows }
}

/// **The three lists, once per frame** (#557). Each is cheap: `bus_rows` reads the mix the crate already
/// computed, and `cue_rows`/`log_rows` read resources that cost nothing when nothing changed. A row is
/// rebuilt only when its column is empty or the text it would show differs, so a settled frame costs a
/// couple of string compares and writes nothing.
/// **The three columns the panel fills**, grouped so [`draw_the_panel`] stays inside clippy's seven-argument
/// limit without an `#[allow]` — the same grouping `keymap_screen::Drawn` uses for the same reason.
#[derive(bevy::ecs::system::SystemParam)]
struct Columns<'w, 's> {
    buses: Query<'w, 's, (Entity, Has<Children>), With<BusList>>,
    tail: Query<'w, 's, (Entity, Has<Children>), With<LogTail>>,
}

fn draw_the_panel(
    panel: Option<Res<AudioPanel>>,
    mix: Option<Res<bevy_siren::SirenMix>>,
    log: Option<Res<bevy_siren::SirenLog>>,
    mut columns: Columns,
    mut held: Local<Drawn>,
    mut commands: Commands,
) {
    if let Some(mix) = mix.as_deref() {
        let rows = bus_rows(mix, panel.as_deref().map_or(&[][..], |p| p.buses.as_slice()));
        fill(&mut commands, &mut columns.buses, &mut held.buses, rows);
    }
    if let Some(log) = log.as_deref() {
        fill(&mut commands, &mut columns.tail, &mut held.log, log_rows(log));
    }
}

/// **The frame's cue walk**, ordered before `world_lists::refresh_cues` draws the marks it writes
/// (#557): both run in `Update`, and without this a press moved the value a frame after the row that
/// shows it. It was the same frame while the panel drew its own cue column; the list that owns the
/// names now draws them, so the order has to be said.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TheCueWalk;

/// **Up/Down over the cue list moves the selection** (#557): `world_lists::SelectedCue`, the editor's own,
/// which **Audition** reads and reveal and the nav stops write, so walking the list is choosing what plays. The arrows are `bevy::prelude::KeyCode`'s own rather
/// than a census row — the Sound tab has no keys of its own (`Context::Sound`), and a tab whose whole job
/// is one list does not need two more rows in every context's budget. Read only while the Sound workspace
/// is open, which is the run condition on [`draw_the_panel`]'s sibling below.
fn walk_the_cues(
    keys: Res<ButtonInput<KeyCode>>,
    live: Option<Res<crate::keys::Live>>,
    panel: Option<Res<AudioPanel>>,
    mut selected: Option<ResMut<crate::world_lists::SelectedCue>>,
) {
    // **A field, the palette, the report, a dialog or the terminal owns the keys**, and an arrow must move what
    // the person is looking at: `Live` is the editor's one answer to who owns the keyboard (2026-11-23 review,
    // finding 4 — the arrows moved the cue while the palette's own selection moved with them).
    if live.as_deref().is_some_and(|l| l.0 == crate::keys::Context::Typing) {
        return;
    }
    let (Some(panel), Some(selected)) = (panel, selected.as_mut()) else { return };
    let step = if keys.just_pressed(KeyCode::ArrowDown) {
        1
    } else if keys.just_pressed(KeyCode::ArrowUp) {
        -1
    } else {
        return;
    };
    selected.0 = stepped(&panel.cues, selected.0.as_deref(), step);
}

/// **The cue `step` presses away from `from`**, wrapping, for a list in the file's order. A list walked for the
/// first time starts at its first row in the direction pressed.
fn stepped(cues: &[String], from: Option<&str>, step: isize) -> Option<String> {
    if cues.is_empty() {
        return None;
    }
    let len = cues.len() as isize;
    let at = from
        .and_then(|name| cues.iter().position(|c| c == name))
        .map_or(if step > 0 { -1 } else { 0 }, |i| i as isize);
    let next = ((at + step) % len + len) % len;
    cues.get(next as usize).cloned()
}

/// What each list last drew, so a frame with no change writes nothing.
#[derive(Default)]
struct Drawn {
    buses: Option<Vec<String>>,
    log: Option<Vec<String>>,
}

/// Puts `rows` into every column in `lists` when they differ from `held`, or when a column has no children
/// yet: despawn the children, then spawn one [`chrome::body`] line per row. The editor's own row vocabulary,
/// so these read like every other panel's list.
fn fill(commands: &mut Commands, lists: &mut Query<(Entity, Has<Children>), With<impl Component>>, held: &mut Option<Vec<String>>, rows: Vec<String>) {
    // **A list that is meant to be empty is not "unfilled"** — `world_lists::keep` documents the trap: `with_children`
    // with no rows inserts no `Children`, so a column of an empty list reads as never filled and is despawned and
    // respawned every frame (2026-11-23 review, finding 3).
    let unfilled = !rows.is_empty() && lists.iter().any(|(_, filled)| !filled);
    if held.as_ref() == Some(&rows) && !unfilled {
        return;
    }
    for (list, _) in lists.iter_mut() {
        commands.entity(list).despawn_children();
        commands.entity(list).with_children(|p| {
            for row in &rows {
                crate::chrome::body(p, row.clone(), crate::chrome::ink().text);
            }
        });
    }
    *held = Some(rows);
}

/// **The panel's plugin** (S-01, #274, #557).
///
/// The panel's state, its audition observer and its file reader are registered beside the rest of the
/// editor's observers in `editor.rs`, where a reader looking for "what does the editor register" finds them
/// without opening this file — a second `SirenPlugins` add here would panic. What this plugin owns is the
/// **drawing**: the three systems that fill the lists [`spawn_section`] leaves empty, so the note above is
/// what the screen shows rather than a promise about a panel that drew a heading and one button.
///
/// **Why this is worth a real plugin rather than an empty struct.** `editor.rs` named this type from the
/// commit that added the panel and the type was never written, so `--features bevy_siren` did not compile
/// for any caller; and for a while after it compiled, nothing it said it drew had a caller (#557).
pub struct AudioPanelPlugin;

impl bevy::prelude::Plugin for AudioPanelPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.add_systems(
            Update,
            // **The walk before the draw**, so a press and the marked row it moves land in one frame.
            (walk_the_cues.in_set(TheCueWalk), draw_the_panel)
                .chain()
                .run_if(crate::workspace::Workspace::sound_is_open),
        );
    }
}
