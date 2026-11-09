//! **The editor as host of the owner's family crates** (#96).
//!
//! Each family crate is an optional cargo feature of this crate, named after it. A project's
//! `family.ron` (`blueprint_model::family`) lists the ones it uses. A listed crate this build was
//! compiled without is a problem the author is told about, never a silent no-op.

use bevy::prelude::*;
use bevy::reflect::TypeRegistry;
use blueprint_model::family::{Family, FamilyWatch, SettingsChange, SettingsKey, read_settings_files, settings_file, settings_path};
use blueprint_model::live::KindVerdict;
use blueprint_model::reload_check::Refusal;
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::editor::EditorState;
use crate::project::Project;

/// Every crate this editor can host, whether or not this build has it. Add a name here in the
/// commit that adds the cargo feature.
pub const KNOWN: &[&str] = &[
    "bevy_carnage",
    "bevy_elements",
    "bevy_flora",
    "bevy_gooey",
    "bevy_lights",
    "bevy_orca",
    "bevy_siren",
    "bevy_society",
    "bevy_speech_bubbles",
    "bevy_stigmergy",
];

/// The family crates this build was compiled with: `bevy_gooey` always (the editor's UI is gooey), and one entry per enabled cargo feature for the rest.
pub fn built() -> Vec<&'static str> {
    let mut on = Vec::new();
    if cfg!(feature = "bevy_carnage") {
        on.push("bevy_carnage");
    }
    if cfg!(feature = "bevy_elements") {
        on.push("bevy_elements");
    }
    if cfg!(feature = "bevy_flora") {
        on.push("bevy_flora");
    }
    on.push("bevy_gooey");
    if cfg!(feature = "bevy_lights") {
        on.push("bevy_lights");
    }
    if cfg!(feature = "bevy_orca") {
        on.push("bevy_orca");
    }
    if cfg!(feature = "bevy_siren") {
        on.push("bevy_siren");
    }
    if cfg!(feature = "bevy_society") {
        on.push("bevy_society");
    }
    if cfg!(feature = "bevy_speech_bubbles") {
        on.push("bevy_speech_bubbles");
    }
    if cfg!(feature = "bevy_stigmergy") {
        on.push("bevy_stigmergy");
    }
    on
}

/// **What is wrong with the project's `family.ron` for this build**: an unreadable file, an unknown
/// crate, or a crate whose feature is off. Empty when the project lists nothing or has no file.
pub fn problems_with(root: &std::path::Path, built: &[&str]) -> Vec<String> {
    match Family::load(root) {
        Ok(family) => family.problems(KNOWN, built),
        Err(e) => vec![e],
    }
}

/// Reports the project's family problems once per opened project root, on the status line where the
/// author looks.
pub fn report_problems(
    project: Option<Res<Project>>,
    mut state: ResMut<EditorState>,
    mut told: Local<Option<PathBuf>>,
) {
    let Some(project) = project else { return };
    if told.as_deref() == Some(project.root.as_path()) {
        return;
    }
    *told = Some(project.root.clone());
    for p in problems_with(&project.root, &built()) {
        state.status.problem(p);
    }
}

/// **The settings files in force** (#96): `(crate, type)` → the text the running resource was last read
/// from. A data file (a dialogue script, a society's definitions) is listed too once it is valid RON; its
/// own panel reads it. A file that does not read is not here, and the previous settings stay.
#[derive(Resource, Default, Debug)]
pub struct FamilyFiles {
    pub applied: BTreeMap<SettingsKey, String>,
}

impl FamilyFiles {
    /// Whether any of `krate`'s settings files is in force.
    pub fn has_crate(&self, krate: &str) -> bool {
        self.applied.keys().any(|(k, _)| k == krate)
    }
}

/// What [`reload_settings`] decided to apply, for [`apply_pending_settings`] to write with the world in
/// hand: `(crate, type, Some(text))` applies a file, `None` puts the type's default back.
#[derive(Resource, Default, Debug)]
pub struct PendingSettings(pub Vec<(String, String, Option<String>)>);

/// The set [`reload_settings`] and [`apply_pending_settings`] run in, which a crate's own system orders
/// after. A set rather than the system itself, because ordering against a system type panics once a
/// schedule holds two of it.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Reloaded;

