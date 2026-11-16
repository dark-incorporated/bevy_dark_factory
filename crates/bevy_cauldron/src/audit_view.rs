//! **The person's audit: a few of the questions, answered in one press, as the human holdout** (DL-07, #466; series labels from use, #459).
//!
//! Keep and Undo label a whole turn, but a student is graded per question, and the assessor (DL-04) can only be graded against a person. So the
//! Agent workspace offers an **Audit** row: one turn's before and after pictures and one of its questions, answered **Y**es, **N**o or **S**kip.
//!
//! # What it is
//!
//! - **Never a pop-up.** The row waits in the AGENT block with a count ("AUDIT 7 of 20 this week, 13 waiting"). The weekly target
//!   ([`WEEKLY_TARGET`]) is shown, never enforced.
//! - **A stratified sample.** [`SPLIT_SHARE`] of what is offered is where the assessor's votes split or were refused, the questions a person's answer
//!   teaches the most; the rest is random, so the holdout is not only hard cases. A question is offered once: an answered one, a skipped one (recorded as
//!   `skip`, never as a no), and one the person has already been asked, leave the pool for good.
//! - **The answers are gold, per question, and marked `holdout`** ([`crate::turn_store::Label::holdout`]). A holdout answer is never trained on: DL-06
//!   writes them to their own file and DL-08 grades every model, the assessor included, against them. [`crate::turn_store::Read::holdout`] is the one
//!   place that says which lines they are.
//! - **Only yes/no questions** are offered: Y, N and S are the whole keyboard of the row.
//! - **Off is off.** With collection off there is no ledger to audit and no row.
//!
//! The keys are read raw, as the hand-back's own `Y`/`N` are ([`crate::confirm`]): the row is not a tab's verb, and the Agent tab has no keys of
//! its own. They answer only while the Agent workspace is open, no question card is up, no text field has the keyboard and no modifier is held (so
//! Ctrl/Cmd+S still saves).

use std::collections::BTreeSet;
use std::path::Path;

use bevy::asset::RenderAssetUsages;
use bevy::image::{Image, ImageSampler};
use bevy::input::ButtonInput;
use bevy::prelude::*;
use bevy_gooey::{Activate, button, label, observe};

use crate::turn_ledger::TurnLedger;
use crate::turn_store::{ImageRef, Read, Store};

/// **The week's target**, shown with the count and never enforced (#466).
pub const WEEKLY_TARGET: usize = 20;

/// **The share of what is offered that comes from disagreement**: the assessor's votes split or were refused. The rest is random.
pub const SPLIT_SHARE: f32 = 0.7;

/// **How often the ledger is looked at for new questions**, in seconds, while the block is on screen.
pub const LOOK_EVERY_S: f32 = 5.0;

/// How big a picture is drawn in the row, in logical pixels.
const PICTURE: Vec2 = Vec2::new(240.0, 150.0);

const WEEK_MS: u64 = 7 * 24 * 60 * 60 * 1000;

/// **Why a question is offered.**
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    /// The assessor's votes split, or one was refused.
    Split,
    /// Chosen at random.
    Random,
}

impl Why {
    /// The word the ledger keeps.
    pub fn word(self) -> &'static str {
        match self {
            Why::Split => "split",
            Why::Random => "random",
        }
    }
}

/// **One question offered to the person**, with what they look at.
#[derive(Clone, Debug, PartialEq)]
pub struct Offer {
    pub turn: String,
    pub question: String,
    /// The question's words, as the record stores them.
    pub text: String,
    pub before: Option<ImageRef>,
    pub after: Option<ImageRef>,
    pub why: Why,
}

impl Offer {
    fn is(&self, other: &Offer) -> bool {
        self.turn == other.turn && self.question == other.question
    }
}

/// **How many questions the person has answered in the week before `now_ms`**, a skip counted: it was looked at.
pub fn answered_this_week(read: &Read, now_ms: u64) -> usize {
    read.audits().filter(|a| a.at_unix_ms + WEEK_MS >= now_ms).count()
}

/// **A turn's yes/no questions the person has not been asked**, in the ledger's order, each with how unsure the assessor was (0 = a coin toss, `None` =
/// it was sure, or never looked).
fn candidates(read: &Read) -> Vec<(Offer, Option<f64>)> {
    let asked: BTreeSet<(&str, &str)> = read.audits().filter_map(|a| Some((a.turn.as_str(), a.question.as_deref()?))).collect();
    let mut out = Vec::new();
    for turn in read.turns() {
        if turn.before.image.is_none() && turn.after.image.is_none() {
            continue;
        }
        // The newest assessment of the turn, if any.
        let assessment = read.assessments().filter(|a| a.turn == turn.id).last();
        for item in &turn.questions.items {
            if asked.contains(&(turn.id.as_str(), item.id.as_str())) || item.question["type"] != "noul" {
                continue;
            }
            let Some(text) = item.question["instructions"].as_str() else { continue };
            let graded = assessment.and_then(|a| a.answers.iter().find(|answer| answer.id == item.id));
            // A refused vote is as unsure as a split; a fraction nearer 0.5 is more so.
            let unsure = graded.and_then(|g| match g.yes_fraction {
                _ if g.refused > 0 => Some(0.0),
                Some(fraction) if fraction > 0.0 && fraction < 1.0 => Some((fraction - 0.5).abs()),
                _ => None,
            });
            out.push((
                Offer {
                    turn: turn.id.clone(),
                    question: item.id.clone(),
                    text: text.to_owned(),
                    before: turn.before.image.clone(),
                    after: turn.after.image.clone(),
                    why: if unsure.is_some() { Why::Split } else { Why::Random },
                },
                unsure,
            ));
        }
    }
    out
}

