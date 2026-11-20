//! **A periodic save that reuses the manual save path, and says a countdown only when it is about to fire** (#508).
//!
//! A crash between two `Cmd+S` presses takes back everything a person or an agent did in between. The answer is a timer
//! that dispatches the *same* saves a key does — `map_save` for the map, [`crate::files::save_all`] for the buffers — so
//! there is one line in the log and no second, silent write path.
//!
//! # Parked when firing would be wrong
//!
//! [`tick`] does not count down while a question is up (`crate::confirm::Confirm`), while a project is not open, or while
//! the game plays. A save that landed while somebody was answering "discard?" would save the work the question exists to
//! discard; one that landed while an agent was mid-turn would put a line between two of its operations.
//!
//! # The countdown is quiet until it matters
//!
//! The band says *Auto save in Ns* only inside [`WARN_S`] of the write, because a countdown drawn all the time is a row a
//! person stops seeing — and the reason a number is there at all is to say a write is imminent.

use bevy::prelude::*;
use bevy::time::Time;

use crate::confirm::Confirm;
use crate::editor::EditorState;
use crate::files::Files;
use crate::project::{OpenMap, Project};
use crate::screen::Screen;

/// **How long the band says nothing, in seconds**, before the save: the last five seconds are the only ones with words.
pub const WARN_S: f64 = 5.0;

/// **When the countdown was last written**, so a paused countdown does not accumulate and the whole of one interval is
/// waited rather than most of it replayed after an answer.
#[derive(Resource, Default)]
pub struct AutoSave {
    /// Seconds still to run before the next save.
    pub remaining: f64,
    /// The map the interval is about, so a project change starts the countdown again rather than carrying another
    /// project's clock across.
    root: Option<std::path::PathBuf>,
}

/// **What the band says**: `""` outside the last [`WARN_S`], and `Auto save in Ns` inside it.
///
/// One function, so the system that paints the band and the test that reads it cannot disagree. `remaining` is seconds;
/// the number shown is rounded up, so the last second reads `1s` and not `0s`.
pub fn band_text(remaining: f64, armed: bool) -> String {
    if !armed || remaining > WARN_S || remaining <= 0.0 {
        return String::new();
    }
    format!("Auto save in {}s", remaining.ceil() as u64)
}

/// **Count down, and save through the manual paths when it reaches zero.**
///
/// The map's save is [`crate::editor::save_map_public`] (which dispatches `map_save`, so the log records it exactly as
/// `Cmd+S` does) and the buffers' is [`Files::save_all`] — the two calls a person's own `Cmd+S` and their leaving the
/// editor make, so a timer cannot write something a key would not.
pub fn tick(
    // **The editor's own clock** (`Time`, which is `Time<Virtual>`), not the wall clock: a countdown that ran on real seconds
    // would fire while the app is paused, and the frame's own step is what "an interval" means here.
    time: Res<Time>,
    settings: Option<Res<crate::settings::EditorSettings>>,
    project: Option<Res<Project>>,
    mut open: Option<ResMut<OpenMap>>,
    mut files: Option<ResMut<Files>>,
    confirm: Option<Res<Confirm>>,
    mut state: Option<ResMut<EditorState>>,
    mut timer: ResMut<AutoSave>,
) {
    let interval = settings.as_deref().map(|s| s.auto_save_interval_s).unwrap_or(0.0);
    // **Off is off**: no countdown, nothing saved, and the drafts (`crate::draft`) are the whole of the answer. **The clock is
    // cleared with it**, so turning the interval back on starts a whole interval rather than resuming a countdown the person
    // stopped half an hour ago.
    if interval <= 0.0 {
        timer.remaining = 0.0;
        timer.root = None;
        return;
    }
    let Some(project) = project else {
        timer.remaining = 0.0;
        return;
    };
    if timer.root.as_deref() != Some(project.root.as_path()) {
        timer.root = Some(project.root.clone());
        timer.remaining = interval;
    }
    // **Parked, not reset**: a question that is up for two seconds leaves the countdown where it was rather than starting
    // the interval over, so a save is never pushed back indefinitely by a person thinking, and never fires *during* the
    // answer either.
    if confirm.as_deref().is_some_and(Confirm::is_open) || crate::ops_play::running_pid(&project.root).is_some() {
        return;
    }
    timer.remaining -= f64::from(time.delta_secs());
    if timer.remaining > 0.0 {
        return;
    }
    timer.remaining = interval;
    // **The map's save, through the key's own call**, so it dispatches `map_save`: one line in the log, and an undo sees a
    // person's save rather than a second, silent write path.
    if let (Some(open), Some(state)) = (open.as_deref_mut(), state.as_deref_mut())
        && open.dirty()
    {
        crate::editor::save_map_public(&project, open, state);
    }
    // **The buffers**, through the one call a person's leaving makes. **A read-only buffer is skipped rather than refused**:
    // a refusal every interval would be a toast nobody can dismiss, and there is nothing to save in a command file anyway.
    if let Some(files) = files.as_deref_mut() {
        let dirty: Vec<String> = files.dirty().into_iter().filter(|rel| files.get(rel).is_some_and(|f| !f.read_only && f.problem.is_none())).map(str::to_owned).collect();
        let mut first: Option<String> = None;
        for rel in dirty {
            if let Err(why) = crate::files::save_file(files, &project.root, &rel) {
                first.get_or_insert(why);
            }
        }
        if let (Some(why), Some(state)) = (first, state.as_deref_mut()) {
            state.status.problem(format!("NOT SAVED: {why}"));
        }
    }
}

pub struct AutoSavePlugin;

impl Plugin for AutoSavePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AutoSave>().add_systems(Update, tick.run_if(in_state(Screen::Editor)));
    }
}
