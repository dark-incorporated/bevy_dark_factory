//! **The bench notices a re-export instead of waiting to be pointed at it.**
//!
//! Four pieces, all frame-driven ECS in the crate's deliberate no-threads style:
//!
//! - `RigWatch` follows the loaded rigs' GLBs through the project index (`crate::project_index`,
//!   #501): it reads the index's stamp (size and modification time) of each one when the index's
//!   generation moves, and a changed stamp queues its rigs. The exporter-mid-write debounce, which was
//!   "the mtime seen equal on two consecutive polls", is the index's: it publishes a stamp once it has
//!   been steady for `DEBOUNCE`, or after `MAX_HOLD` of a file that never settles, so a file rewritten
//!   without a pause for longer than that is measured part-way and again when it ends. A file the
//!   project's `.gitignore` names is not listed, so it is not followed.
//! - `MeasureQueue` + `step_measure_queue` re-measure **one rig per frame**, the `thumbs::bake`
//!   shape — a re-export that touches sixteen GLBs costs sixteen frames, never one stalled one.
//!   Selection, the watcher, and check-all all feed the same queue: one measurement path.
//! - [`BenchReports`] holds the results keyed by rig NAME, and [`BenchGeneration`] bumps only when
//!   a stored report actually changed — the `ThumbGeneration` idiom, because `resource_changed` on
//!   a resource a system takes as `ResMut` fires every frame.
//! - `sync_rigs_reload` takes an edit to `rigs.yaml` itself the moment it is made: a valid edit is
//!   `applied` and measured again, a broken one is `refused`, in `blueprint_state.reloads`.
//!
//! The stale count is painted onto the tab strip's `ANIM` label in place, so it survives tab
//! switches: a fact about the project does not stop being true because a different tab is open.

use std::collections::{HashMap, HashSet, VecDeque};

use bevy::prelude::*;

use blueprint_model::rig_check::{Finding, Level, Staleness};

use crate::anim_tab::BenchState;
use crate::project_index::{FileStamp, ProjectIndex, Snapshot as Listing};
use crate::tiles::Mode;

/// The folder the rigs' meshes are named under, as the project index names it.
const ASSETS_REL: &str = "assets";

/// `rigs.yaml` as the project index names it.
const RIGS_REL: &str = "assets/blueprint/rigs.yaml";

/// The stamp the project index holds for `rel`: `None` when it lists no such file.
fn stamp_in(listing: &Listing, rel: &str) -> Option<FileStamp> {
    listing.files.binary_search_by(|(path, _)| path.as_str().cmp(rel)).ok().map(|at| listing.files[at].1)
}

/// **Say `message` once**, so a failure that lasts is one line and not one a frame.
fn say_once(said: &mut Option<String>, message: String) {
    if said.as_deref() != Some(message.as_str()) {
        warn!("{message}");
        *said = Some(message);
    }
}

/// **Say once that the index cannot answer.**
fn refuse(said: &mut Option<String>, who: &str, why: String) {
    say_once(said, format!("{who}: the project index has no listing ({why}), so nothing is read"));
}

/// The watcher's memory: what the rigs' meshes looked like when last taken.
#[derive(Resource, Default)]
pub(crate) struct RigWatch {
    /// The index generation last looked at.
    generation: u64,
    /// The stamp each mesh had when it was last taken (or the first one ever seen), per mesh path.
    seen: HashMap<String, Option<FileStamp>>,
    /// Why the index could not answer, as last said.
    refused: Option<String>,
    /// The meshes the index does not list, said once each.
    unlisted: HashSet<String>,
}

/// **Whether a file's stamp moved since it was last taken**, remembering the new one. The first sight
/// of a file is its baseline, not a change. (A change that is undone before the index publishes it is
/// no change at all, and the index says so by publishing nothing: it is not seen here.)
pub(crate) fn moved(known: &mut Option<FileStamp>, observed: FileStamp) -> bool {
    known.replace(observed).is_some_and(|before| before != observed)
}

/// Rig names awaiting a measure. Fed by selection (front), the watcher, and check-all (back).
#[derive(Resource, Default)]
pub(crate) struct MeasureQueue(VecDeque<String>);

