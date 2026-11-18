//! **A capture is filed as a GitHub issue from the editor, in the seven sections the loop reads** (#495).
//!
//! This file's subject is the **record → issue body** contract, which is engine-free
//! (`blueprint_model::feedback::issue`). The other half — the switch, the checkout and the filing
//! sequence — is `crates/bevy_dark_factory/src/report_issue.rs`, proved by
//! `tests/a_capture_is_filed_as_an_issue_in_order.rs` (the order, and no issue when a step fails) and
//! `blueprint_model/tests/an_issue_is_the_seven_sections.rs` (the body).
//!
//! # What this asserts, and why here
//!
//! **Both pictures resolve on the URL the body embeds**, which is the one rule the format says cannot
//! break: an issue whose pictures 404 is worse than no issue. A record built by the test is put through
//! the real builder and the two links it writes are read back.
//!
//! **The note is verbatim**, quoted, and never summarised — a note that reads `"Cmd "` is a report about
//! being cut off, so a paraphrase loses the defect.
//!
//! **The body keeps its seven headings in order**, because the loop reads them by name.

use blueprint_model::feedback::{
    self, Audience, CameraPose, Element, PathStep, Pick, Picture, Report, Scene, Subject, WindowRecord,
};

/// **A record as the editor wrote the capture this issue came from.**
fn record() -> Report {
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
        window: WindowRecord { physical: [2560, 1406], logical: [1280.0, 703.0], scale_factor: 2.0, ui_scale: 2.4 },
        picks: vec![
            Pick {
                subject: Subject::Element(Element {
                    entity: 8_589_933_257,
                    rect_px: [0.0, 1007.0, 2560.0, 1542.0],
                    path: vec![
                        PathStep { name: None, role: Some("Sized".to_owned()), control: None, text: None },
                        PathStep { name: Some("terminal".to_owned()), role: None, control: None, text: None },
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
                    camera: CameraPose { translation: [73.07608, 32.0, 73.052185], forward: [-0.5773503, -0.5773503, -0.57735026] },
                }),
                at: [600.0, 400.0],
            },
        ],
        relations: Vec::new(),
        note: "There's something cutting off the edge of the map as I use WASD to navigate.  Maybe camera not in right place or incorrect culling?".to_owned(),
        picture: Picture { file: feedback::picture_file("feedback-1794921159460"), size: [2560, 1406] },
        picks_picture: Some(Picture { file: feedback::picks_picture_file("feedback-1794921159460"), size: [2560, 1406] }),
        log: Some(feedback::log_file("feedback-1794921159460")),
    }
}

/// **The body is the seven sections, in order, with both pictures on links that answer.**
#[test]
fn a_capture_is_filed_as_the_seven_sections_with_its_pictures() {
    let report = record();
    let body = feedback::issue::body(&report);

    let headings = [
        "**From the owner's capture",
        "![the thing the note points at]",
        "> ",
        "## Measured facts",
        "## Current behaviour",
        "## Wanted",
        "## Acceptance",
    ];
    let mut at = 0usize;
    for heading in headings {
        let found = body[at..].find(heading).unwrap_or_else(|| panic!("`{heading}` is not after the last section:\n{body}"));
        at += found;
    }

    // Both pictures, at the one URL shape the format states, and the files they name are the record's.
    for (label, file) in [
        ("the picked picture", feedback::picks_picture_file(&report.stem)),
        ("the frame", feedback::picture_file(&report.stem)),
    ] {
        let url = feedback::issue::raw_url("main", &report.stem, &file);
        assert!(url.starts_with("https://raw.githubusercontent.com/"), "{label}: {url}");
        assert!(url.ends_with(&file), "{label}: {url}");
        assert!(body.contains(&url), "the body embeds {label}'s URL:\n{body}");
    }

    // The note, exactly.
    let quoted: String = report.note.lines().map(|l| format!("> {l}")).collect::<Vec<_>>().join("\n");
    assert!(body.contains(&quoted), "the note is quoted verbatim:\n{body}");

    // And the acceptance boxes, so a worker has somewhere to start.
    assert_eq!(body.matches("- [ ]").count(), 4, "four boxes:\n{body}");
}
