//! **Assess-then-fix pairs: a turn that was told something was wrong, and made it right** (DL-05, #464; series labels from use, #459).
//!
//! A turn that only ever succeeds teaches a student to say yes. Some turns do not: the agent asks `grimoire_decide` whether something is wrong,
//! the answer says it is, and the agent changes what it made until the same check says it is not. The frame the check flagged is a negative
//! with a known answer, and the frame it cleared is a positive, and both are in the one turn.
//!
//! # What a pair is
//!
//! **The same named check, answered twice in one turn**: a first answer that found the problem (`finding.found` true), and a later one that did
//! not, with at least one operation applied between them. The later answer always belongs to the nearest earlier flag of its check that no
//! other answer has already closed, so a check flagged twice and cleared once makes one pair, from the second flag (the frame closest to the
//! fix). What lies between is the fix, named by operation id.
//!
//! # Silver, and why
//!
//! The pair rests on one assessor agreeing with itself across two frames, which is not the engine's word and not a person's. DL-06 weighs it as
//! silver: used when it agrees with the engine's facts or a person's verdict on the turn, and parked when it does not.
//!
//! # Where the frames come from
//!
//! The editor takes a picture as it files an answer that flags a problem, or one that clears a problem flagged earlier in the turn
//! ([`wants_frame`], called by the ledger), and files it under the answer's `frame` key. An answer that flags nothing and clears nothing asks
//! for no picture. A pair whose answer has no `frame` (the turn was recorded by a build from before DL-05) carries a `missing` shot that says
//! so; it is never a silently empty field.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::turn_store::{AssessFix, AssessedAnswer, Line, Read, Shot, Store, TIER_SILVER, Turn, scene_of};

/// **A named check's answer, as `grimoire_decide` returns it and the editor files it**: which check, and whether it found the problem.
#[derive(Clone, Debug, PartialEq)]
pub struct Finding {
    pub check: String,
    pub found: bool,
    pub p_yes: Option<f64>,
    pub threshold: Option<f64>,
}

/// **The finding in one decision record**, or `None` for a call that is not a named check's answer (ad-hoc questions, an error, no answer).
pub fn finding_of(decision: &Value) -> Option<Finding> {
    let finding = decision.get("answer")?.get("finding")?;
    Some(Finding {
        check: finding.get("check")?.as_str()?.to_owned(),
        found: finding.get("found")?.as_bool()?,
        p_yes: finding.get("p_yes").and_then(Value::as_f64),
        threshold: finding.get("threshold").and_then(Value::as_f64),
    })
}

/// **Whether the answer being reported is worth a picture**, given the answers already filed in the turn: it flags a problem, or it clears a
/// check whose latest answer was a flag.
pub fn wants_frame(filed: &[Value], report: &Value) -> bool {
    let Some(now) = finding_of(report) else { return false };
    if now.found {
        return true;
    }
    filed.iter().rev().filter_map(finding_of).find(|earlier| earlier.check == now.check).is_some_and(|earlier| earlier.found)
}

/// How many operations stood behind a filed answer.
fn after_operations(decision: &Value) -> Option<usize> {
    decision.get("after_operations")?.as_u64().and_then(|n| usize::try_from(n).ok())
}

/// **The frame a filed answer carries**, or the reason it has none.
fn frame_of(decision: &Value) -> Shot {
    match decision.get("frame").cloned().map(serde_json::from_value::<Shot>) {
        Some(Ok(shot)) => shot,
        _ => Shot::missing("the editor filed no picture with this answer, so there is no frame of what it was about"),
    }
}

fn assessment(at: usize, decision: &Value, finding: &Finding, after_operations: usize) -> AssessedAnswer {
    AssessedAnswer {
        decision: at,
        after_operations,
        p_yes: finding.p_yes,
        threshold: finding.threshold,
        model: decision.get("model").and_then(Value::as_str).map(str::to_owned),
        frame: frame_of(decision),
    }
}

/// **The assess-then-fix pairs of one turn**, in the order their fixes happened. Empty for a turn with none.
pub fn pairs(turn: &Turn) -> Vec<AssessFix> {
    let scene = scene_of(turn);
    // The latest flag of each check that nothing has answered since.
    let mut standing: BTreeMap<String, usize> = BTreeMap::new();
    let mut out = Vec::new();
    for (at, decision) in turn.decisions.iter().enumerate() {
        let Some(finding) = finding_of(decision) else { continue };
        let Some(ops_now) = after_operations(decision) else { continue };
        if finding.found {
            standing.insert(finding.check, at);
            continue;
        }
        let Some(&flagged_at) = standing.get(&finding.check) else { continue };
        let flagged = &turn.decisions[flagged_at];
        let (Some(flagged_finding), Some(ops_then)) = (finding_of(flagged), after_operations(flagged)) else { continue };
        // **Operations between the two answers are the fix.** None between them is a model that changed its mind, not a turn that fixed anything.
        if ops_now <= ops_then {
            continue;
        }
        standing.remove(&finding.check);
        let fixing_operations = turn.operations.iter().skip(ops_then).take(ops_now - ops_then).map(|o| o.operation_id.clone()).collect();
        out.push(AssessFix {
            id: format!("assessfix-{}-{}", turn.id, out.len() + 1),
            scene: scene.clone(),
            source_turn: turn.id.clone(),
            check: finding.check.clone(),
            tier: TIER_SILVER.to_owned(),
            flagged: assessment(flagged_at, flagged, &flagged_finding, ops_then),
            cleared: assessment(at, decision, &finding, ops_now),
            fixing_operations,
            bench: turn.bench,
        });
    }
    out
}

/// **Write the pairs a ledger's turns hold and the ledger does not yet**, for a ledger written before pairs were. A pair already there (by id) is
/// left alone, so running this twice writes nothing the second time. Returns the ids written.
pub fn write_missing(read: &Read, store: &mut Store) -> Result<Vec<String>, String> {
    let known: Vec<&str> = read.assess_fixes().map(|pair| pair.id.as_str()).collect();
    let mut written = Vec::new();
    for turn in read.turns() {
        for pair in pairs(turn) {
            if known.contains(&pair.id.as_str()) {
                continue;
            }
            store.append(&Line::assess_fix(pair.clone()))?;
            written.push(pair.id);
        }
    }
    Ok(written)
}