impl MeasureQueue {
    pub(crate) fn push_back_unique(&mut self, name: &str) {
        if !self.0.iter().any(|n| n == name) {
            self.0.push_back(name.to_owned());
        }
    }

    /// The selected rig jumps the line — it is the one on screen.
    pub(crate) fn push_front_unique(&mut self, name: &str) {
        self.0.retain(|n| n != name);
        self.0.push_front(name.to_owned());
    }
}

/// Everything one measuring pass said about one rig — what the pane renders.
/// Serde because `anim_cache` persists reports between sessions.
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RigReport {
    /// The file fingerprint the findings describe. `None` when the GLB could not be read.
    pub fingerprint: Option<u64>,
    /// Whether the manifest's recorded measurement still describes the asset.
    pub staleness: Option<Staleness>,
    pub findings: Vec<Finding>,
    /// Asset clip names by clip index.
    pub clip_names: Vec<Option<String>>,
    /// The provenance stamp's date, when there is one.
    pub date: Option<String>,
    /// The clip-list diff, when stale — the diagnosis lines.
    pub diff: Vec<String>,
    /// Phase-gridded curves per gait slot `(slot index, curves)` — the plots' input, a by-product
    /// of the FK the checks already ran. Empty for a rig with no gaits (or no contact joint).
    pub curves: Vec<(usize, blueprint_model::clips::GaitCurves)>,
    /// The measured numbers per gait slot, straight from `check_rig` — what the ghost stages, the
    /// plots overlay, and the measured sub-line prints. Used to be dropped on the floor here while
    /// `adopt` re-measured from scratch.
    pub slots: Vec<blueprint_model::rig_check::SlotMeasure>,
    /// The skate arithmetic per gait slot — empty when the rig declares no `drive_speed`.
    pub skates: Vec<blueprint_model::rig_check::SkateReport>,
    /// The most severe finding level, for summaries and the badge.
    pub worst: Level,
}

/// The measured state of the project, keyed by rig name. Name-keyed so a report can never describe
/// a different rig than the pane thinks it does — the index-raced flash the old per-selection cache
/// had to guard against cannot be expressed.
#[derive(Resource, Default)]
pub struct BenchReports {
    pub by_rig: HashMap<String, RigReport>,
}

impl BenchReports {
    /// How many rigs' recorded measurements no longer describe their assets.
    pub fn stale_count(&self) -> usize {
        self.by_rig
            .values()
            .filter(|r| r.staleness == Some(Staleness::Stale))
            .count()
    }
}

/// Bumped only when a stored report actually changed.
#[derive(Resource, Default)]
pub struct BenchGeneration(pub u32);