/// Whether `(krate, ident)` is a settings type this build registers, which is what the editor applies.
/// Any other file under `family/<crate>/` is a crate's data, read by its own panel.
fn is_settings(registry: &TypeRegistry, krate: &str, ident: &str) -> bool {
    built().contains(&krate) && bevy_blueprint::settings::settings_type(registry, krate, ident).is_ok()
}

/// **What the crate's own type makes of a settings file** (#200): a settings type this build registers is
/// read with it by reflection, so a refusal names the file and the field it is wrong at
/// (`family/bevy_elements/WorldClock.ron (date.month)`), and the words returned with it say what stays.
/// The editor holds no schema of its own. Any other file is held to being RON, which [`FamilyWatch`]
/// has already seen.
fn read_as(registry: &TypeRegistry, krate: &str, ident: &str, text: &str) -> Result<(), (Refusal, &'static str)> {
    if !is_settings(registry, krate, ident) {
        return Ok(());
    }
    bevy_blueprint::settings::read_settings(registry, krate, ident, text)
        .map(drop)
        .map_err(|e| (Refusal { kind: "family", file: e.file, field: e.field, reason: e.reason }, "the previous settings stay"))
}

/// The `(crate, type)` a verdict's file names, `family/<crate>/<type>.ron`.
fn key_of(file: &str) -> Option<SettingsKey> {
    let rest = file.strip_prefix("family/")?.strip_suffix(".ron")?;
    let (krate, ident) = rest.split_once('/')?;
    Some((krate.to_owned(), ident.to_owned()))
}

/// How often a refused settings file is compared with the disk, seconds.
const SETTLE_REFUSED_SECS: f64 = 0.5;

/// What [`reload_settings`] holds between frames. Opaque: a system's `Local` has to be nameable by
/// the system, nothing else uses it.
pub struct Held {
    root: PathBuf,
    watch: FamilyWatch,
    /// The clock reading when this watch was made, in seconds on [`Time<Real>`](bevy::time::Real).
    /// Every poll subtracts it, so `settled_at` and the watch's own intervals are measured from the
    /// same origin — and a test that drives `TimeUpdateStrategy::ManualDuration` drives both (#279).
    made_at: f64,
    /// When refused files were last compared with the disk, in seconds since `made_at`.
    settled_at: f64,
    /// The text of each refused file as last decided, so the same bytes are decided once: the watch
    /// says a file is invalid again whenever another settings file changes.
    bad: BTreeMap<SettingsKey, String>,
}

/// **The settings files that were not valid RON when the project opened**, with their text and why.
/// [`FamilyWatch`] keeps only the ones that were, and says nothing about the rest until one of them
/// changes, so a file broken before the editor started would go unnamed.
fn unread_at_launch(root: &std::path::Path, good: &BTreeMap<SettingsKey, String>) -> Vec<(SettingsKey, String, String)> {
    let mut out = Vec::new();
    for (key, text) in read_settings_files(&blueprint_model::family::settings_dir(root)) {
        if good.contains_key(&key) {
            continue;
        }
        if let Err(e) = ron::from_str::<ron::Value>(&text) {
            out.push((key, text, e.to_string()));
        }
    }
    out
}

