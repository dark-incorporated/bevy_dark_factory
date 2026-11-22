//! **The WATER, FOREST, CUES, CUE USES and LIGHTS lists read the project index, and keep no clock of their own** (#501, checklist item 10;
//! `world_lists::READ_SECS` is retired). The two halves every such test has are in `index_waits`; what the lists say is checked against
//! files the test wrote (counted with plain `std::fs` reads) and numbers it chose. **A change to a file none of the lists shows moves the
//! index and reads nothing** (`world_lists::reads_on_this_thread`), a list a door change respawned is filled again with no read, and
//! choosing a cue re-marks the rows without reading the sound file.

use std::path::PathBuf;
use std::time::Duration;

use bevy::prelude::*;
use bevy_dark_factory::project::{OpenMap, Project};
use bevy_dark_factory::workspace::Workspace;
use bevy_dark_factory::world_lists::{self, CueList, CueUses, ForestList, LightsList, SelectedCue, WaterList};

use crate::fixtures;
use crate::index_waits::*;


const FOREST_OF_THREE: &str = "\
version: 1
trees:
- {id: 0, species: oak, at: [3.0, 4.0], ground_m: 0.0, yaw: 0.0, scale: 1.0, variant: 0}
- {id: 1, species: oak, at: [5.0, 4.0], ground_m: 0.0, yaw: 0.0, scale: 1.0, variant: 1}
- {id: 2, species: pine, at: [7.0, 4.0], ground_m: 0.0, yaw: 0.0, scale: 1.0, variant: 0}
";

const FOREST_OF_ONE: &str = "\
version: 1
trees:
- {id: 0, species: birch, at: [3.0, 4.0], ground_m: 0.0, yaw: 0.0, scale: 1.0, variant: 0}
";

const WATER_WITH_A_SPRING: &str = "\
version: 1
springs:
- {x: 3.0, z: 4.0, rate_m3_s: 2.0, radius_m: 5.0}
";

const WATER_WITH_A_SINK: &str = "\
version: 1
sinks:
- {x: 6.0, z: 7.0, rate_m3_s: 1.0, radius_m: 2.0}
";

const TWO_CUES: &str = "cues:\n  cues:\n  - name: alarm\n  - name: bell\n";
const ONE_CUE: &str = "cues:\n  cues:\n  - name: drum\n";

struct Lists {
    root: PathBuf,
    forest: PathBuf,
    water: PathBuf,
    cues: PathBuf,
    unrelated: PathBuf,
}

fn lists_project(tag: &str) -> Lists {
    let root = fixtures::Fixture::new(&format!("w501c_lists_{tag}_{}", std::process::id())).descriptor("stool", "alpha").place("stool", (0.0, 0.0)).build("m");
    let blueprint = Project::open(&root, None).unwrap_or_else(|e| panic!("{e}")).blueprint_dir;
    let lists = Lists {
        forest: blueprint.join("forest/m.yaml"),
        water: blueprint.join("water/m.yaml"),
        cues: root.join("family/bevy_siren.yaml"),
        unrelated: root.join("notes/unrelated.yaml"),
        root,
    };
    for (file, text) in [(&lists.forest, FOREST_OF_THREE), (&lists.water, WATER_WITH_A_SPRING), (&lists.cues, TWO_CUES), (&lists.unrelated, "note: first\n")] {
        let dir = file.parent().unwrap_or_else(|| panic!("{file:?} has no folder"));
        std::fs::create_dir_all(dir).unwrap_or_else(|e| panic!("{dir:?}: {e}"));
        write_in_place(file, text);
    }
    lists
}

