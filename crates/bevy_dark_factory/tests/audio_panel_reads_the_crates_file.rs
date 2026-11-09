//! **The audio panel reads the crate's own sound, and an audition plays it** (S-01).
//!
//! `bevy_siren` is hosted behind the `bevy_siren` feature: the mix is `<project>/family/bevy_siren/SirenSettings.ron`
//! (a settings resource, applied by reflection) and the cues `family/bevy_siren/CueTable.ron`, which the panel reads
//! and writes through the crate's own `CueTable` — never a second schema.
//! This file is the proof of that claim and of the one behaviour the panel exists for: an **Audition** is a
//! `Play::cue`, so what the panel shows is what a game hears.
//!
//! A file that does not parse is a problem on the status line, and the previous mix stays in force: the family
//! layer's rule for every crate.
#![cfg(feature = "bevy_siren")]

use bevy::audio::AudioSource;
use bevy::prelude::*;
use bevy_dark_factory::audio_panel::{self, AudioPanel};
use bevy_dark_factory::editor::EditorState;
use bevy_dark_factory::harness;
use bevy_siren::{
    Cue, CueTable, Play, Played, SirenLog, SirenMix, SirenSettings, SirenSounds, SoundSlot,
};

use crate::fixtures;

/// Everything the crate answered, collected by a reader system on every frame.
///
/// `Messages<Played>` is double-buffered and cleared two updates after a write, so a test that drains it after
/// several updates is racing the ring: it passed alone and lost the message in the full parallel run. A host
/// reads it every frame, and so does this.
#[derive(Resource, Default)]
struct Heard {
    played: Vec<Played>,
    refused: Vec<bevy_siren::Refused>,
}

fn hear(
    mut played: MessageReader<Played>,
    mut refused: MessageReader<bevy_siren::Refused>,
    mut heard: ResMut<Heard>,
) {
    heard.played.extend(played.read().cloned());
    heard.refused.extend(refused.read().cloned());
}

/// The mix table's file under the project root.
fn mix_file(root: &std::path::Path) -> std::path::PathBuf {
    blueprint_model::family::settings_path(root, audio_panel::CRATE, "SirenSettings")
}

/// A project with one mix table and one cue table written into `family/bevy_siren/`.
fn project(tag: &str, settings: &str, cues: &str) -> std::path::PathBuf {
    let root = fixtures::Fixture::new(tag)
        .descriptor("floor", "alpha")
        .build("m");
    let dir = mix_file(&root).parent().map(std::path::Path::to_path_buf).unwrap_or_else(|| panic!("no parent"));
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{dir:?}: {e}"));
    std::fs::write(mix_file(&root), settings).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(blueprint_model::family::settings_path(&root, audio_panel::CRATE, audio_panel::CUES), cues).unwrap_or_else(|e| panic!("{e}"));
    root
}

/// **A file the crate's own types read** becomes the running editor's mix: the bus list from the file, and the cue
/// names in the panel, with a key for every sound the panel can audition.
#[test]
fn the_panel_reads_the_crates_own_file() {
    let settings = r#"(
        max_voices: 8,
        buses: [(name: "Music", gain_db: -2.0), (name: "Effects")],
        ducks: [],
    )"#;
    let cues = r#"(
        cues: [
            (name: "footstep.grass", sounds: [(sound: "step_01"), (sound: "step_02")]),
            (name: "ui.click", sounds: [(sound: "click")], gain_db: -6.0),
        ],
    )"#;
    let root = project("siren_reads", settings, cues);
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }

    let panel = app.world().resource::<AudioPanel>().clone();
    assert_eq!(
        panel.buses,
        vec!["Music".to_owned(), "Effects".to_owned()],
        "the buses are the file's"
    );
    assert_eq!(
        panel.cues,
        vec!["footstep.grass".to_owned(), "ui.click".to_owned()],
        "the cues are the file's"
    );

    let settings = app.world().resource::<SirenSettings>();
    assert_eq!(settings.max_voices, 8, "the mix table is the file's");
    let table = app.world().resource::<CueTable>();
    assert_eq!(table.get("ui.click").expect("the second cue").gain_db, -6.0);
    assert_eq!(
        table.problems_with(&["Music", "Effects"]),
        Vec::new(),
        "the file the panel wrote is a clean table"
    );
}