/// **One rig, measured.** The policy is `rig_check::check_rig` — the same call CI makes.
pub(crate) fn measure_rig(root: &std::path::Path, rig: &blueprint_model::rigs::Rig) -> RigReport {
    let path = root.join("assets").join(&rig.mesh);
    let (glb, hash) = match blueprint_model::glb::Glb::open_fingerprinted(&path) {
        Ok(pair) => pair,
        Err(e) => {
            let findings = vec![Finding::bad(format!("cannot read {}: {e}", rig.mesh))];
            let worst = blueprint_model::rig_check::worst(&findings);
            return RigReport {
                fingerprint: None,
                staleness: None,
                findings,
                clip_names: Vec::new(),
                date: rig.provenance.as_ref().map(|p| p.date.clone()),
                diff: Vec::new(),
                curves: Vec::new(),
                slots: Vec::new(),
                skates: Vec::new(),
                worst,
            };
        }
    };
    let clips = blueprint_model::clips::clips(&glb);
    let staleness = blueprint_model::rig_check::staleness(rig, hash);
    let diff = match (staleness, &rig.provenance) {
        (Staleness::Stale, Some(p)) => {
            blueprint_model::rig_check::clip_list_diff(&p.clip_names, &clips)
        }
        _ => Vec::new(),
    };
    let report =
        blueprint_model::rig_check::check_rig(&glb, rig, staleness == Staleness::Current);
    let worst = blueprint_model::rig_check::worst(&report.findings);
    // The plots' curves — same FK, same anchors, gathered while the file is open.
    let mut curves = Vec::new();
    if rig.has_gaits() {
        if let Some(foot) = blueprint_model::clips::node_index(&glb, rig.contact_joint()) {
            let root = blueprint_model::clips::node_index(&glb, rig.root_node());
            for (i, slot) in rig.slots.iter().enumerate() {
                if !matches!(slot.playback, blueprint_model::rigs::Playback::Gait { .. }) {
                    continue;
                }
                if let Some(c) =
                    blueprint_model::clips::gait_curves(&glb, slot.clip, foot, root, rig.contact_eps)
                {
                    curves.push((i, c));
                }
            }
        }
    } else {
        // A gait-less rig still deserves curves — height and speed of its most foot-like joint,
        // with no contact claim. One ordered selection rule: the conventional contact joint when
        // the asset has it, else the best measured candidate, else no curve.
        for (i, slot) in rig.slots.iter().enumerate() {
            if !matches!(slot.playback, blueprint_model::rigs::Playback::Free { .. }) {
                continue;
            }
            let joint = blueprint_model::clips::node_index(&glb, rig.contact_joint()).or_else(|| {
                blueprint_model::clips::contact_candidates(&glb, slot.clip)
                    .first()
                    .and_then(|(name, _)| blueprint_model::clips::node_index(&glb, name))
            });
            let Some(joint) = joint else { continue };
            if let Some(c) = blueprint_model::clips::joint_curves(&glb, slot.clip, joint) {
                curves.push((i, c));
            }
        }
    }
    RigReport {
        fingerprint: Some(hash),
        staleness: Some(staleness),
        findings: report.findings,
        clip_names: clips.into_iter().map(|c| c.name).collect(),
        date: rig.provenance.as_ref().map(|p| p.date.clone()),
        diff,
        curves,
        slots: report.slots,
        skates: report.skates,
        worst,
    }
}

/// Forget everything measured — the manifest's text just changed under the reports (an adopt, an
/// undo), so every one of them describes a file that no longer exists. The selected rig re-enters
/// the queue on the next `queue_selected` pass, and the badge recounts to zero meanwhile.
pub(crate) fn invalidate(reports: &mut BenchReports, generation: &mut BenchGeneration) {
    if !reports.by_rig.is_empty() {
        reports.by_rig.clear();
        generation.0 = generation.0.wrapping_add(1);
    }
}

/// Follow the loaded rigs' GLBs through the project index. Only rigs already loaded — the tab's lazy
/// load stands. A frame where neither the index's generation nor the tab moved looks at nothing; the
/// tab moving is `bench.is_changed()`, which any change to the tab's state sets (a selection among
/// them), and is a look because a rig that came with an edit of the manifest has a mesh nobody has
/// taken a stamp of yet.
pub(crate) fn follow_the_rig_files(
    index: Res<ProjectIndex>,
    bench: Option<Res<BenchState>>,
    mut watch: ResMut<RigWatch>,
    mut queue: ResMut<MeasureQueue>,
) {
    let Some(bench) = bench else {
        return;
    };
    let Some(rigs) = &bench.rigs else {
        return;
    };
    // (See the note above: the tab changing is a look as well.)
    if watch.generation == index.generation() && !bench.is_changed() {
        return;
    }
    let Some(listing) = index.snapshot() else {
        // No listing yet is the index still walking; no listing with a failure is the root that cannot be read.
        if let Some(why) = index.failure() {
            refuse(&mut watch.refused, "animation watch", why);
        }
        return;
    };
    watch.refused = None;
    watch.generation = listing.generation;
    // Meshes first, so two rigs sharing a GLB are one lookup — and both re-measure when it changes.
    let mut by_mesh: HashMap<&str, Vec<&str>> = HashMap::new();
    for (name, rig) in &rigs.rigs {
        by_mesh.entry(rig.mesh.as_str()).or_default().push(name);
    }
    for (mesh, names) in by_mesh {
        // A file the index does not list is missing or named by the project's `.gitignore`: it cannot be
        // followed, which is said once. The measure pass says "cannot read" loudly for a file that is
        // missing, and runs as soon as the file reappears with a new stamp.
        let rel = format!("{ASSETS_REL}/{mesh}");
        let Some(observed) = stamp_in(&listing, &rel) else {
            if watch.unlisted.insert(mesh.to_owned()) {
                warn!("animation watch: {rel} is not in the project index (it is missing, or the project's .gitignore names it), so changes to it are not followed");
            }
            continue;
        };
        watch.unlisted.remove(mesh);
        if moved(watch.seen.entry(mesh.to_owned()).or_default(), observed) {
            for name in names {
                queue.push_back_unique(name);
            }
        }
    }
}