/// **The WATER and FOREST lists follow their files through the index; a change to a file none of the lists shows reads nothing; a list a
/// door change respawned is filled again with no read; the LIGHTS list follows the map and reads no file.**
#[test]
fn the_world_lists_follow_their_files_and_read_nothing_for_a_file_they_do_not_show() {
    let l = lists_project("world");
    let mut app = stepped(&l.root);
    open_workspace(&mut app, "workspace.world", Workspace::World);
    sweep(&app, SWEEP_NEVER);

    let trees = std::fs::read_to_string(&l.forest).unwrap_or_default().matches("- {id:").count();
    assert_eq!(trees, 3, "the forest file on disk holds three trees");
    assert_eq!(texts_under::<ForestList>(&mut app), [format!("{trees} tree(s)"), "oak: 2".to_owned(), "pine: 1".to_owned()]);
    assert_eq!(texts_under::<WaterList>(&mut app), ["spring at (3, 4): 2 m3/s, radius 5 m"], "the water plan's one spring, by the numbers the test wrote");
    assert_eq!(texts_under::<LightsList>(&mut app), [world_lists::NO_LIGHTS]);
    let read_to_fill = world_lists::reads_on_this_thread();

    // **A change to a file none of the lists shows moves the index and reads nothing.**
    let before = published(&app);
    write_in_place(&l.unrelated, "note: second, and longer\n");
    announce(&l.root, &l.unrelated);
    assert!(pump_until(&mut app, SHOWN_WITHIN, |app| published(app) > before).is_some(), "the index did not publish the unrelated change, so a read count of nothing would prove nothing");
    pump_for(&mut app, Duration::from_millis(300));
    assert_eq!(world_lists::reads_on_this_thread(), read_to_fill, "a change to a file no list shows made a list read a file");

    // **The forest file: no clock, then the announcement, and exactly one read.**
    write_in_place(&l.forest, FOREST_OF_ONE);
    pump_for(&mut app, LONGER_THAN_THE_OLD_CLOCKS);
    assert_eq!(texts_under::<ForestList>(&mut app)[0], "3 tree(s)", "the forest list changed with no word from the index: a clock is reading the file");
    assert_eq!(world_lists::reads_on_this_thread(), read_to_fill, "a list read its file with no word from the index");
    announce(&l.root, &l.forest);
    let one_tree = std::fs::read_to_string(&l.forest).unwrap_or_default().matches("- {id:").count();
    let shown = pump_until(&mut app, SHOWN_WITHIN, |app| texts_under::<ForestList>(app) == [format!("{one_tree} tree(s)"), "birch: 1".to_owned()]);
    assert!(shown.is_some(), "the announced forest was not shown within {SHOWN_WITHIN:?}: {:?}", texts_under::<ForestList>(&mut app));
    assert_eq!(world_lists::reads_on_this_thread(), read_to_fill + 1, "one changed file is one read: the water plan was not read again");
    assert_eq!(texts_under::<WaterList>(&mut app), ["spring at (3, 4): 2 m3/s, radius 5 m"], "the water list is untouched by a forest change");

    // **The water plan, the same way.**
    write_in_place(&l.water, WATER_WITH_A_SINK);
    announce(&l.root, &l.water);
    let shown = pump_until(&mut app, SHOWN_WITHIN, |app| texts_under::<WaterList>(app) == ["sink at (6, 7): 1 m3/s, radius 2 m"]);
    assert!(shown.is_some(), "the announced water plan was not shown within {SHOWN_WITHIN:?}: {:?}", texts_under::<WaterList>(&mut app));
    assert_eq!(world_lists::reads_on_this_thread(), read_to_fill + 2, "the water change is one more read, and the forest was not read again");

    // **A list a door change respawned has no children and is filled again from the rows held: no read.**
    let forest_list = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<ForestList>>().single(world).expect("one forest list")
    };
    app.world_mut().entity_mut(forest_list).despawn_related::<Children>();
    app.update();
    assert_eq!(texts_under::<ForestList>(&mut app), ["1 tree(s)", "birch: 1"], "the respawned list is filled again");
    assert_eq!(world_lists::reads_on_this_thread(), read_to_fill + 2, "filling a respawned list read a file");

    // **The lights follow the map on the frame it changes, and read no file.**
    let id = {
        let mut open = app.world_mut().resource_mut::<OpenMap>();
        let placed = open.map.placements.first_mut().expect("the fixture places a stool");
        placed.family.insert(world_lists::LIGHTS_KEY.to_owned(), "intensity:   3".to_owned());
        placed.id.clone()
    };
    app.update();
    assert_eq!(texts_under::<LightsList>(&mut app), [format!("{id}: intensity: 3")]);
    assert_eq!(world_lists::reads_on_this_thread(), read_to_fill + 2, "the lights read a file");
}

