//! **Every agent turn becomes one record, written by the editor and by nobody else** (DL-01, #460; series labels from use, #459).
//!
//! The decision model's training sets come from using the editor: a person asks for a mountain, the agent makes it, and what happened is kept so
//! it can be judged and learned from later. This module is the first half of that: **it writes down the turn and does nothing else**. It trains
//! nothing, asks no model and adds nothing for the agent to do — an agent that drives the editor as it always did is recorded all the same.
//!
//! # What a turn is
//!
//! **From the agent's first write operation after its last hand-back, to its hand-back** (`grimoire_present`, or a resolved note).
//!
//! 1. The **first write arrives** ([`TurnLedger::turn_begins`], called from `grimoire/operation_apply` before the operation runs). The turn opens
//!    and a **before** picture is asked for.
//! 2. Each applied operation of the agent is noted ([`TurnLedger::operation_seen`]), with where it was shown.
//! 3. **The hand-back arrives** ([`TurnLedger::hand_back`]). The record is complete except for the **after** picture, and that waits for the view
//!    to settle ([`advance_turns`]): the follow camera (AF-02, #358) has nothing left to do, and nothing on screen has changed for
//!    [`SETTLE_FRAMES`] frames in a row. **Never a mid-transition frame**: a view that never settles is a record whose after picture says so
//!    (`missing`) rather than a picture of a camera in flight.
//! 4. The line is appended to the ledger ([`crate::turn_store`]).
//!
//! # Why the editor is the one writer
//!
//! It holds the presentations, the operation log and the view. **Several MCP servers can run at once** (an agent each), and none of them writes
//! the ledger: `grimoire_decide` reports each call to the editor over BRP (`grimoire/decide_report`) and the editor files it under the turn
//! that is open.
//!
//! # Pictures
//!
//! **A capture is asked for and then lands**: [`TurnLedger`] hands out tickets, and whoever can draw delivers a picture for each
//! ([`TurnLedger::deliver_shot`]). In the running editor that is [`issue_shots`], which reads back the surface the whole application draws into.
//!
//! **Why the before picture is the frame the person last saw.** A readback (like a screenshot) copies the frame that is being finished when it is
//! spawned. The BRP bridge answers an agent's call in `RemoteLast`, after `Last` and so after every system that draws, and the redraw of what the
//! call changed waits for the next frame's `Update`: so the readback [`issue_shots`] spawns in `RemoteLast` (the schedule is named in
//! [`TurnLedgerPlugin`]) sees the map as it stood **before** the first write was drawn. The after picture is asked for from `Update`, once the view
//! has settled, and sees the settled view. A headless editor has nothing to draw, so the test that drives it delivers the pictures; a picture that
//! does not land within [`SHOT_LANDS_FRAMES`] is recorded as missing.
//!
//! # Off is off
//!
//! With `collect_labels` off, or with no labels folder named ([`LabelsDir`], which only the binary inserts — a test's editor has none, so no
//! test writes to the real data directory), **no picture is asked for and nothing is written**, and a turn already open is dropped.

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::render::gpu_readback::{ReadbackComplete, ReadbackOnce};
use serde_json::{Value, json};

use crate::ops::Entry;
use crate::present::HandBackKind;
use crate::turn_store::{AgentRef, HandBackRef, ImageRef, Line, OperationRecord, Provenance, Request, Shot, Store, Turn};

/// **The commit this build was made from**, stamped by `build.rs`; `unknown` when none could be found.
pub const BUILD_COMMIT: &str = env!("BUILD_COMMIT");

/// **How many frames in a row nothing on screen may change** before the after picture is taken: the camera's matrix, the rig and every
/// placement the map draws are the same as the frame before. Twelve is a fifth of a second at 60 frames, the shortest run that is not one
/// frame of a camera that happens to be passing through a rest.
pub const SETTLE_FRAMES: u32 = 12;

/// **How many frames the ledger waits for a view to settle** before it writes the record with no after picture and says why: thirty seconds at
/// 60 frames. A person working the camera for that long is not leaving the agent's result on screen to be judged.
pub const SETTLE_GIVE_UP_FRAMES: u32 = 1_800;