/// **A small deterministic shuffle**, so a test (and a person reading the same ledger twice) sees the same sample for the same seed.
fn shuffled<T>(mut items: Vec<T>, seed: u64) -> Vec<T> {
    let mut state = seed | 1;
    for i in (1..items.len()).rev() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        items.swap(i, (state % (i as u64 + 1)) as usize);
    }
    items
}

/// **Choose what to offer**: up to `count` questions, [`SPLIT_SHARE`] of them from the assessor's disagreement (the most split first) and the rest at
/// random (a disagreement that did not fill its share leaves room for the random part).
pub fn pick(read: &Read, count: usize, seed: u64) -> Vec<Offer> {
    let (mut split, random): (Vec<_>, Vec<_>) = candidates(read).into_iter().partition(|(_, unsure)| unsure.is_some());
    split.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    let want = ((count as f32 * SPLIT_SHARE).ceil() as usize).min(count);
    let mut offers: Vec<Offer> = split.drain(..want.min(split.len())).map(|(offer, _)| offer).collect();
    // What disagreement did not take is random like the rest.
    let rest: Vec<Offer> = split.into_iter().chain(random).map(|(offer, _)| offer).collect();
    offers.extend(shuffled(rest, seed).into_iter().take(count.saturating_sub(offers.len())));
    offers
}

/// **The audit's state**: what is waiting, and how the week stands.
#[derive(Resource, Default)]
pub struct Audit {
    /// The questions waiting, the one being asked first.
    pub offers: Vec<Offer>,
    /// How many were answered in the week, as of the last read.
    pub this_week: usize,
    /// The seconds on the clock when the ledger was last looked at.
    looked_at: Option<f32>,
    /// The ledger's size when it was last read: it is read again only when it grew.
    read_len: Option<u64>,
    /// What the row on screen shows, so it is rebuilt only when this moves.
    shown: Option<(String, String, usize, usize)>,
}

/// **The row's container**, spawned empty by the AGENT block; its one child is this module's.
#[derive(Component)]
pub struct AuditRows;

/// The row.
#[derive(Component)]
pub struct AuditRow;

/// **An answer's button**: which answer it gives.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum AuditButton {
    Yes,
    No,
    Skip,
}

impl AuditButton {
    fn word(self) -> &'static str {
        match self {
            AuditButton::Yes => "yes",
            AuditButton::No => "no",
            AuditButton::Skip => "skip",
        }
    }
}

/// **Answers pressed, waiting to be written**: by the buttons and the keys alike, so the two are one path.
#[derive(Resource, Default)]
pub struct Answers(pub Vec<AuditButton>);

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// **Read the ledger for what to offer**, now and then, while the AGENT block is on screen, and only when the file grew.
fn look_at_the_ledger(time: Res<Time<Real>>, ledger: Option<Res<TurnLedger>>, mut audit: ResMut<Audit>, containers: Query<(), With<AuditRows>>) {
    if containers.is_empty() {
        audit.looked_at = None;
        return;
    }
    let now = time.elapsed_secs();
    if audit.looked_at.is_some_and(|at| now - at < LOOK_EVERY_S) {
        return;
    }
    audit.looked_at = Some(now);
    let Some(folder) = ledger.as_deref().and_then(TurnLedger::folder) else {
        // Off is off: nothing to audit.
        audit.offers.clear();
        audit.read_len = None;
        return;
    };
    let path = Store::new(folder.to_path_buf()).ledger_path();
    let len = std::fs::metadata(&path).map(|m| m.len()).ok();
    if len.is_none() || len == audit.read_len {
        return;
    }
    let Ok(read) = crate::turn_store::read_file(&path) else { return };
    audit.read_len = len;
    audit.this_week = answered_this_week(&read, now_ms());
    let waiting = WEEKLY_TARGET.saturating_sub(audit.this_week).max(1);
    let mut offers = pick(&read, waiting, read.lines.len() as u64);
    // The question on screen stays first: a new look must not take it from under the person's hand.
    if let Some(first) = audit.offers.first().cloned()
        && offers.iter().any(|o| o.is(&first))
    {
        offers.retain(|o| !o.is(&first));
        offers.insert(0, first);
    }
    audit.offers = offers;
}

