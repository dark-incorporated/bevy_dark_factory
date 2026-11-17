//! **The agent tab names a model on omp's launch line, chosen once in Settings** (#477, `feedback-1794783326602`).
//!
//! The owner, looking at the terminal pane: *"This need to open as omp session, that uses default cloud agent, like sonnet 5.5
//! from anthropic"*. Today the line is `omp -e <dir> "<self test prompt>"` with no model, so which model answers is whatever that
//! machine's `omp` config picks — different on every machine. The setting `terminal_agent_model` is a free-text model id, read by
//! the one `plan()` that builds the line, so the fresh and the resume forms cannot disagree, and an empty setting omits `--model`
//! rather than passing an empty value.
//!
//! The model the tab was started with is also on the self-test card, so a person can tell which one is answering.

use std::path::Path;

use bevy_cauldron::settings::{EditorSettings, SettingsLayer, TerminalAgent};
use bevy_cauldron::terminal_agent::{self, AgentSetup};

use crate::fixtures;

/// The model this test sets, spelled as an exact `provider/model` omp accepts.
const MODEL: &str = "anthropic/claude-sonnet-5-5";

/// A setup in a fresh temporary folder, with a file standing in for the server binary, named for this test and this process.
fn setup(tag: &str) -> AgentSetup {
    let root = std::env::temp_dir().join(format!("agent_model_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap_or_else(|e| panic!("{}: {e}", root.display()));
    let server = root.join("bevy_cauldron_mcp");
    std::fs::write(&server, "").unwrap_or_else(|e| panic!("{}: {e}", server.display()));
    AgentSetup { port: Some(15799), server, files: root.join("agents") }
}

/// **The launch line carries `--model <id>`, in the fresh form and in the resume form**, and one builder makes both, so the two
/// cannot disagree.
#[test]
fn the_model_the_settings_name_is_on_the_launch_line_in_both_the_fresh_and_the_resume_form() {
    let setup = setup("both");
    let session = "01a1bb82-fdc4-7343-83c6-b6dce13deffa";
    let fresh = terminal_agent::plan_with_model(TerminalAgent::OhMyPi, None, Some(&setup), Some(Path::new("/tmp/p")), None, Some(MODEL), 0.0);
    let resumed = terminal_agent::plan_with_model(TerminalAgent::OhMyPi, None, Some(&setup), Some(Path::new("/tmp/p")), Some(session), Some(MODEL), 0.0);
    let omp = setup.files.join("omp");
    assert_eq!(
        String::from_utf8_lossy(&fresh.line),
        format!("omp -e '{}' --model '{MODEL}' '{}'\r", omp.display(), terminal_agent::SELF_TEST_PROMPT),
        "the fresh launch names the model"
    );
    assert_eq!(
        String::from_utf8_lossy(&resumed.line),
        format!("omp -e '{}' --model '{MODEL}' --resume '{session}'\r", omp.display()),
        "the resume launch names the same model, from the same builder"
    );
}

/// **An empty setting omits `--model` entirely**, rather than passing an empty value omp would refuse.
#[test]
fn an_empty_model_setting_omits_the_flag_rather_than_passing_an_empty_value() {
    let setup = setup("empty");
    for model in [None, Some(""), Some("   ")] {
        let launch = terminal_agent::plan_with_model(TerminalAgent::OhMyPi, None, Some(&setup), None, None, model, 0.0);
        let line = String::from_utf8_lossy(&launch.line).into_owned();
        assert!(!line.contains("--model"), "{model:?} omits the flag: {line:?}");
    }
}

/// **The model a tab was started with is on its card**, so a person can tell which one is answering. A tab with no model says
/// nothing extra rather than leaving a blank.
#[test]
fn the_card_names_the_model_the_tab_was_started_with() {
    let setup = setup("card");
    let launch = terminal_agent::plan_with_model(TerminalAgent::OhMyPi, None, Some(&setup), None, None, Some(MODEL), 0.0);
    assert_eq!(launch.post.heading(), format!("Agent self test · omp · {MODEL}"), "the card's heading names the model the tab runs");
    assert_eq!(launch.post.model.as_deref(), Some(MODEL), "and the record carries it, which is what the heading is built from");
    let quiet = terminal_agent::plan_with_model(TerminalAgent::OhMyPi, None, Some(&setup), None, None, None, 0.0);
    assert_eq!(quiet.post.heading(), "Agent self test · omp", "a tab with no model says exactly what it said before");
    assert_eq!(quiet.post.model, None);
    // A custom harness keeps its own flags, so the settings' model never rides on its card.
    let custom = terminal_agent::plan_with_model(TerminalAgent::Custom, Some("echo hi"), Some(&setup), None, None, Some(MODEL), 0.0);
    assert_eq!(custom.post.model, None, "a custom command's model is its own");
    assert_eq!(String::from_utf8_lossy(&custom.line), "echo hi\r");
}

/// **A model id that is not a plain word is handed to the shell as one word**: it comes from a settings file, so a value holding
/// `;` or a quote must not become a second command. `shell_quote` is the whole defence, and this asserts the launch line carries
/// the escaped form and nothing wider.
#[test]
fn a_model_id_with_a_shell_in_it_is_quoted_as_one_word_and_never_read_as_a_command() {
    let setup = setup("quoted");
    let hostile = "x'; rm -rf ~; echo '";
    let launch = terminal_agent::plan_with_model(TerminalAgent::OhMyPi, None, Some(&setup), None, None, Some(hostile), 0.0);
    let line = String::from_utf8_lossy(&launch.line).into_owned();
    assert_eq!(line.matches("--model ").count(), 1, "the model is one argument: {line}");
    assert!(
        line.contains(&format!("--model {}", terminal_agent::shell_quote(hostile))),
        "the model is handed over as the shell-quoted word `{}`: {line}",
        terminal_agent::shell_quote(hostile)
    );
    // Every `'` the value holds is closed, escaped and reopened, which is what makes the shell read it as data.
    assert_eq!(hostile.matches('\'').count(), 2, "the value holds two quotes");
    assert_eq!(line.matches("'\\''").count(), 2, "each of them is escaped: {line}");
}

/// **The settings key is a free-text model id, beside `terminal_agent`**, read by the launch from the resolved settings: a file that
/// sets it puts the model on the line, and a layer that leaves it out keeps the one below.
#[test]
fn the_model_setting_is_read_beside_the_agent_and_keeps_the_layer_below_when_a_layer_leaves_it_out() {
    let text = format!("{{version: 1, terminal_agent: OhMyPi, terminal_agent_model: \"{MODEL}\"}}");
    let file = "settings.yaml";
    let layer = SettingsLayer::read(file, &text).unwrap_or_else(|r| panic!("{r:?}"));
    assert_eq!(layer.terminal_agent_model.as_deref(), Some(MODEL), "the key reads as the model it names");
    let resolved = EditorSettings::resolve(Some(&layer), None);
    assert_eq!(resolved.terminal_agent_model.as_deref(), Some(MODEL), "the resolved settings carry it");
    // A project file that leaves it out keeps the person's.
    let project = SettingsLayer::read("editor.yaml", "{version: 1}").unwrap_or_else(|r| panic!("{r:?}"));
    let both = EditorSettings::resolve(Some(&layer), Some(&project));
    assert_eq!(both.terminal_agent_model.as_deref(), Some(MODEL), "a layer that says nothing keeps the layer below");
    // The built-in default is no model, which is what the editor does today.
    assert_eq!(EditorSettings::default().terminal_agent_model, None, "no model is named until a person names one");
}

/// **The editor hands the model to the terminal with the rest of its agent settings**, so the button and the resume path start the
/// same model.
#[test]
fn the_terminal_is_given_the_model_the_settings_name() {
    let root = fixtures::Fixture::new(&format!("agent_model_terminal_{}", std::process::id())).descriptor("floor", "alpha").build("m");
    let file = root.join("assets/grimoire/editor.yaml");
    let settings = format!("{{version: 1, terminal_agent: OhMyPi, terminal_agent_model: \"{MODEL}\"}}");
    std::fs::write(&file, &settings).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..10 {
        app.update();
    }
    let terminal = app.world().resource::<bevy_cauldron::terminal::Terminal>();
    assert_eq!(terminal.agent_model(), Some(MODEL), "the terminal holds the model the project's file names");
    assert_eq!(terminal.agent(), TerminalAgent::OhMyPi);
}

/// **A model omp refuses reads back as a failed check naming the id and what omp said** (#477's third design note), never a silent
/// fallback to omp's default. omp prints this to stderr and exits 1; the terminal holds the words, and the card says them.
///
/// Measured against the real binary on 2026-11-17 (`omp --model no-such-model-xyz -p hi`): `Model "no-such-model-xyz" not found`,
/// then `Run \`omp models find <pattern>\``. The rows below are those bytes.
#[test]
fn a_model_omp_refused_is_a_failed_connected_row_naming_the_id_and_what_omp_said() {
    let setup = setup("refused");
    let mut launch = terminal_agent::plan_with_model(TerminalAgent::OhMyPi, None, Some(&setup), None, None, Some(MODEL), 0.0);
    let token = launch.post.token.clone();
    launch.post.started();
    let mut terminal = bevy_cauldron::terminal::Terminal::default();
    terminal.spawn_agent(80, 24, Path::new("/tmp"), launch);
    // The shell echoes the line, then omp refuses it: the two rows omp writes.
    let said = format!("Model \"{MODEL}\" not found\n\nRun `omp models find <pattern>` to search, or `omp models` to list all.\n");
    terminal.tab_mut().expect("the agent's tab").screen_mut().feed(said.as_bytes());
    assert!(terminal_agent::read_a_refused_model(&mut terminal), "the refusal is read from the tab's own screen");
    let post = terminal.agent_with_token_mut(&token).expect("the agent's record");
    match &post.checks[terminal_agent::CONNECTED_CHECK] {
        terminal_agent::CheckStatus::Failed(why) => {
            assert!(why.contains(MODEL), "the row names the model omp refused: {why}");
            assert!(why.contains("not found"), "and quotes what omp said: {why}");
            assert!(why.contains("terminal_agent_model"), "and names the setting that fixes it: {why}");
        }
        other => panic!("a refused model fails the connection: {other:?}"),
    }
    assert!(matches!(post.checks[terminal_agent::SELF_TEST_CHECK], terminal_agent::CheckStatus::Skipped(_)), "nothing self-tests");
}

/// **A screen with no refusal in it changes nothing**, so a shell that printed anything at all is not read as a refusal.
#[test]
fn a_screen_without_a_refusal_leaves_the_row_as_it_was() {
    let setup = setup("not_refused");
    let mut launch = terminal_agent::plan_with_model(TerminalAgent::OhMyPi, None, Some(&setup), None, None, Some(MODEL), 0.0);
    let token = launch.post.token.clone();
    launch.post.started();
    let mut terminal = bevy_cauldron::terminal::Terminal::default();
    terminal.spawn_agent(80, 24, Path::new("/tmp"), launch);
    terminal.tab_mut().expect("the agent's tab").screen_mut().feed(b"omp-coding-agent 1.0.0\r\nWhat would you like to do?\r\n");
    assert!(!terminal_agent::read_a_refused_model(&mut terminal), "nothing was refused");
    let post = terminal.agent_with_token_mut(&token).expect("the agent's record");
    assert!(matches!(post.checks[terminal_agent::CONNECTED_CHECK], terminal_agent::CheckStatus::Pending));
}