/// **How many frames a picture may take to land** once asked for: five seconds at 60 frames. A window that is hidden draws nothing, and a
/// record is not held for ever for a picture that is not coming.
pub const SHOT_LANDS_FRAMES: u32 = 300;

/// **The environment variable a benchmark's runner sets** on the editor it starts (BM-02): its turns are recorded too, marked `bench`, so that
/// DL-06 can keep them out of a split.
pub const BENCH_ENV: &str = "GRIMOIRE_BENCH_RUN";

/// **Where the ledger is kept**, named by the binary only (`main.rs`): `<data_dir>/Grimoire/labels`.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct LabelsDir(pub PathBuf);

/// **A frame as the renderer hands it over**: tightly packed RGBA, `width * height * 4` bytes.
#[derive(Clone, Debug)]
pub struct ShotImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// One picture asked for.
#[derive(Clone, Copy, Debug)]
struct Ask {
    ticket: u64,
    /// Whether a readback has been spawned for it.
    issued: bool,
    /// Frames since it was asked for.
    age: u32,
}

/// Where a turn's before picture is.
#[derive(Clone, Debug)]
enum Slot {
    /// Asked for; this ticket will carry it.
    Asked(u64),
    /// Landed, or failed with a reason.
    Settled(Shot),
}

/// One operation the agent applied in the open turn.
#[derive(Clone, Debug)]
struct Seen {
    operation_id: String,
    /// The log's own line for it.
    line: Value,
    /// The placements the operation said it touched (`Shown::Map`), by id.
    touched: Vec<String>,
}

/// The turn that has begun and not been handed back.
#[derive(Debug)]
struct OpenTurn {
    id: String,
    root: PathBuf,
    started_ms: u64,
    before: Slot,
    seen: Vec<Seen>,
    decisions: Vec<Value>,
}

/// What `finish` needs once the hand-back has arrived.
#[derive(Debug)]
struct Draft {
    id: String,
    started_ms: u64,
    handed_back_ms: u64,
    request: Request,
    before: Slot,
    operations: Vec<OperationRecord>,
    touched: Vec<(String, Vec<String>)>,
    decisions: Vec<Value>,
    agent: Option<AgentRef>,
    project_hash: String,
    /// `Some` once the facts were taken, at the moment the after picture was asked for.
    facts: Option<Value>,
    render: Value,
    after: Option<Slot>,
    /// Frames since the hand-back.
    waited: u32,
    /// Frames in a row the view has been the same and at rest.
    stable: u32,
    last_signature: Option<u64>,
}

/// **What a hand-back tells the ledger.**
pub struct HandBackInput<'a> {
    pub kind: HandBackKind,
    pub id: &'a str,
    pub title: &'a str,
    pub summary: &'a str,
    pub specified: &'a [String],
    pub inferred: &'a [String],
    pub check: Vec<String>,
    /// A note's text.
    pub note: Option<&'a str>,
    /// The operations the agent listed, in its order.
    pub listed: &'a [String],
    /// The operation log, as the caller has read it, so the turn's lines are the log's own.
    pub log: &'a [Entry],
    pub root: &'a Path,
    pub agent: Option<AgentRef>,
}

/// **The ledger's state in the editor**: the turn that is open, the hand-backs waiting for a settled view, the pictures asked for and the
/// store they are written to. A `Session` resource: a turn can cross a door, and what was written is the person's for good.
#[derive(Resource, Default)]
pub struct TurnLedger {
    store: Option<Store>,
    dir: Option<PathBuf>,
    /// The disk budget when it is not the default ([`crate::turn_store::DEFAULT_BUDGET_BYTES`]): a test's, today.
    budget: Option<u64>,
    /// Collecting now: a folder is named and the setting is on.
    on: bool,
    /// **Why collection stopped**, when the disk budget did it.
    stopped: Option<String>,
    asks: Vec<Ask>,
    next_ticket: u64,
    /// Every picture ever asked for, so a test can count what collection cost.
    requested: u32,
    /// Pictures that landed, by ticket, until the record that asked for them takes them.
    landed: BTreeMap<u64, Shot>,
    open: Option<OpenTurn>,
    finishing: Vec<Draft>,
    /// The ids of the turns written this session.
    written: Vec<String>,
    next_turn: u32,
    /// Sentences for the status line, drained by [`say_problems`].
    problems: Vec<String>,
}

