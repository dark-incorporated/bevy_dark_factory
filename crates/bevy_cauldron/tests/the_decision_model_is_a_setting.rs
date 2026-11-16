//! **Which decision model is asked is one setting** (DM-03, #454): `decision_model`, Local or Remote, in Settings.
//!
//! - only the person's file chooses it, and a project's file that names it is a settings problem naming that file;
//! - the Decision model row says before it is chosen that Remote sends screenshots to OpenRouter and Perplexity, says the exact
//!   refusal when Remote is chosen with no key, and refuses to be written into the project's file;
//! - the status band carries one LED with the model's name that follows the setting and the round trips that happened.

use std::path::PathBuf;
use std::time::Duration;

use bevy::prelude::*;
use bevy_cauldron::decision_model::{DecisionLed, DecisionLedChip, DecisionState};
use bevy_cauldron::live::EditorReloads;
use bevy_cauldron::setting_catalogue::{self, REMOTE_DISCLOSURE};
use bevy_cauldron::settings::{EditorSettings, SettingsLayer, UserSettingsFile};
use bevy_cauldron::settings_screen::{OpenSettings, Scope, SettingsDialog};
use bevy_gooey::testing::{click, find_by_label};
use bevy_gooey::{Led, LedState, LedWord, Role, Tooltip};
use bevy_grimoire_core::live::Outcome;
use bevy_oracle::{DecisionModel, Endpoints, REMOTE_WITHOUT_KEY};

use crate::fixtures;

struct Rig {
    app: App,
    user: PathBuf,
    project: PathBuf,
}

fn rig(tag: &str, user_settings: Option<&str>, project_settings: Option<&str>, key: Option<&str>) -> Rig {
    let tag = format!("{tag}_{}", std::process::id());
    let root = fixtures::Fixture::new(&tag).descriptor("site/wall", "alpha").build("m");
    let project = root.join("assets/grimoire/editor.yaml");
    if let Some(text) = project_settings {
        std::fs::write(&project, text).unwrap_or_else(|e| panic!("{e}"));
    }
    let user = root.join(format!("user_config_{tag}/settings.yaml"));
    std::fs::create_dir_all(user.parent().unwrap_or(&root)).unwrap_or_else(|e| panic!("{e}"));
    if let Some(text) = user_settings {
        std::fs::write(&user, text).unwrap_or_else(|e| panic!("{e}"));
    }
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(UserSettingsFile(user.clone()));
    app.insert_resource(DecisionState { endpoints: Endpoints { key: key.map(str::to_owned), ..Default::default() }, ..Default::default() });
    updates(&mut app, 6);
    Rig { app, user, project }
}