/// **Bring the refused files up to date with the disk.** [`FamilyWatch`] reports a change from the
/// settings in force, so a file the author puts back to exactly those (undoing a typo) is no change
/// to it, and a file that never read and is then deleted was never in force to be removed. Neither
/// would ever leave the verdicts, so a refused file is looked at twice a second: back to the settings
/// in force it is `applied` again, gone it is forgotten, and anything else is the watch's to report.
/// A type that refused a file is the watch's too: it holds the text it took (valid RON) and the
/// settings in force are the last the type read, so the file's next edit is a change to it; the one thing
/// it cannot say is a return to that text after an unreadable edit in between, which is said here.
#[allow(clippy::too_many_arguments)]
fn settle_refused(
    registry: &TypeRegistry,
    root: &std::path::Path,
    good: &BTreeMap<SettingsKey, String>,
    bad: &mut BTreeMap<SettingsKey, String>,
    current: &BTreeMap<SettingsKey, String>,
    state: &mut EditorState,
    reloads: &mut crate::live::EditorReloads,
) {
    let refused: Vec<String> = reloads
        .kinds()
        .iter()
        .filter(|k| k.kind == "family" && k.outcome == blueprint_model::live::Outcome::Refused)
        .map(|k| k.file.clone())
        .collect();
    let mut decided = Vec::new();
    for file in refused {
        let Some(key) = key_of(&file) else { continue };
        match std::fs::read_to_string(root.join(&file)) {
            Err(_) => {
                bad.remove(&key);
                reloads.forget_file("family", &file);
            }
            // The settings in force are not what the watch holds: the type refused its text.
            Ok(text) if current.get(&key) != good.get(&key) => {
                if current.get(&key) == Some(&text)
                    && bad.get(&key) != Some(&text)
                    && let Err((refusal, stays)) = read_as(registry, &key.0, &key.1, &text)
                {
                    state.status.problem(format!("{}; {stays}", refusal.sentence()));
                    decided.push(KindVerdict::refused("family", refusal.file, refusal.field, refusal.reason));
                    bad.insert(key, text);
                }
            }
            Ok(text) if good.get(&key) == Some(&text) => {
                bad.remove(&key);
                decided.push(KindVerdict::applied("family", file));
            }
            Ok(_) => {}
        }
    }
    if !decided.is_empty() {
        reloads.decide(decided);
    }
}