/// **The CUES and CUE USES lists follow the sound file through the index; choosing a cue re-marks the rows and reads nothing.**
#[test]
fn the_sound_lists_follow_the_sound_file_and_a_chosen_cue_reads_nothing() {
    let l = lists_project("sound");
    let mut app = stepped(&l.root);
    open_workspace(&mut app, "workspace.sound", Workspace::Sound);
    sweep(&app, SWEEP_NEVER);
    assert_eq!(texts_under::<CueList>(&mut app), ["alarm", "bell"], "the cues the test wrote");
    assert!(texts_under::<CueUses>(&mut app).is_empty(), "with no cue chosen there is nothing to say about its uses");
    let read_to_fill = world_lists::reads_on_this_thread();

    // **Choosing a cue is shown on the frame it is made, and the file is not read again.**
    app.world_mut().resource_mut::<SelectedCue>().0 = Some("alarm".to_owned());
    app.update();
    assert_eq!(texts_under::<CueList>(&mut app), [format!("{}alarm", world_lists::CUE_MARK), "bell".to_owned()]);
    let uses = texts_under::<CueUses>(&mut app);
    assert!(!uses.is_empty() && uses.iter().all(|row| row.contains("alarm")), "the uses list names the chosen cue: {uses:?}");
    app.world_mut().resource_mut::<SelectedCue>().0 = Some("bell".to_owned());
    app.update();
    assert_eq!(texts_under::<CueList>(&mut app), ["alarm".to_owned(), format!("{}bell", world_lists::CUE_MARK)]);
    assert!(texts_under::<CueUses>(&mut app).iter().all(|row| row.contains("bell")), "the uses list follows the choice");
    assert_eq!(world_lists::reads_on_this_thread(), read_to_fill, "choosing a cue read the sound file");

    // **The sound file: no clock, then the announcement.**
    write_in_place(&l.cues, ONE_CUE);
    pump_for(&mut app, LONGER_THAN_THE_OLD_CLOCKS);
    assert_eq!(texts_under::<CueList>(&mut app)[0], "alarm", "the cue list changed with no word from the index: a clock is reading the file");
    announce(&l.root, &l.cues);
    let shown = pump_until(&mut app, SHOWN_WITHIN, |app| texts_under::<CueList>(app) == ["drum"]);
    assert!(shown.is_some(), "the announced sound file was not shown within {SHOWN_WITHIN:?}: {:?}", texts_under::<CueList>(&mut app));
    assert_eq!(world_lists::reads_on_this_thread(), read_to_fill + 1, "one changed file is one read");
}

/// **A file that is not there is not judged again by a change to any other file**, in a project with no forest, no water plan and no sound
/// file: the lists' files are not in the listing, and "not there" stands until the listing's `.gitignore` moves. When it does move, the
/// verdict is reached again, and a forest folder the rules now name is said as not followed instead of `no trees planted`.
#[test]
fn a_file_that_is_not_there_is_not_judged_again_until_the_rules_move() {
    let root = fixtures::Fixture::new(&format!("w501c_lists_bare_{}", std::process::id())).descriptor("stool", "alpha").place("stool", (0.0, 0.0)).build("m");
    let unrelated = root.join("notes/unrelated.yaml");
    std::fs::create_dir_all(root.join("notes")).unwrap_or_else(|e| panic!("{e}"));
    write_in_place(&unrelated, "note: first\n");
    let mut app = stepped(&root);
    open_workspace(&mut app, "workspace.world", Workspace::World);
    sweep(&app, SWEEP_NEVER);
    assert_eq!(texts_under::<ForestList>(&mut app), [world_lists::NO_TREES]);
    assert_eq!(texts_under::<WaterList>(&mut app), [world_lists::NO_WATER]);
    let (reads, judged) = (world_lists::reads_on_this_thread(), world_lists::verdict_lookups_on_this_thread());

    let before = published(&app);
    write_in_place(&unrelated, "note: second, and longer\n");
    announce(&root, &unrelated);
    assert!(pump_until(&mut app, SHOWN_WITHIN, |app| published(app) > before).is_some(), "the index did not publish the unrelated change, so a count of nothing would prove nothing");
    pump_for(&mut app, Duration::from_millis(300));
    assert_eq!(world_lists::reads_on_this_thread(), reads, "a change to an unrelated file made a list read a file");
    assert_eq!(world_lists::verdict_lookups_on_this_thread(), judged, "a change to an unrelated file made a list judge a file that is not there again (it reads .gitignore)");

    // The rules move: the verdict is reached again, and now says the forest is not followed.
    let ignore = root.join(".gitignore");
    write_in_place(&ignore, "forest/\n");
    announce(&root, &ignore);
    let said = pump_until(&mut app, SHOWN_WITHIN, |app| texts_under::<ForestList>(app).iter().any(|row| row.contains("not followed")));
    assert!(said.is_some(), "a .gitignore that names the forest folder did not change what the FOREST list says: {:?}", texts_under::<ForestList>(&mut app));
    assert!(world_lists::verdict_lookups_on_this_thread() > judged, "the verdict was not reached again when the rules moved");
    assert_eq!(texts_under::<WaterList>(&mut app), [world_lists::NO_WATER], "the water list's file is not named by the rules");
}