/// **Who the agent is, for a record**: the client's name and version from its MCP handshake, when it made one.
pub fn agent_of(client: Option<&crate::present::AgentClient>) -> Option<AgentRef> {
    let named = client?.0.as_ref()?;
    Some(AgentRef { name: named.name.clone(), version: named.version.clone(), model: None })
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

impl TurnLedger {
    /// **Point the ledger at a folder, and turn it on or off.** Called every frame; it changes nothing unless something changed.
    ///
    /// Turning it **off** drops the open turn, every hand-back waiting for its picture and every picture asked for: off is off, and a record
    /// finished after the person said no would be one they did not agree to. Turning it back **on** forgets a stop for the disk budget, so a
    /// person who cleared the folder is collected from again.
    pub fn configure(&mut self, dir: Option<&Path>, enabled: bool) {
        let on = enabled && dir.is_some();
        if self.dir.as_deref() != dir {
            self.dir = dir.map(Path::to_path_buf);
            self.store = dir.map(|d| self.new_store(d.to_path_buf()));
        }
        if on && !self.on {
            self.stopped = None;
            if let Some(dir) = self.dir.clone() {
                self.store = Some(self.new_store(dir));
            }
        }
        if !on && self.on {
            self.open = None;
            self.finishing.clear();
            self.asks.clear();
            self.landed.clear();
        }
        self.on = on;
    }

    /// **Whether a turn would be recorded now.**
    pub fn collecting(&self) -> bool {
        self.on && self.stopped.is_none()
    }

    /// Why the disk budget stopped collection, if it did.
    pub fn stopped(&self) -> Option<&str> {
        self.stopped.as_deref()
    }

    /// **Change the disk budget**, for the folder in use and for any named later.
    pub fn set_budget(&mut self, bytes: u64) {
        self.budget = Some(bytes);
        if let Some(store) = self.store.take() {
            self.store = Some(store.with_budget(bytes));
        }
    }

    fn new_store(&self, dir: PathBuf) -> Store {
        let store = Store::new(dir);
        match self.budget {
            Some(bytes) => store.with_budget(bytes),
            None => store,
        }
    }

    /// **How many pictures have been asked for, ever.** With collection off this does not move.
    pub fn shots_requested(&self) -> u32 {
        self.requested
    }

    /// **The tickets still waiting for a picture**, oldest first.
    pub fn pending_shots(&self) -> Vec<u64> {
        self.asks.iter().map(|a| a.ticket).collect()
    }

    /// **The ids of the turns written this session**, oldest first.
    pub fn written(&self) -> &[String] {
        &self.written
    }

    /// Whether a turn has begun and not been handed back.
    pub fn turn_is_open(&self) -> bool {
        self.open.is_some()
    }

    /// Whether hand-backs are waiting for a settled view or a picture.
    pub fn waiting_to_finish(&self) -> usize {
        self.finishing.len()
    }

    /// The folder the ledger is written to, while it is on.
    pub fn folder(&self) -> Option<&Path> {
        self.on.then_some(self.dir.as_deref()).flatten()
    }

    fn request_shot(&mut self) -> u64 {
        self.next_ticket += 1;
        self.requested += 1;
        self.asks.push(Ask { ticket: self.next_ticket, issued: false, age: 0 });
        self.next_ticket
    }

    fn settle_slot(&mut self, slot: &mut Slot) -> bool {
        if let Slot::Asked(ticket) = slot
            && let Some(shot) = self.landed.remove(ticket)
        {
            *slot = Slot::Settled(shot);
        }
        matches!(slot, Slot::Settled(_))
    }

    /// **The first write of a turn is arriving**: open the turn and ask for the before picture, unless a turn is open already.
    ///
    /// Called before the operation is applied, so the picture is of what the person was looking at.
    pub fn turn_begins(&mut self, root: Option<&Path>) {
        let Some(root) = root else { return };
        if !self.collecting() {
            return;
        }
        // A turn open in another project is no turn of this one: its hand-back never came, and it is dropped rather than carried across.
        if self.open.as_ref().is_some_and(|open| open.root != root) {
            self.open = None;
        }
        if self.open.is_some() {
            return;
        }
        self.next_turn += 1;
        let started_ms = now_ms();
        let before = Slot::Asked(self.request_shot());
        self.open = Some(OpenTurn {
            id: format!("turn-{started_ms}-{}", self.next_turn),
            root: root.to_path_buf(),
            started_ms,
            before,
            seen: Vec::new(),
            decisions: Vec::new(),
        });
    }

    /// **An operation of the agent's was applied**: note it, and where the change was shown.
    pub fn operation_seen(&mut self, entry: &Entry, shown: &crate::ops::Shown) {
        let Some(open) = self.open.as_mut() else { return };
        if open.seen.iter().any(|s| s.operation_id == entry.operation_id) {
            return;
        }
        // **A placement's id is minted at commit**, so `Shown` for an add names the box and no ids: the id comes from what the operation
        // answered (`after.id`), and a change to a piece already there names it in its parameters (`params.id`).
        let mut touched = match shown {
            crate::ops::Shown::Map { placements, .. } => placements.clone(),
            _ => Vec::new(),
        };
        let named = [entry.after.as_ref().and_then(|after| after.get("id")), entry.params.get("id")];
        for id in named.into_iter().flatten().filter_map(Value::as_str) {
            if !touched.iter().any(|known| known == id) {
                touched.push(id.to_owned());
            }
        }
        open.seen.push(Seen {
            operation_id: entry.operation_id.clone(),
            line: serde_json::to_value(entry).unwrap_or(Value::Null),
            touched,
        });
    }

    /// **A `grimoire_decide` call was reported by its MCP server**, to be kept with the turn that is open. `false` when none is: a decision
    /// made before the turn's first write, or after its hand-back, belongs to no turn.
    pub fn decision_reported(&mut self, mut report: Value) -> bool {
        let Some(open) = self.open.as_mut().filter(|_| self.on && self.stopped.is_none()) else { return false };
        if let Some(object) = report.as_object_mut() {
            object.insert("reported_unix_ms".to_owned(), json!(now_ms()));
        }
        open.decisions.push(report);
        true
    }

    /// **The agent handed the turn back.** The record is complete but for its after picture, which waits for the view to settle.
    ///
    /// A hand-back with no turn open (the editor was started after the agent's writes, or the agent handed back twice) is recorded all the same,
    /// with a before picture that says there was none.
    pub fn hand_back(&mut self, input: HandBackInput<'_>) -> bool {
        if !self.collecting() {
            return false;
        }
        let open = self.open.take().filter(|open| open.root == input.root);
        let handed_back_ms = now_ms();
        let (id, started_ms, before, seen, decisions) = match open {
            Some(open) => (open.id, open.started_ms, open.before, open.seen, open.decisions),
            None => {
                self.next_turn += 1;
                (
                    format!("turn-{handed_back_ms}-{}", self.next_turn),
                    handed_back_ms,
                    Slot::Settled(Shot::missing("no write of the agent's was seen since the last hand-back, so no before picture was asked for")),
                    Vec::new(),
                    Vec::new(),
                )
            }
        };
        // **The operations are the ones the editor watched, then any the agent listed that it did not**: both are the log's own lines, so a
        // reader never compares the agent's account with a second one.
        let mut wanted: Vec<String> = seen.iter().map(|s| s.operation_id.clone()).collect();
        for id in input.listed {
            if !wanted.contains(id) {
                wanted.push(id.clone());
            }
        }
        let operations = wanted
            .iter()
            .map(|operation_id| {
                let logged = input.log.iter().find(|e| &e.operation_id == operation_id && e.outcome == "applied");
                let from_seen = seen.iter().find(|s| &s.operation_id == operation_id).map(|s| s.line.clone());
                let line = logged.and_then(|e| serde_json::to_value(e).ok()).or(from_seen).unwrap_or(Value::Null);
                OperationRecord {
                    operation_id: operation_id.clone(),
                    op: line["op"].as_str().unwrap_or_default().to_owned(),
                    seq: line["seq"].as_u64().unwrap_or_default(),
                    source: line["source"].as_str().unwrap_or_default().to_owned(),
                    line,
                }
            })
            .collect();
        self.finishing.push(Draft {
            id,
            started_ms,
            handed_back_ms,
            request: Request {
                hand_back: HandBackRef {
                    kind: match input.kind {
                        HandBackKind::Presentation => "presentation",
                        HandBackKind::Note => "note",
                    }
                    .to_owned(),
                    id: input.id.to_owned(),
                },
                title: input.title.to_owned(),
                summary: input.summary.to_owned(),
                specified: input.specified.to_vec(),
                inferred: input.inferred.to_vec(),
                check: input.check,
                note: input.note.map(str::to_owned),
            },
            before,
            operations,
            touched: seen.iter().map(|s| (s.operation_id.clone(), s.touched.clone())).collect(),
            decisions,
            agent: input.agent,
            project_hash: crate::turn_store::sha256_hex(input.root.to_string_lossy().as_bytes()),
            facts: None,
            render: Value::Null,
            after: None,
            waited: 0,
            stable: 0,
            last_signature: None,
        });
        true
    }

    /// **A picture landed for `ticket`.** It is encoded and stored now; a ticket nobody is waiting for (the ledger was turned off, or the
    /// picture came late) is dropped. Returns whether the ticket was one the ledger was waiting on.
    pub fn deliver_shot(&mut self, ticket: u64, picture: Result<ShotImage, String>) -> bool {
        let Some(at) = self.asks.iter().position(|a| a.ticket == ticket) else { return false };
        self.asks.remove(at);
        let shot = match picture {
            Ok(image) => match self.keep(&image) {
                Ok(stored) => Shot::taken(stored),
                Err(why) => {
                    self.stop(why.clone());
                    Shot::missing(why)
                }
            },
            Err(why) => Shot::missing(why),
        };
        self.landed.insert(ticket, shot);
        true
    }

    fn keep(&mut self, image: &ShotImage) -> Result<ImageRef, String> {
        let png = encode_png(image)?;
        let store = self.store.as_mut().ok_or_else(|| "no folder is named for the ledger".to_owned())?;
        store.put_image(&png, image.width, image.height)
    }

    /// **Collection stops**, and the person is told once.
    fn stop(&mut self, why: String) {
        if self.stopped.is_none() {
            self.problems.push(why.clone());
            self.stopped = Some(why);
        }
    }

    /// Age every ask by a frame; one that has waited [`SHOT_LANDS_FRAMES`] fails with its reason.
    fn age_asks(&mut self) {
        let mut late = Vec::new();
        for ask in &mut self.asks {
            ask.age += 1;
            if ask.age >= SHOT_LANDS_FRAMES {
                late.push(ask.ticket);
            }
        }
        for ticket in late {
            self.deliver_shot(ticket, Err(format!("the picture did not land within {SHOT_LANDS_FRAMES} frames")));
        }
    }

    fn provenance_of(draft: &Draft) -> Provenance {
        Provenance {
            cauldron_version: env!("CARGO_PKG_VERSION").to_owned(),
            build_commit: BUILD_COMMIT.to_owned(),
            render: draft.render.clone(),
            agent: draft.agent.clone(),
            project_hash: draft.project_hash.clone(),
        }
    }

    fn write(&mut self, draft: Draft) {
        let provenance = Self::provenance_of(&draft);
        let Draft { id, started_ms, handed_back_ms, request, before: Slot::Settled(before), operations, facts, decisions, after: Some(Slot::Settled(after)), .. } =
            draft
        else {
            return;
        };
        let turn = Turn {
            id: id.clone(),
            bench: std::env::var_os(BENCH_ENV).is_some(),
            started_unix_ms: started_ms,
            handed_back_unix_ms: handed_back_ms,
            provenance,
            request,
            before,
            after,
            operations,
            facts: facts.unwrap_or(Value::Null),
            decisions,
        };
        let Some(store) = self.store.as_mut() else { return };
        match store.append(&Line::turn(turn)) {
            Ok(()) => self.written.push(id),
            Err(why) => self.stop(why),
        }
    }
}

/// **Frames' worth of what is on screen**, gathered once per frame for the settle test and the facts.
#[derive(SystemParam)]
pub struct Sight<'w, 's> {
    following: Option<Res<'w, crate::follow::Following>>,
    camera: Query<'w, 's, &'static GlobalTransform, With<crate::view::MainCamera>>,
    open: Option<Res<'w, crate::project::OpenMap>>,
    project: Option<Res<'w, crate::project::Project>>,
    rig: Option<Res<'w, crate::view::Rig>>,
    due: Option<Res<'w, crate::watch::Due>>,
    confirm: Option<Res<'w, crate::confirm::Confirm>>,
    ui_scale: Option<Res<'w, UiScale>>,
    surface: Option<Res<'w, crate::surface::Surface>>,
    images: Option<Res<'w, Assets<Image>>>,
}

impl Sight<'_, '_> {
    /// **Whether the view is at rest**: the follow has nothing queued, held or easing, and the map is not waiting to be drawn again.
    fn at_rest(&self) -> bool {
        self.following.as_ref().is_none_or(|following| following.at_rest()) && !self.due.as_ref().is_some_and(|due| due.map_replaced)
    }

    /// **A number that changes when anything the picture shows changes**: the camera's matrix, the rig and every placement's id, place and yaw.
    fn signature(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for transform in &self.camera {
            transform.to_matrix().to_cols_array().map(f32::to_bits).hash(&mut hasher);
        }
        if let Some(rig) = &self.rig {
            [rig.focus.x, rig.focus.y, rig.focus.z, rig.height, rig.yaw, rig.elevation].map(f32::to_bits).hash(&mut hasher);
        }
        if let Some(open) = &self.open {
            open.map.placements.len().hash(&mut hasher);
            for placed in &open.map.placements {
                placed.id.hash(&mut hasher);
                [placed.at.0, placed.at.1, placed.yaw, placed.lift].map(f32::to_bits).hash(&mut hasher);
            }
        }
        hasher.finish()
    }

    /// **How the surface drew**: its size and format, the interface scale, and whether a question stood on the picture.
    fn render(&self) -> Value {
        let surface = self.surface.as_ref().zip(self.images.as_ref()).and_then(|(surface, images)| images.get(&surface.image)).map(|image| {
            json!({
                "width": image.width(),
                "height": image.height(),
                "format": format!("{:?}", image.texture_descriptor.format),
            })
        });
        json!({
            "surface": surface,
            "ui_scale": self.ui_scale.as_ref().map(|scale| scale.0),
            // **A question on the picture**: the hand-back's own Keep / Undo modal stands over the map while the after picture is taken.
            "question_open": self.confirm.as_ref().is_some_and(|confirm| confirm.is_open()),
        })
    }

    /// **The facts at the after picture** (#460): the pieces each operation touched, with names and positions, the camera, and the engine's
    /// standing findings — computed from the map as it is *now*, which is when the picture is taken.
    fn facts(&self, touched: &[(String, Vec<String>)]) -> Value {
        let (Some(open), Some(project)) = (self.open.as_ref(), self.project.as_ref()) else {
            return json!({"map": null, "note": "no map was open when the after picture was taken"});
        };
        let operations: Vec<Value> = touched
            .iter()
            .map(|(operation_id, ids)| {
                let pieces: Vec<Value> = ids
                    .iter()
                    .map(|id| match open.map.placements.iter().find(|placed| &placed.id == id) {
                        Some(placed) => json!({
                            "id": placed.id,
                            "descriptor": placed.descriptor,
                            "at": [placed.at.0, placed.at.1],
                            "yaw": placed.yaw,
                        }),
                        None => json!({"id": id, "present": false}),
                    })
                    .collect();
                json!({"operation_id": operation_id, "pieces": pieces})
            })
            .collect();
        json!({
            "map": {
                "name": open.map.name,
                "origin": [open.map.origin.0, open.map.origin.1, open.map.origin.2],
                "bounds": [open.map.bounds.0, open.map.bounds.1, open.map.bounds.2],
                "placements": open.map.placements.len(),
            },
            "operations": operations,
            "camera": self.rig.as_ref().map(|rig| json!({
                "focus": [rig.focus.x, rig.focus.y, rig.focus.z],
                "height": rig.height,
                "yaw": rig.yaw,
                "elevation": rig.elevation,
            })),
            "checks": crate::checks::report(None, &crate::checks::find_in(project, open)),
        })
    }
}

/// **Keep the ledger pointed at its folder and in step with the setting**, every frame.
fn sync_collection(
    settings: Option<Res<crate::settings::EditorSettings>>,
    dir: Option<Res<LabelsDir>>,
    mut ledger: ResMut<TurnLedger>,
) {
    let enabled = settings.is_none_or(|settings| settings.collect_labels);
    // `configure` compares before it changes, but taking the `ResMut` marks the resource changed, so only touch it when something differs.
    let folder = dir.as_deref().map(|d| d.0.as_path());
    let wants_on = enabled && folder.is_some();
    if ledger.on != wants_on || ledger.dir.as_deref() != folder {
        ledger.configure(folder, enabled);
    }
}

/// **Spawn the readbacks for pictures asked for and not yet read**, in the running editor: from `RemoteLast` for a picture asked for as an agent's
/// call arrives (so it is of the frame the person last saw, see the module notes), and from `Update` for the rest. The surface is what the whole
/// application draws into, so a copy of it is what the person sees.
fn issue_shots(
    mut commands: Commands,
    mut ledger: ResMut<TurnLedger>,
    surface: Option<Res<crate::surface::Surface>>,
    images: Option<Res<Assets<Image>>>,
) {
    if !ledger.asks.iter().any(|ask| !ask.issued) {
        return;
    }
    let Some((surface, image)) = surface.zip(images).and_then(|(surface, images)| {
        let image = images.get(&surface.image)?;
        Some((surface.image.clone(), (image.width(), image.height(), image.texture_descriptor.format)))
    }) else {
        return;
    };
    let (width, height, format) = image;
    for ask in ledger.asks.iter_mut().filter(|ask| !ask.issued) {
        ask.issued = true;
        let ticket = ask.ticket;
        commands.spawn(ReadbackOnce::texture(surface.clone())).observe(
            move |ready: On<ReadbackComplete>, mut ledger: ResMut<TurnLedger>, mut commands: Commands| {
                commands.entity(ready.entity).despawn();
                let picture = rgba_of(&ready.data, width, height, format);
                ledger.deliver_shot(ticket, picture);
            },
        );
    }
}

/// **The surface's bytes as RGBA**, whichever order the texture holds them in.
fn rgba_of(data: &[u8], width: u32, height: u32, format: bevy::render::render_resource::TextureFormat) -> Result<ShotImage, String> {
    use bevy::render::render_resource::TextureFormat;
    let expected = width as usize * height as usize * 4;
    if data.len() != expected {
        return Err(format!("the readback held {} bytes where a {width} x {height} surface holds {expected}", data.len()));
    }
    let mut rgba = data.to_vec();
    match format {
        TextureFormat::Bgra8Unorm | TextureFormat::Bgra8UnormSrgb => rgba.chunks_exact_mut(4).for_each(|px| px.swap(0, 2)),
        TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb => {}
        other => return Err(format!("the surface is {other:?}, which the ledger does not read")),
    }
    Ok(ShotImage { width, height, rgba })
}

/// **A picture as a PNG.**
fn encode_png(image: &ShotImage) -> Result<Vec<u8>, String> {
    let rgba = image::RgbaImage::from_raw(image.width, image.height, image.rgba.clone())
        .ok_or_else(|| format!("{} bytes are not a {} x {} picture", image.rgba.len(), image.width, image.height))?;
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(rgba).write_to(&mut png, image::ImageFormat::Png).map_err(|e| format!("the picture did not encode: {e}"))?;
    Ok(png.into_inner())
}

/// **Take each waiting hand-back one frame closer to its record**: wait for the view to settle, ask for the after picture, and write the line when
/// both pictures are in.
pub fn advance_turns(mut ledger: ResMut<TurnLedger>, sight: Sight) {
    if !ledger.on {
        return;
    }
    // A frame passes for every picture asked for; this is also what turns a picture that never lands into a reason.
    if !ledger.asks.is_empty() {
        ledger.age_asks();
    }
    if ledger.finishing.is_empty() {
        return;
    }
    let signature = sight.signature();
    let at_rest = sight.at_rest();
    let waiting = std::mem::take(&mut ledger.finishing);
    let mut still = Vec::with_capacity(waiting.len());
    for mut draft in waiting {
        draft.waited += 1;
        ledger.settle_slot(&mut draft.before);
        // **The view is still being waited on**: nothing has been asked for yet.
        if draft.after.is_none() {
            if at_rest && draft.last_signature == Some(signature) {
                draft.stable += 1;
            } else {
                draft.stable = 0;
            }
            draft.last_signature = Some(signature);
            if draft.stable >= SETTLE_FRAMES {
                draft.facts = Some(sight.facts(&draft.touched));
                draft.render = sight.render();
                draft.after = Some(Slot::Asked(ledger.request_shot()));
            } else if draft.waited >= SETTLE_GIVE_UP_FRAMES {
                draft.facts = Some(sight.facts(&draft.touched));
                draft.render = sight.render();
                draft.after = Some(Slot::Settled(Shot::missing(format!(
                    "the view did not settle within {SETTLE_GIVE_UP_FRAMES} frames, so no after picture was taken"
                ))));
            }
        }
        if let Some(after) = draft.after.as_mut() {
            ledger.settle_slot(after);
        }
        if matches!(draft.before, Slot::Settled(_)) && matches!(draft.after, Some(Slot::Settled(_))) {
            ledger.write(draft);
        } else {
            still.push(draft);
        }
    }
    // Whatever was handed back while this ran (nothing today) stays behind what was already waiting.
    still.append(&mut ledger.finishing);
    ledger.finishing = still;
}

/// **Say what the ledger has to say**, on the status line: collection stopped for the disk budget, or a record could not be written.
fn say_problems(mut ledger: ResMut<TurnLedger>, state: Option<ResMut<crate::editor::EditorState>>) {
    if ledger.problems.is_empty() {
        return;
    }
    let problems = std::mem::take(&mut ledger.problems);
    if let Some(mut state) = state {
        for why in problems {
            state.status.problem(why);
        }
    }
}

/// **The ledger's one plugin**, in the editor's shared list (`harness::add_editor_plugins`), so a headless test runs the same systems the binary does.
pub struct TurnLedgerPlugin;

impl Plugin for TurnLedgerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TurnLedger>()
            .add_systems(
                Update,
                (sync_collection, issue_shots, advance_turns, say_problems)
                    .chain()
                    // **After the settings are read this frame**, so a person who just turned collection off is not collected from for one more frame.
                    .after(crate::settings::sync_settings),
            )
            // **At the end of the frame an agent's call is answered in**, after the bridge has run it, so a before picture asked for by the
            // first write is read back in the same frame and before that write is drawn. The schedule only runs where the bridge is built.
            .add_systems(bevy::remote::RemoteLast, issue_shots.after(bevy::remote::RemoteSystems::ProcessRequests));
    }
}