/// **Follow the settings files, and say what became of each edit** (#96, #200).
///
/// A file that reads replaces its type's resource (by reflection, in [`apply_pending_settings`]); one
/// that is not valid RON is refused and the last good settings stay ([`FamilyWatch`] holds them). A file
/// that is RON but not its type (`read_as`) is refused the same way, naming the file and the field, and
/// is never applied. Each poll's changes are one decision in [`crate::live::EditorReloads`], kind
/// `family`, so `factory_state.reloads` names the file and why, once per edit; a file that is gone is
/// no longer listed, and one that was already broken when the project opened is named from the start.
#[allow(clippy::too_many_arguments)]
pub fn reload_settings(
    project: Option<Res<Project>>,
    // **The clock is Bevy's, not a `std::time::Instant` this system owns** (#279): `Time<Real>` is the
    // wall clock by default and `TimeUpdateStrategy::ManualDuration` lets a test step it.
    real: Res<Time<Real>>,
    registry: Res<AppTypeRegistry>,
    mut files: ResMut<FamilyFiles>,
    mut pending: ResMut<PendingSettings>,
    mut state: ResMut<EditorState>,
    mut reloads: Option<ResMut<crate::live::EditorReloads>>,
    mut held: Local<Option<Held>>,
) {
    let Some(project) = project else { return };
    let registry = registry.read();
    if held.as_ref().is_none_or(|h| h.root != project.root) {
        let fresh = FamilyWatch::new(&project.root);
        // Another project's settings are not this one's: every type that was applied goes back to its
        // default before this project's files are read.
        for (krate, ident) in std::mem::take(&mut files.applied).into_keys() {
            if is_settings(&registry, &krate, &ident) {
                pending.0.push((krate, ident, None));
            }
        }
        let mut verdicts = Vec::new();
        let mut bad = BTreeMap::new();
        for ((krate, ident), text) in fresh.current() {
            match read_as(&registry, krate, ident, text) {
                Ok(()) => {
                    files.applied.insert((krate.clone(), ident.clone()), text.clone());
                    if is_settings(&registry, krate, ident) {
                        pending.0.push((krate.clone(), ident.clone(), Some(text.clone())));
                    }
                    verdicts.push(KindVerdict::applied("family", settings_file(krate, ident)));
                }
                Err((refusal, _)) => {
                    state.status.problem(format!("{}; its default applies", refusal.sentence()));
                    verdicts.push(KindVerdict::refused("family", refusal.file, refusal.field, refusal.reason));
                    bad.insert((krate.clone(), ident.clone()), text.clone());
                }
            }
        }
        for ((krate, ident), text, error) in unread_at_launch(&project.root, fresh.current()) {
            state.status.problem(format!("{} is not valid RON, so its default applies: {error}", settings_file(&krate, &ident)));
            let field = read_as(&registry, &krate, &ident, &text).err().and_then(|(r, _)| r.field);
            verdicts.push(KindVerdict::refused("family", settings_file(&krate, &ident), field, error));
            bad.insert((krate, ident), text);
        }
        if let Some(reloads) = reloads.as_deref_mut() {
            reloads.forget(&["family"]);
            if !verdicts.is_empty() {
                reloads.decide(verdicts);
            }
        }
        *held = Some(Held { root: project.root.clone(), watch: fresh, made_at: real.elapsed_secs_f64(), settled_at: 0.0, bad });
    }
    let Some(held) = held.as_mut() else { return };
    let now = real.elapsed_secs_f64() - held.made_at;
    let (mut decided, mut gone) = (Vec::new(), Vec::new());
    for change in held.watch.poll(now) {
        match change {
            SettingsChange::Reloaded { krate, ident, text } => match read_as(&registry, &krate, &ident, &text) {
                Ok(()) => {
                    state.status.note(format!("{} reloaded", settings_file(&krate, &ident)));
                    held.bad.remove(&(krate.clone(), ident.clone()));
                    decided.push(KindVerdict::applied("family", settings_file(&krate, &ident)));
                    if is_settings(&registry, &krate, &ident) {
                        pending.0.push((krate.clone(), ident.clone(), Some(text.clone())));
                    }
                    files.applied.insert((krate, ident), text);
                }
                Err((refusal, stays)) => {
                    state.status.problem(format!("{}; {stays}", refusal.sentence()));
                    decided.push(KindVerdict::refused("family", refusal.file, refusal.field, refusal.reason));
                    held.bad.insert((krate, ident), text);
                }
            },
            SettingsChange::Invalid { krate, ident, error } => {
                // The same bytes are decided once, however often the watch says so.
                let key = (krate.clone(), ident.clone());
                let text = std::fs::read_to_string(settings_path(&held.root, &krate, &ident)).ok();
                if text.is_some() && held.bad.get(&key) == text.as_ref() {
                    continue;
                }
                state.status.problem(format!("{} is not valid RON, so the previous settings stay: {error}", settings_file(&krate, &ident)));
                let field = text.as_deref().and_then(|t| read_as(&registry, &krate, &ident, t).err()).and_then(|(r, _)| r.field);
                decided.push(KindVerdict::refused("family", settings_file(&krate, &ident), field, error));
                if let Some(text) = text {
                    held.bad.insert(key, text);
                }
            }
            SettingsChange::Removed { krate, ident } => {
                state.status.note(format!("{} removed; its default applies", settings_file(&krate, &ident)));
                let key = (krate.clone(), ident.clone());
                held.bad.remove(&key);
                gone.push(settings_file(&krate, &ident));
                files.applied.remove(&key);
                if is_settings(&registry, &krate, &ident) {
                    pending.0.push((krate, ident, None));
                }
            }
        }
    }
    if let Some(reloads) = reloads.as_deref_mut() {
        for file in &gone {
            reloads.forget_file("family", file);
        }
        if !decided.is_empty() {
            reloads.decide(decided);
        }
        if now - held.settled_at >= SETTLE_REFUSED_SECS {
            held.settled_at = now;
            settle_refused(&registry, &held.root, &files.applied, &mut held.bad, held.watch.current(), &mut state, reloads);
        }
    }
}

/// **Applies what [`reload_settings`] decided**, by reflection, with the world in hand: the same
/// `bevy_blueprint::settings` path a game's live reload takes, so the preview and the game read a file
/// the same way. A file the type refuses at this point (it read a moment ago) is a problem and the
/// previous settings stay.
pub fn apply_pending_settings(world: &mut World) {
    let pending = std::mem::take(&mut world.resource_mut::<PendingSettings>().0);
    for (krate, ident, text) in pending {
        #[cfg(feature = "bevy_elements")]
        let before = crate::environment::clock_before(world, &krate, &ident);
        let result = match &text {
            Some(text) => bevy_blueprint::settings::apply_settings_text(world, &krate, &ident, text),
            None => bevy_blueprint::settings::reset_settings(world, &krate, &ident),
        };
        if let Err(e) = result {
            world.resource_mut::<EditorState>().status.problem(format!("{e}; the previous settings stay"));
            continue;
        }
        #[cfg(feature = "bevy_elements")]
        crate::environment::ramp_clock_edit(world, before);
        if krate == crate::ui_skin::CRATE {
            crate::ui_skin::report_skin_problems(world);
        }
    }
}