/// Keep the selected rig measured. Cheap: a map lookup per frame until the report exists.
pub(crate) fn queue_selected(
    bench: Option<Res<BenchState>>,
    reports: Option<Res<BenchReports>>,
    mut queue: ResMut<MeasureQueue>,
) {
    let (Some(bench), Some(reports)) = (bench, reports) else {
        return;
    };
    let names = bench.names();
    let Some(name) = names.get(bench.selected) else {
        return;
    };
    if reports.by_rig.contains_key(*name) {
        return;
    }
    queue.push_front_unique(name);
}

/// **One rig per frame** — the `thumbs::bake` shape. The FK over every keyframe of every clip is
/// real work, and sixteen of it in one frame is a stalled editor.
pub(crate) fn step_measure_queue(
    project: Option<Res<crate::project::Project>>,
    bench: Option<Res<BenchState>>,
    mut queue: ResMut<MeasureQueue>,
    mut reports: ResMut<BenchReports>,
    mut generation: ResMut<BenchGeneration>,
) {
    let (Some(project), Some(bench)) = (project, bench) else {
        return;
    };
    let Some(name) = queue.0.pop_front() else {
        return;
    };
    // A rig that vanished from the manifest between enqueue and now is simply no longer measurable.
    let Some(rig) = bench.rigs.as_ref().and_then(|r| r.get(&name)) else {
        return;
    };
    let report = measure_rig(&project.root, rig);
    if reports.by_rig.get(&name) != Some(&report) {
        reports.by_rig.insert(name, report);
        generation.0 = generation.0.wrapping_add(1);
    }
}

/// Repaint the ANIM tab's badge with the stale count: nothing, or `2 STALE`.
///
/// In place, never rebuilt — the strip is shared chrome — and on the strip precisely so the fact
/// survives tab switches: the developer who re-exported from Blender is on no particular tab when
/// the bench notices.
///
/// The badge is its own text child, not a suffix on the label: `style_tabs` owns every
/// [`crate::tiles::TabLabel`]'s colour per frame, so an `Ink::danger` written into the label is stomped
/// a frame later — the word rendered in the tab's ordinary grey, which the pane's own doc calls
/// the one word here allowed to shout, whispering. See [`crate::tiles::TabBadge`] for the colour
/// argument (Lewandowska et al. 2022: persistent peripheral signal at medium intensity).
pub(crate) fn paint_stale_badge(
    reports: Option<Res<BenchReports>>,
    tabs: Query<(&crate::tiles::Tab, &Children)>,
    mut badges: Query<&mut Text, With<crate::tiles::TabBadge>>,
) {
    let Some(reports) = reports else {
        return;
    };
    let stale = reports.stale_count();
    let want = if stale == 0 {
        String::new()
    } else {
        format!("{stale} STALE")
    };
    for (tab, children) in &tabs {
        if tab.0 != Mode::Anim {
            continue;
        }
        for child in children {
            if let Ok(mut text) = badges.get_mut(*child)
                && text.0 != want
            {
                text.0 = want.clone();
            }
        }
    }
}

/// What [`sync_rigs_reload`] last saw of `rigs.yaml`, and what it last decided.
#[derive(Default)]
pub(crate) struct RigsSeen {
    /// The index's stamp of `rigs.yaml` when it was last read (`Some(None)`: the index listed none), and
    /// `None` before the first read since the tab loaded the manifest. It is set even when the read
    /// failed, so a file that cannot be read is tried again when the index next moves and not every frame.
    looked: Option<Option<FileStamp>>,
    /// The index generation last looked at.
    generation: u64,
    /// Why the index could not answer, as last said.
    refused: Option<String>,
    /// That the index does not list `rigs.yaml` has been said, so it is one line however many looks follow.
    said_read: Option<String>,
    /// `(what the disk held, what the tab held)` as last decided to refuse or to wait on, so one bad
    /// edit is one verdict however many frames pass and the same bytes are never decided twice. `None`
    /// while the two agree.
    decided: Option<(Option<String>, Option<String>)>,
}

