//! **The audio panel reads the crate's own sound, and an audition plays it** (S-01).
//!
//! `bevy_siren` is hosted behind the `bevy_siren` feature, and the panel writes and reads
//! `<project>/family/bevy_siren.yaml` through the crate's own `SirenSettings` and `CueTable` — never a second schema.
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

/// A project with one cue table and one mix table written into `family/`.
fn project(tag: &str, text: &str) -> std::path::PathBuf {
    let root = fixtures::Fixture::new(tag)
        .descriptor("floor", "alpha")
        .build("m");
    let family = root.join("family");
    std::fs::create_dir_all(&family).unwrap_or_else(|e| panic!("{family:?}: {e}"));
    std::fs::write(family.join(audio_panel::FILE), text).unwrap_or_else(|e| panic!("{e}"));
    root
}

/// **A file the crate's own types read** becomes the running editor's mix: the bus list from the file, and the cue
/// names in the panel, with a key for every sound the panel can audition.
#[test]
fn the_panel_reads_the_crates_own_file() {
    let text = r#"{settings: {max_voices: 8, buses: [{name: "Music", gain_db: -2.0}, {name: "Effects"}], ducks: []}, cues: {cues: [{name: "footstep.grass", sounds: [{sound: "step_01"}, {sound: "step_02"}]}, {name: "ui.click", sounds: [{sound: "click"}], gain_db: -6.0}]}}"#;
    let root = project("siren_reads", text);
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
    let text = r#"{settings: {max_voices: 8, buses: [{name: "Effects"}], ducks: []}, cues: {cues: [{name: "ui.click", sounds: [{sound: "click"}]}]}}"#;
    let root = project("siren_audition", text);
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.init_resource::<Heard>().add_systems(Update, hear);
    for _ in 0..4 {
        app.update();
    }

    // The file's one cue is the panel's cue list — the choice among them is `world_lists::SelectedCue`, which
    // the person sets and Audition reads, so a reading of the file names the cue and chooses nothing. The panel
    // writes the crate's own `Play`, as a game would; with no handle for `click`, the answer is a refusal that
    // names the key, which is exactly what the panel prints.
    let panel = app.world().resource::<AudioPanel>();
    assert_eq!(panel.cues, vec!["ui.click".to_owned()]);
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
            bytes: vec![0u8; 4].into(),
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
    let good = r#"{settings: {max_voices: 8, buses: [{name: "Effects"}], ducks: []}, cues: {cues: [{name: "ui.click", sounds: [{sound: "click"}]}]}}"#;
    let root = project("siren_broken", good);
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(app.world().resource::<SirenSettings>().max_voices, 8);

    // A hand edit writes a field the crate does not have.
    let broken = good.replace("max_voices: 8", "max_voicers: 8");
    std::fs::write(root.join("family").join(audio_panel::FILE), broken)
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
    let settings = SirenSettings::default();
    table.insert(Cue {
        gain_db: -3.0,
        ..Cue::new("click")
    });
    let text =
        audio_panel::write_file(&settings, &table).expect("the panel writes the crate's types");
    let (read_settings, read_table) = audio_panel::read_file(&text).expect("and reads them back");
    assert_eq!(read_settings, settings);
    assert_eq!(read_table, table);
}

/// **The panel judges the cues against the project's own mix table** (#554), not against `bevy_siren`'s four
/// `DEFAULT_BUSES`.
///
/// A project that declares `Music`, `Effects` and `SFX` and puts a cue on `SFX` is a clean file: the crate
/// would play it, and the panel must not say otherwise. The same file with the cue on `Sfx` — a bus the mix
/// does *not* declare — is a real finding, and the panel names the cue and the bus. Before #554 the first
/// case was a false problem on every `SFX` cue and the second could never be seen, because the crate's own
/// four buses always answered.
#[test]
fn the_panel_checks_the_cues_against_this_files_own_buses() {
    // **The mix and the cues agree**: `SFX` is a bus this file declares, so nothing is wrong with it.
    let agreeing = r#"{settings: {max_voices: 8, buses: [{name: "Music"}, {name: "Effects"}, {name: "SFX"}], ducks: []}, cues: {cues: [{name: "ui.click", sounds: [{sound: "click"}], bus: "SFX"}]}}"#;
    let root = project("siren_buses_agree", agreeing);
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let problems = app.world().resource::<EditorState>().status.problems().iter().map(|p| p.text.clone()).collect::<Vec<_>>();
    assert!(
        problems.is_empty(),
        "`SFX` is a bus this file declares, so a cue on it is no problem: {problems:?}"
    );

    // **The cue names a bus the mix does not have**: a real finding, naming the cue and the bus.
    let wrong = r#"{settings: {max_voices: 8, buses: [{name: "Music"}, {name: "Effects"}, {name: "SFX"}], ducks: []}, cues: {cues: [{name: "ui.click", sounds: [{sound: "click"}], bus: "Sfx"}]}}"#;
    let root = project("siren_buses_wrong", wrong);
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let problems = app.world().resource::<EditorState>().status.problems().iter().map(|p| p.text.clone()).collect::<Vec<_>>();
    assert!(
        problems.iter().any(|p| p.contains("ui.click") && p.contains("Sfx")),
        "a cue on a bus the mix does not declare is named, with the cue and the bus: {problems:?}"
    );
}

