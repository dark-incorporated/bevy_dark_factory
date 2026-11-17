//! **A topic the game does not have is refused with the topics that do exist** (#444, a wish from the style study).
//!
//! In the study an agent asked `blueprint_style` for the topic `dialogue`, which the guide names as an example, and was told
//! *"the game has no topic `dialogue`; its topics are `style`."* `style` is the overall feel every request already serves, not
//! a topic to ask for, so the sentence listed nothing the agent could use. The refusal now lists the topics that can be asked
//! for (every topic file of the game except `style`), says plainly when there are none, and says what a topic is.

use std::path::PathBuf;

use blueprint_model::design;
use blueprint_model::style::{self, Request};
use blueprint_model::trust::{Decision, Scope};

fn game(tag: &str, files: &[&str]) -> design::Tree {
    let dir: PathBuf = std::env::temp_dir().join(format!("blueprint-style-topics-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("design")).expect("the design folder is made");
    for file in files {
        std::fs::write(dir.join("design").join(file), "A line.\n").expect("a page is written");
    }
    design::Tree::read(&dir)
}

fn ask(tree: &design::Tree, topic: &str) -> Result<style::Answer, String> {
    let request = Request { topic: Some(topic.to_owned()), ..Request::default() };
    style::compose(tree, &request, &mut |_: &str, _: &str| Ok(Decision::Trusted(Scope::Command)))
}

/// **A game with only its overall feel has no topic to ask for, and the refusal says so** instead of listing `style`.
#[test]
fn a_game_with_only_its_style_says_it_has_no_topics() {
    let tree = game("none", &["style.md"]);
    let refused = ask(&tree, "dialogue").expect_err("the game has no dialogue topic");
    assert!(refused.contains("no topic `dialogue`"), "{refused}");
    assert!(!refused.contains("its topics are `style`"), "`style` is not a topic to ask for, and the refusal lists it as one: {refused}");
    assert!(refused.contains("no topic files beside `style.md`"), "the refusal does not say the game has no topics: {refused}");
    assert!(refused.contains("design/<topic>.md"), "the refusal does not say what a topic is: {refused}");
}

/// **A game with topics gets exactly those back**, in the tree's order, and not `style`.
#[test]
fn a_game_with_topics_lists_them_and_not_its_style() {
    let tree = game("some", &["style.md", "dialogue.md", "ui.md"]);
    let refused = ask(&tree, "sound").expect_err("the game has no sound topic");
    assert!(refused.contains("no topic `sound`"), "{refused}");
    assert!(refused.contains("`dialogue`") && refused.contains("`ui`"), "the refusal does not list the topics that exist: {refused}");
    assert!(!refused.contains("`style`"), "`style` is listed as a topic to ask for: {refused}");
    assert!(ask(&tree, "dialogue").is_ok(), "a topic that exists is served");
}
