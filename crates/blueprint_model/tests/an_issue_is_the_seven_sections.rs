//! **A capture's issue body says the seven sections, with the note verbatim and both pictures embedded**
//! (#495).
//!
//! The fleet reads GitHub issues. A capture that is not an issue does not exist to it — on 2026-11-15
//! thirty captures sat `unread` in the ledger, and each hand-filing is one careful pass of reading the
//! record, cropping the pick, measuring the geometry and writing the issue.
//!
//! `docs/loop/captures/ISSUE-FORMAT.md` is the contract, and this is its engine-free half:
//! `feedback::issue::body` turns a real record into the seven sections, and `feedback::issue::title`
//! names the defect and the capture. Both are pure, so the format is tested without a window and an
//! agent can preview it.

use blueprint_model::feedback::{
    self, Audience, CameraPose, Element, PathStep, Pick, Picture, Report, Scene, Subject, WindowRecord,
};

/// **A record as the editor writes one**: a project report from the Map door, an element pick over the
/// terminal, and the note the capture in the issue carried.
fn report() -> Report {
    Report {
        version: feedback::FEEDBACK_VERSION,
        stem: "feedback-1794921159460".to_owned(),
        taken_at_ms: 1_794_921_159_460,
        audience: Audience::Developers,
        app_version: "0.7.0".to_owned(),
        screen: "editor".to_owned(),
        project: Some("/Users/ladvien/Documents/DarkFactory Projects/lavender_hills".to_owned()),
        map: Some("liches_lair".to_owned()),
        tool: Some("Place".to_owned()),
        window: WindowRecord {
            physical: [2560, 1406],
            logical: [1280.0, 703.0],
            scale_factor: 2.0,
            ui_scale: 2.4,
        },
        picks: vec![
            Pick {
                subject: Subject::Element(Element {
                    entity: 8_589_933_257,
                    rect_px: [0.0, 1007.0, 2560.0, 1542.0],
                    path: vec![
                        PathStep { name: None, role: Some("Sized".to_owned()), control: None, text: None },
                        PathStep { name: Some("terminal".to_owned()), role: Some("Sized".to_owned()), control: None, text: None },
                    ],
                }),
                at: [1465.6094, 934.3125],
            },
            Pick {
                subject: Subject::Scene(Scene {
                    world: [47.497528, 0.0, 44.532257],
                    map: Some([47.497528, 44.532257]),
                    cell: Some([47, 44]),
                    placement: None,
                    camera: CameraPose {
                        translation: [73.07608, 32.0, 73.052185],
                        forward: [-0.5773503, -0.5773503, -0.57735026],
                    },
                }),
                at: [600.0, 400.0],
            },
        ],
        relations: Vec::new(),
        note: "There's something cutting off the edge of the map as I use WASD to navigate.  Maybe camera not in right place or incorrect culling?".to_owned(),
        picture: Picture { file: feedback::picture_file("feedback-1794921159460"), size: [2560, 1406] },
        picks_picture: Some(Picture {
            file: feedback::picks_picture_file("feedback-1794921159460"),
            size: [2560, 1406],
        }),
        log: Some(feedback::log_file("feedback-1794921159460")),
    }
}

/// The issue as it is filed: the body and the title, from the record alone.
#[test]
fn the_body_carries_the_seven_sections_with_the_note_verbatim_and_both_pictures() {
    let body = feedback::issue::body(&report());

    // 1. Provenance, first line, read from the record.
    let first = body.lines().next().unwrap_or_default();
    assert!(first.starts_with("**From the owner's capture, `feedback-1794921159460`**"), "the first line is the provenance: {first}");
    for said in ["DarkFactory 0.7.0", "1280", "`ui_scale` 2.4", "lavender_hills", "liches_lair"] {
        assert!(first.contains(said), "the provenance reads `{said}`: {first}");
    }

    // 3. The note, verbatim, in a blockquote of its own.
    let note = &report().note;
    let quoted: String = note.lines().map(|l| format!("> {l}\n")).collect();
    assert!(body.contains(quoted.trim_end()), "the note is quoted verbatim:\n{body}");

    // 2. Both pictures, embedded on the repository's raw URLs.
    let picks = feedback::issue::raw_url("main", "feedback-1794921159460", &feedback::picks_picture_file("feedback-1794921159460"));
    let frame = feedback::issue::raw_url("main", "feedback-1794921159460", &feedback::picture_file("feedback-1794921159460"));
    assert!(body.contains(&format!("![the thing the note points at]({picks})")), "the picked picture is embedded:\n{body}");
    assert!(body.contains(&format!("Frame: ![whole window]({frame})")), "and the frame beneath it:\n{body}");

    // 4. Measured facts: the element's rect and path, and the scene pick's cell and camera.
    for said in ["rect_px", "1007", "terminal", "cell [47, 44]", "73.08", "-0.577"] {
        assert!(body.contains(said), "the measured facts carry `{said}`:\n{body}");
    }

    // 5–7. The three prose sections are there and in order.
    let order: Vec<usize> = ["## Current behaviour", "## Wanted", "## Acceptance"]
        .iter()
        .map(|h| body.find(h).unwrap_or_else(|| panic!("{h} is not in the body:\n{body}")))
        .collect();
    assert!(order.windows(2).all(|w| w[0] < w[1]), "the sections keep their order: {order:?}");
}

/// **Both pictures answer at the URL the body embeds**, which is the one rule that cannot break: an
/// issue whose pictures 404 is worse than no issue.
#[test]
fn the_two_pictures_are_the_ones_the_body_links() {
    let report = report();
    let body = feedback::issue::body(&report);
    let picks = feedback::issue::raw_url("main", &report.stem, &report.picks_picture.unwrap_or_else(|| panic!("a pick picture")).file);
    let frame = feedback::issue::raw_url("main", &report.stem, &report.picture.file);
    assert!(picks.starts_with("https://raw.githubusercontent.com/dark-incorporated/bevy_dark_factory/main/docs/loop/captures/feedback-1794921159460/"));
    assert!(picks.ends_with("feedback-1794921159460_picks.png"));
    assert!(body.contains(&picks) && body.contains(&frame));
}

/// **The title names the defect and the capture**, as the format asks.
#[test]
fn the_title_names_the_capture() {
    let title = feedback::issue::title(&report());
    assert!(title.contains("feedback-1794921159460"), "the title names the capture: {title}");
    assert!(!title.trim().is_empty());
}