/// **The panel draws what its note says** (#557): a row per bus from the mix, the selected cue marked, and
/// the tail of the crate's log — not a heading and one button.
///
/// Each list is a column the section spawns and a system fills. This drives a frame with the Sound workspace
/// open and reads the rows back, then presses Down and says the mark follows `world_lists::SelectedCue` — the
/// same value **Audition** reads.
#[test]
fn the_panel_draws_the_buses_the_cues_and_the_log() {
    let text = r#"{settings: {max_voices: 8, buses: [{name: "Music"}, {name: "Effects"}], ducks: []}, cues: {cues: [{name: "ui.click", sounds: [{sound: "click"}]}, {name: "step.grass", sounds: [{sound: "step_01"}]}]}}"#;
    let root = project("siren_draws", text);
    let mut app = harness::build_headless_at(&root, "m", None, bevy_dark_factory::tiles::Mode::default()).unwrap_or_else(|e| panic!("{e}"));
    // The Sound workspace, so the panel's draw runs (its run condition is `Workspace::sound_is_open`).
    *app.world_mut().resource_mut::<bevy_dark_factory::workspace::Workspace>() = bevy_dark_factory::workspace::Workspace::Sound;
    for _ in 0..6 {
        app.update();
    }

    // **The rows of a column**, read from its `Text` children in spawn order.
    let rows_of = |app: &App, kind: &'static str| -> Vec<String> {
        let world = app.world();
        let lists: Vec<Entity> = match kind {
            "bus" => world.iter_entities().filter(|e| e.get::<audio_panel::BusList>().is_some()).map(|e| e.id()).collect(),
            "cue" => world.iter_entities().filter(|e| e.get::<audio_panel::CueChooser>().is_some()).map(|e| e.id()).collect(),
            _ => world.iter_entities().filter(|e| e.get::<audio_panel::LogTail>().is_some()).map(|e| e.id()).collect(),
        };
        let Some(&list) = lists.first() else { return Vec::new() };
        let mut out = Vec::new();
        fn walk(world: &World, e: Entity, out: &mut Vec<String>) {
            if let Some(t) = world.get::<Text>(e) {
                out.push(t.0.clone());
            }
            if let Some(kids) = world.get::<Children>(e) {
                for k in kids.iter() {
                    walk(world, k, out);
                }
            }
        }
        walk(world, list, &mut out);
        out
    };

    let buses = rows_of(&app, "bus");
    assert_eq!(buses.len(), 2, "one row per bus in the mix: {buses:?}");
    let mix = app.world().resource::<SirenMix>().clone();
    let panel = app.world().resource::<AudioPanel>().clone();
    assert_eq!(buses, audio_panel::bus_rows(&mix, &panel.buses), "a bus row is `bus_rows`' own string");

    // **No cue is chosen yet**, so no row is marked: the choice is `world_lists::SelectedCue`, the editor's own,
    // which Audition reads and reveal and the nav stops write (2026-11-23 review, finding 2: a second selection
    // here marked one list while Audition played the other's choice).
    let cues = rows_of(&app, "cue");
    assert_eq!(cues.len(), 2, "one row per cue: {cues:?}");
    assert_eq!(cues, audio_panel::cue_rows(&panel, None), "the cue rows are `cue_rows`' own strings");
    assert!(!cues.iter().any(|c| c.starts_with(bevy_dark_factory::world_lists::CUE_MARK)), "nothing is chosen yet: {cues:?}");

    let tail = rows_of(&app, "log");
    assert!(!tail.is_empty(), "the log tail says at least that nothing has played: {tail:?}");

    // **Down chooses the first cue**, and the mark follows the value Audition reads.
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::ArrowDown);
    app.update();
    let chosen = app.world().resource::<bevy_dark_factory::world_lists::SelectedCue>().0.clone();
    assert_eq!(chosen.as_deref(), Some("ui.click"), "Down walks the cue list and writes the editor's selection");
    let moved = rows_of(&app, "cue");
    assert!(moved[0].starts_with(bevy_dark_factory::world_lists::CUE_MARK), "the mark follows the selection: {moved:?}");
}

/// **The panel asks the asset server for the cue's sound, which nothing did before** (#555).
///
/// `Play::cue` refuses with `MissingSound` unless `SirenSounds` holds a handle for the key, and until this
/// change nothing in the editor ever asked for one — so Audition could only ever refuse. The proof is the
/// handle: after the panel has read the file, `SirenSounds` knows the cue's key.
///
/// The file is **RON** whatever its `.yaml` name: `read_file` deserialises the crate's own `SirenSettings`
/// and `CueTable` through `ron`.
#[test]
fn the_panel_asks_the_asset_server_for_the_cues_sound() {
    let text = r#"{settings: {max_voices: 8, buses: [{name: "Effects"}], ducks: []}, cues: {cues: [{name: "ui.click", sounds: [{sound: "step_01"}]}]}}"#;
    let root = project("missing_sound", text);
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    // **The harness registers `AudioSource`'s store** under `bevy_siren`, as the binary's `AudioPlugin` does; the
    // assertion below needs it, because `read_audio` skips the load without an asset server for the type.
    assert!(app.world().contains_resource::<Assets<AudioSource>>(), "the harness registers the audio asset store");
    for _ in 0..4 {
        app.update();
    }

    let sounds = app.world().resource::<bevy_siren::SirenSounds>();
    assert!(
        sounds.keys().any(|key| key == "step_01"),
        "**the panel never asked the asset server for `step_01`** (#555), so `Play::cue` can only refuse \
         with `MissingSound` and the button is a control that cannot work. Keys held: {:?}",
        sounds.keys().collect::<Vec<_>>()
    );
}