/// The resources an edit of the manifest touches besides the tab: what is measured and what is queued
/// to be, and the adopt exclusions that described the text the edit replaces. Bundled to keep
/// [`sync_rigs_reload`] under the parameter cap.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct Measuring<'w> {
    exclude: ResMut<'w, crate::anim_tab::AdoptExclude>,
    reports: ResMut<'w, BenchReports>,
    generation: ResMut<'w, BenchGeneration>,
    queue: ResMut<'w, MeasureQueue>,
}

/// **Take an edit of `rigs.yaml` the moment it is made, and say what became of it** (#200, H3; #202 made the
/// kind `Live`).
///
/// The tab reads the manifest when it loads (the first entry to the tab, and again after a door change
/// resets the bench) and holds the text it read (`BenchState`). A file that reads is taken at once: the tab
/// holds it (the text it replaces is one step of the tab's undo), every report is dropped and every rig
/// queued to be measured again, the rig the author was on stays selected by name, and the kind's verdict in
/// [`crate::live::EditorReloads`] is `applied`. A file that does not read is `refused` with the file and the
/// field and the tab keeps what it holds; a file that is gone is `needs_restart`, because there is no
/// manifest to take in its place. Each distinct edit is decided once. The tab's own writes (`commit_text`)
/// update the held text with the file, so they agree and say nothing; a refused or missing file put back
/// as the tab holds it clears that verdict, and so does a door change, which drops what was held. An
/// `applied` verdict stays until the next edit replaces it: it is still true of the file.
pub(crate) fn sync_rigs_reload(
    index: Res<ProjectIndex>,
    project: Option<Res<crate::project::Project>>,
    mut bench: ResMut<BenchState>,
    mut measuring: Measuring,
    mut seen: Local<RigsSeen>,
    mut reloads: Option<ResMut<crate::live::EditorReloads>>,
    mut state: Option<ResMut<crate::editor::EditorState>>,
) {
    use blueprint_model::live::{KindVerdict, Outcome};
    use blueprint_model::reload_check::{self, Check};
    use blueprint_model::rigs::Rigs;

    const FILE: &str = "rigs.yaml";
    let Some(project) = project else { return };
    // Nothing to compare with until the tab has read the manifest, and a door change drops what it
    // held (`BenchState` is the door's): a verdict about those rigs is not true of the next load, which
    // reads the file as it is.
    if !bench.loaded {
        if seen.decided.take().is_some()
            && let Some(reloads) = reloads.as_deref_mut()
        {
            reloads.forget(&["rigs"]);
        }
        seen.looked = None;
        return;
    }
    // **Nothing is read until the index says `rigs.yaml` moved** (its stamp), and the tab's first look
    // after it loaded the manifest reads once, so an edit between that load and the index's first
    // listing, which publishes no message, is not lost.
    if seen.looked.is_some() && seen.generation == index.generation() {
        return;
    }
    let Some(listing) = index.snapshot() else {
        if let Some(why) = index.failure() {
            refuse(&mut seen.refused, "animation tab", why);
        }
        return;
    };
    seen.refused = None;
    seen.generation = listing.generation;
    let stamp = stamp_in(&listing, RIGS_REL);
    if seen.looked == Some(stamp) {
        return;
    }
    seen.looked = Some(stamp);
    let path = project.root.join(RIGS_REL);
    // The file is read whether or not the index lists it: the stamp says when to read, not whether there is a file.
    let disk = match std::fs::read_to_string(&path) {
        Ok(text) => {
            if stamp.is_none() {
                say_once(
                    &mut seen.said_read,
                    format!("animation tab: {RIGS_REL} is not in the project index (the project's .gitignore names it?), so later edits to it are not followed"),
                );
            } else {
                seen.said_read = None;
            }
            Some(text)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            // Neither taken nor called gone: said at each look, and a look is one the index's stamp asked for, so a file that
            // stays unreadable is read again when the index next moves and not on every frame (`looked` is set above).
            warn!("animation tab: {} cannot be read: {e}; the rigs it holds stay", path.display());
            return;
        }
    };
    if disk == bench.text {
        if seen.decided.take().is_some()
            && let Some(reloads) = reloads.as_deref_mut()
        {
            reloads.forget(&["rigs"]);
        }
        return;
    }
    let pair = (disk, bench.text.clone());
    if seen.decided.as_ref() == Some(&pair) {
        return;
    }
    // What the tab holds decides what is true and what to say: rigs, or nothing because the file did
    // not read when the tab loaded.
    let held = pair.1.is_some();
    let mut taken = false;
    let verdict = match &pair.0 {
        None => KindVerdict::needs_restart(
            "rigs",
            FILE,
            "the file is gone, and the animation tab still holds the rigs it read: it has no manifest to take in their place, so restore the file or restart the editor to drop them",
        ),
        Some(text) => match Rigs::parse(text) {
            Ok(rigs) => {
                let count = rigs.rigs.len();
                bench.take_edit(rigs, text.clone());
                measuring.exclude.clear();
                invalidate(&mut measuring.reports, &mut measuring.generation);
                let names: Vec<String> = bench.names().iter().map(|n| (*n).to_owned()).collect();
                for name in &names {
                    measuring.queue.push_back_unique(name);
                }
                if let Some(state) = state.as_deref_mut() {
                    state.status.note(if held {
                        format!("{FILE}: the animation tab took the edit ({count} rig(s)) and is measuring them again")
                    } else {
                        format!("{FILE}: the animation tab loaded no rigs, because the file did not read then, and holds the {count} rig(s) of this edit")
                    });
                }
                taken = true;
                KindVerdict::applied("rigs", FILE)
            }
            Err(reason) => {
                let field = blueprint_model::reload::kind_by_id("rigs").and_then(|k| match reload_check::check(k, FILE, text) {
                    Check::Refused(r) => r.field,
                    _ => None,
                });
                KindVerdict::refused("rigs", FILE, field, reason)
            }
        },
    };
    if let Some(state) = state.as_deref_mut() {
        match verdict.outcome {
            Outcome::Refused => {
                let kept = if held { "the rigs already loaded stay" } else { "no rigs are loaded from it" };
                state.status.raise("", verdict.problem(kept));
            }
            Outcome::NeedsRestart => state.status.note(format!("{}: {}", verdict.file, verdict.reason)),
            Outcome::Applied => {}
        }
    }
    if let Some(reloads) = reloads.as_deref_mut() {
        reloads.forget(&["rigs"]);
        reloads.decide(vec![verdict]);
    }
    // A taken edit leaves disk and tab agreeing, so nothing is waiting to be decided again, and its
    // `applied` verdict stays true until the next edit replaces it.
    seen.decided = (!taken).then_some(pair);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    fn stamp(secs: u64) -> FileStamp {
        FileStamp { len: 7, modified: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(secs)) }
    }

    #[test]
    fn a_changed_stamp_is_taken_once_and_becomes_the_new_baseline() {
        let mut known = None;
        assert!(!moved(&mut known, stamp(100)), "first sight is baseline, not change");
        assert!(!moved(&mut known, stamp(100)), "unchanged");
        assert!(moved(&mut known, stamp(200)), "the index published a changed stamp: re-measure");
        assert!(!moved(&mut known, stamp(200)), "and it is the new baseline");
    }

    #[test]
    fn a_stamp_differing_in_size_alone_or_older_than_the_known_one_is_a_change() {
        let mut known = None;
        assert!(!moved(&mut known, stamp(100)));
        let longer = FileStamp { len: 8, ..stamp(100) };
        assert!(moved(&mut known, longer), "the same modification time with another size is a change");
        assert!(moved(&mut known, stamp(50)), "a stamp older than the known one is a change: the index published it");
        assert!(!moved(&mut known, stamp(50)), "and then it is the baseline");
    }

    #[test]
    fn the_queue_deduplicates_and_the_front_wins() {
        let mut q = MeasureQueue::default();
        q.push_back_unique("a");
        q.push_back_unique("b");
        q.push_back_unique("a");
        assert_eq!(q.0.len(), 2);
        q.push_front_unique("b");
        assert_eq!(q.0.front().map(String::as_str), Some("b"));
        assert_eq!(q.0.len(), 2);
    }
}