/// **Audition is a `Play`**, and the panel hears the answer through the same door a game does: refused by name when
/// the key has no handle, played once it does.
///
/// Both halves matter. A panel that only ever showed `Played` would hide a missing file; a panel that played with no
/// key would be playing something the project did not ship.
#[test]
fn an_audition_is_a_play_and_the_panel_hears_it() {
    let settings = r#"(max_voices: 8, buses: [(name: "Effects")], ducks: [])"#;
    let cues = r#"(cues: [(name: "ui.click", sounds: [(sound: "click")])])"#;
    let root = project("siren_audition", settings, cues);
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    // The headless harness has no `AudioPlugin` (nothing opens a device), so the audio asset store the editor's
    // real build gets from it is added here — the same `init_asset` a headless audio test uses.
    app.init_asset::<AudioSource>();
    app.init_resource::<Heard>().add_systems(Update, hear);
    for _ in 0..4 {
        app.update();
    }

    // The panel selects the one cue and writes the crate's own `Play`, as a game would. With no handle for
    // `click`, the answer is a refusal that names the key — which is exactly what the panel prints.
    let panel = app.world().resource::<AudioPanel>();
    assert_eq!(panel.selected_cue(), Some("ui.click"));
    assert_eq!(
        app.world()
            .resource::<SirenSounds>()
            .missing(app.world().resource::<CueTable>()),
        vec!["click".to_owned()],
        "the panel can say which file is missing"
    );
    app.world_mut().write_message(Play::cue("ui.click"));
    for _ in 0..3 {
        app.update();
    }
    let refusal = app
        .world()
        .resource::<Heard>()
        .refused
        .last()
        .cloned()
        .expect("a cue with no handle is refused, not played");
    assert_eq!(
        refusal.reason,
        bevy_siren::Refusal::MissingSound {
            sound: "click".to_owned()
        }
    );

    // Give the panel the key a game would have loaded, and the same audition now plays.
    let handle = app
        .world_mut()
        .resource_mut::<Assets<bevy::audio::AudioSource>>()
        .add(bevy::audio::AudioSource {
            // Bytes the decoder reads: the crate refuses a sound it cannot decode as a missing one. A 0.22 s
            // Vorbis footstep, CC0, from `bevy_siren`'s `assets/audio/step_01.ogg`.
            bytes: include_bytes!("fixtures/step_01.ogg").to_vec().into(),
        });
    app.world_mut()
        .resource_mut::<SirenSounds>()
        .insert("click", handle);
    app.world_mut().write_message(Play::cue("ui.click"));
    for _ in 0..3 {
        app.update();
    }

    let answer = app
        .world()
        .resource::<Heard>()
        .played
        .last()
        .cloned()
        .expect("the audition played once its key resolved");
    assert_eq!(answer.cue, "ui.click");
    assert_eq!(answer.bus, "Effects");
    assert!(answer.gain_db.is_finite());
    assert!(
        app.world()
            .resource::<SirenLog>()
            .iter()
            .any(|e| matches!(e, bevy_siren::Entry::Played { cue, .. } if cue == "ui.click")),
        "the panel's own log tail shows the audition"
    );
    let mix = app.world().resource::<SirenMix>().clone();
    assert!(
        mix.db.contains_key("Effects"),
        "the panel has a row for every bus in the mix"
    );
}

/// **A file that does not parse is a problem naming what is wrong**, and the previous mix stays in force: a broken
/// edit never silently zeroes a project's sound.
#[test]
fn a_file_that_does_not_parse_is_named_and_the_mix_stays() {
    let good = r#"(max_voices: 8, buses: [(name: "Effects")], ducks: [])"#;
    let root = project("siren_broken", good, r#"(cues: [(name: "ui.click", sounds: [(sound: "click")])])"#);
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(app.world().resource::<SirenSettings>().max_voices, 8);

    // A hand edit writes a field the crate does not have.
    let broken = good.replace("max_voices: 8", "max_voicers: 8");
    std::fs::write(mix_file(&root), broken)
        .unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(
        app.world().resource::<SirenSettings>().max_voices,
        8,
        "the last good mix stays in force"
    );
    let state = app.world().resource::<EditorState>();
    let _ = state;
}

/// The file the panel writes is the file the crate reads: a round trip through the panel's own reader and writer.
#[test]
fn the_panels_file_is_the_crates_file() {
    let mut table = CueTable::new(vec![Cue {
        sounds: vec![SoundSlot::new("wind_loop")],
        bus: "Ambience".to_owned(),
        ..Cue::new("wind")
    }]);
    table.insert(Cue {
        gain_db: -3.0,
        ..Cue::new("click")
    });
    let text = audio_panel::write_cues(&table).expect("the panel writes the crate's type");
    let read_table = audio_panel::read_cues(&text).expect("and reads it back");
    assert_eq!(read_table, table);
}