/// **A stored picture as an image**, or `None` when it cannot be read.
fn picture(images: &mut Assets<Image>, folder: &Path, image: &ImageRef) -> Option<Handle<Image>> {
    let bytes = std::fs::read(Store::new(folder.to_path_buf()).image_path(&image.sha256)).ok()?;
    let decoded = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png).ok()?.to_rgba8();
    let (width, height) = decoded.dimensions();
    let mut picture = Image::new(
        bevy::render::render_resource::Extent3d { width, height, depth_or_array_layers: 1 },
        bevy::render::render_resource::TextureDimension::D2,
        decoded.into_raw(),
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    picture.sampler = ImageSampler::linear();
    Some(images.add(picture))
}

/// **Build the row, or take it away**, as the questions come and go.
fn follow_the_audit(
    mut commands: Commands,
    ledger: Option<Res<TurnLedger>>,
    mut images: ResMut<Assets<Image>>,
    mut audit: ResMut<Audit>,
    containers: Query<Entity, With<AuditRows>>,
    rows: Query<Entity, With<AuditRow>>,
) {
    let now = audit.offers.first().map(|o| (o.turn.clone(), o.question.clone(), audit.offers.len(), audit.this_week));
    if audit.shown == now && (now.is_some() != rows.is_empty()) {
        return;
    }
    for row in &rows {
        commands.entity(row).try_despawn();
    }
    audit.shown = now;
    let (Some(offer), Some(folder), Ok(container)) = (audit.offers.first().cloned(), ledger.as_deref().and_then(TurnLedger::folder), containers.single()) else {
        return;
    };
    let before = offer.before.as_ref().and_then(|image| picture(&mut images, folder, image));
    let after = offer.after.as_ref().and_then(|image| picture(&mut images, folder, image));
    let header = format!("AUDIT: {} of {} this week, {} waiting", audit.this_week, WEEKLY_TARGET, audit.offers.len());
    let why = match offer.why {
        Why::Split => "asked because the assessor was unsure",
        Why::Random => "asked at random",
    };
    commands.entity(container).with_children(|list| {
        list.spawn((AuditRow, Name::new("audit"), bevy_gooey::column_with(crate::chrome::GAP_TIGHT_STEPS, bevy_gooey::RowLayout { shrink: Some(0.0), ..default() })))
            .with_children(|row| {
                row.spawn(label(header));
                row.spawn(label(why));
                for (name, handle) in [("before", before), ("after", after)] {
                    match handle {
                        Some(handle) => {
                            row.spawn(label(name));
                            row.spawn(bevy_gooey::image_box(handle, PICTURE));
                        }
                        None => {
                            row.spawn(label(format!("{name}: no picture")));
                        }
                    }
                }
                row.spawn(label(offer.text.clone()));
                row.spawn(bevy_gooey::row_with(crate::chrome::GAP_TIGHT_STEPS, bevy_gooey::RowLayout::default())).with_children(|buttons| {
                    for (kind, words) in [(AuditButton::Yes, "Yes (Y)"), (AuditButton::No, "No (N)"), (AuditButton::Skip, "Skip (S)")] {
                        buttons.spawn((button(words), kind, observe(move |_: On<Activate>, mut answers: ResMut<Answers>| answers.0.push(kind))));
                    }
                });
            });
    });
}

/// **`Y`, `N` and `S` answer the question on screen** under the conditions in the module's words.
fn answer_on_the_keys(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    workspace: Option<Res<crate::workspace::Workspace>>,
    confirm: Option<Res<crate::confirm::Confirm>>,
    focus: Option<Res<bevy::input_focus::InputFocus>>,
    audit: Res<Audit>,
    mut answers: ResMut<Answers>,
) {
    if workspace.is_none_or(|w| *w != crate::workspace::Workspace::Agent)
        || audit.offers.is_empty()
        || confirm.is_some_and(|c| c.is_open())
        || focus.is_some_and(|f| f.get().is_some())
        || keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight, KeyCode::SuperLeft, KeyCode::SuperRight, KeyCode::AltLeft, KeyCode::AltRight])
    {
        return;
    }
    for (code, kind) in [(KeyCode::KeyY, AuditButton::Yes), (KeyCode::KeyN, AuditButton::No), (KeyCode::KeyS, AuditButton::Skip)] {
        if crate::confirm::take(&mut keys, code) {
            answers.0.push(kind);
        }
    }
}

/// **Write the answers**, in order: each is a line in the ledger, and the next question follows.
fn write_the_answers(mut answers: ResMut<Answers>, mut audit: ResMut<Audit>, mut ledger: Option<ResMut<TurnLedger>>) {
    for kind in std::mem::take(&mut answers.0) {
        if audit.offers.is_empty() {
            break;
        }
        let offer = audit.offers.remove(0);
        if let Some(ledger) = ledger.as_deref_mut() {
            ledger.audit_answered(&offer.turn, &offer.question, kind.word(), offer.why.word());
        }
        audit.this_week += 1;
        // The file grew by this answer: the next look reads it.
        audit.read_len = None;
    }
}

/// **The audit's plugin.**
pub struct AuditPlugin;

impl Plugin for AuditPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Audit>().init_resource::<Answers>().add_systems(
            Update,
            (answer_on_the_keys.after(crate::confirm::ConfirmKeys).before(crate::keys::Phase::Sense), (write_the_answers, look_at_the_ledger, follow_the_audit).chain()),
        );
    }
}