fn updates(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

fn until(app: &mut App, what: &str, done: impl Fn(&mut App) -> bool) {
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(8) {
        updates(app, 2);
        if done(app) {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("never saw: {what}");
}

fn layer(text: &str) -> SettingsLayer {
    SettingsLayer::read("test", text).unwrap_or_else(|r| panic!("{}", r.reason))
}

/// The LED's state, its name, and what its hover says.
fn led(app: &mut App) -> (LedState, String, String) {
    let world = app.world_mut();
    let (entity, state) = world.query_filtered::<(Entity, &Led), With<DecisionLed>>().single(world).map(|(e, l)| (e, l.0)).expect("the status band carries the decision model's LED");
    let mut name = String::new();
    let kids: Vec<Entity> = world.get::<Children>(entity).map(|c| c.iter().collect()).unwrap_or_default();
    for kid in kids {
        if world.get::<LedWord>(kid).is_none()
            && let Some(text) = world.get::<Text>(kid)
        {
            name = text.0.clone();
        }
    }
    let tip = world.query_filtered::<&Tooltip, With<DecisionLedChip>>().single(world).map(|t| t.text.clone()).expect("the chip carries the hover");
    (state, name, tip)
}

fn problems(app: &App, file_part: &str) -> Vec<String> {
    app.world()
        .resource::<EditorReloads>()
        .kinds()
        .iter()
        .filter(|k| k.outcome == Outcome::Refused && k.file.contains(file_part))
        .map(|k| format!("{} {}", k.file, k.reason))
        .collect()
}

#[test]
fn only_the_persons_file_chooses_the_model() {
    let remote = layer("{version: 1, decision_model: remote}");
    let local = layer("{version: 1, decision_model: local}");
    assert_eq!(EditorSettings::default().decision_model, DecisionModel::Local);
    assert_eq!(EditorSettings::resolve(Some(&remote), None).decision_model, DecisionModel::Remote);
    assert_eq!(EditorSettings::resolve(Some(&remote), Some(&local)).decision_model, DecisionModel::Remote, "a project's Local does not undo the person's Remote");
    assert_eq!(EditorSettings::resolve(None, Some(&remote)).decision_model, DecisionModel::Local, "a project's Remote chooses nothing");
    assert_eq!(EditorSettings::resolve(Some(&local), Some(&remote)).decision_model, DecisionModel::Local);
}

#[test]
fn the_row_is_listed_with_the_remote_disclosure_before_it_is_chosen() {
    let setting = setting_catalogue::find("decision_model").expect("the screen lists the decision model");
    assert_eq!(setting.label, "Decision model");
    assert!(setting.description.contains(REMOTE_DISCLOSURE), "{}", setting.description);
    assert!(setting.description.contains("OPENROUTER_API_KEY"));
    assert_eq!(REMOTE_DISCLOSURE, "Remote sends screenshots of this project to OpenRouter and Perplexity.");
}

#[test]
fn a_project_file_that_sets_remote_is_a_settings_problem_naming_it_and_the_choice_stays_local() {
    let mut r = rig("dm_project", None, Some("{version: 1, decision_model: remote}"), Some("sk-or-test"));
    until(&mut r.app, "the project's file to be refused", |app| !problems(app, "editor.yaml").is_empty());
    let said = problems(&r.app, "editor.yaml").join("\n");
    assert!(said.contains("decision_model"), "the verdict names the field: {said}");
    assert_eq!(r.app.world().resource::<EditorSettings>().decision_model, DecisionModel::Local, "the choice stays the person's");
    assert!(r.project.exists());
}

#[test]
fn the_led_follows_the_setting_and_the_round_trips() {
    let mut r = rig("dm_led", Some("{version: 1, decision_model: remote}"), None, None);
    // Remote with no key is failed at once, with the sentence a decision is refused with.
    until(&mut r.app, "the LED to follow the user's Remote", |app| led(app).1 == "pplx-decider-v1.3:free");
    let (state, _, tip) = led(&mut r.app);
    assert_eq!(state, LedState::Failed);
    assert!(tip.contains(REMOTE_WITHOUT_KEY) && tip.contains("OPENROUTER_API_KEY"), "{tip}");

    // Back to Local: the light is off until Local is asked, and the name is Local's.
    std::fs::write(&r.user, "{version: 1, decision_model: local}").unwrap_or_else(|e| panic!("{e}"));
    until(&mut r.app, "the LED to follow Local", |app| led(app).1 == "d3-flash");
    assert_eq!(led(&mut r.app).0, LedState::Off);

    let link = r.app.world().resource::<DecisionState>().link.clone();
    link.begin();
    until(&mut r.app, "busy", |app| led(app).0 == LedState::Busy);
    link.answered(DecisionModel::Local, Some("d3-flash".to_owned()));
    until(&mut r.app, "live", |app| led(app).0 == LedState::Live);
    link.begin();
    link.failed(DecisionModel::Local, "decision endpoint http://127.0.0.1:8093/v1/systemone did not answer: refused".to_owned());
    until(&mut r.app, "failed", |app| led(app).0 == LedState::Failed);
    assert!(led(&mut r.app).2.contains("did not answer"), "the reason is in the tooltip");
    let _ = (&r.project,);
}

fn open_on_the_row(app: &mut App) {
    app.world_mut().write_message(OpenSettings);
    updates(app, 4);
    let search = find_by_label(app, "Search settings").expect("the screen has a search field");
    click(app, search);
    let row = app
        .world_mut()
        .query::<(Entity, &bevy_gooey::TextInputValue)>()
        .iter(app.world())
        .filter(|(_, v)| v.0.is_empty())
        .map(|(e, _)| e)
        .next()
        .expect("the search row holds no text");
    app.world_mut().commands().trigger(bevy::ui_widgets::ValueChange { source: row, value: "decision model".to_owned(), is_final: false });
    updates(app, 4);
}

fn all_text(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    world.query::<&Text>().iter(world).map(|t| t.0.clone()).collect()
}

fn option(app: &mut App, words: &str) -> Entity {
    let row = find_by_label(app, "Decision model").unwrap_or_else(|e| panic!("the row is listed: {e}"));
    let world = app.world_mut();
    let mut stack = vec![row];
    while let Some(at) = stack.pop() {
        if world.get::<Role>(at) == Some(&Role::ChoiceOption) {
            let said = world.get::<Children>(at).and_then(|k| k.iter().find_map(|kid| world.get::<Text>(kid).map(|t| t.0.clone()))).or_else(|| world.get::<Text>(at).map(|t| t.0.clone()));
            if said.as_deref() == Some(words) {
                return at;
            }
        }
        if let Some(kids) = world.get::<Children>(at) {
            stack.extend(kids.iter());
        }
    }
    panic!("the row offers no `{words}`");
}

#[test]
fn the_row_says_the_refusal_when_remote_is_chosen_with_no_key_and_choosing_writes_the_users_file() {
    let mut r = rig("dm_row", None, None, None);
    open_on_the_row(&mut r.app);
    assert!(all_text(&mut r.app).iter().any(|t| t.contains(REMOTE_DISCLOSURE)), "the disclosure is read before Remote is chosen");
    assert!(!all_text(&mut r.app).iter().any(|t| t == REMOTE_WITHOUT_KEY), "nothing is wrong while Local is chosen");

    let remote = option(&mut r.app, "Remote");
    click(&mut r.app, remote);
    until(&mut r.app, "the user's file to hold Remote", |_| std::fs::read_to_string(&r.user).is_ok_and(|t| t.contains("remote")));
    assert_eq!(layer(&std::fs::read_to_string(&r.user).unwrap()).decision_model, Some(DecisionModel::Remote));
    until(&mut r.app, "the row to say the refusal", |app| all_text(app).iter().any(|t| t == REMOTE_WITHOUT_KEY));
}

#[test]
fn a_write_into_the_projects_file_is_refused_and_says_why() {
    let mut r = rig("dm_scope", None, None, Some("sk-or-test"));
    open_on_the_row(&mut r.app);
    {
        let world = r.app.world_mut();
        let mut dialogs = world.query::<&mut SettingsDialog>();
        dialogs.single_mut(world).expect("the dialog is open").scope = Scope::Project;
    }
    updates(&mut r.app, 6);
    let remote = option(&mut r.app, "Remote");
    click(&mut r.app, remote);
    updates(&mut r.app, 6);
    let project_file = std::fs::read_to_string(&r.project).unwrap_or_default();
    assert!(!project_file.contains("decision_model"), "nothing was written to the project's file: {project_file}");
    assert!(all_text(&mut r.app).iter().any(|t| t.contains("your own choice")), "the screen says why");
}
