//! **No name in the PLACE list breaks inside a word, at the window where it did** (#693).
//!
//! # What the review saw
//!
//! At 1100×700 the palette drew `Fridg` on one line and `e` on the next, and cut `Water dispenser` after
//! `dispe`. The row's name column was narrower than a six-letter word and the text was laid out with
//! `LineBreak::WordOrCharacter`, which the crate uses on purpose wherever a long single word would otherwise
//! draw past its panel (#535, #481). A piece's name in a list of things a person recognises is the other case:
//! it is read whole or not at all.
//!
//! # The owner's call
//!
//! **The count gives way, and a word that does not fit is cut with an ellipsis.** A piece's cost is recoverable from its
//! tooltip; its name is not. So the name takes the width the row leaves and wraps only between words
//! (`LineBreak::WordBoundary`), and a name of many words still wraps in its own column (#486). A *single* word wider than
//! the column, which `bevy_text` 0.20 would break where the line runs out, is cut by the editor to fit and ends in `…`
//! (`editor::fit_the_place_names`, reading the layout). A `NoWrap` name is laid out one line wide whatever the column,
//! which the panel census measures as text hidden past the panel's content box (#494).
//!
//! # What this measures
//!
//! Every drawn name's lines, from `bevy_text`'s own `PositionedGlyph::line_index` — the layout's answer, not the
//! spawn's intention. A name is read whole if it draws on one line; if it must take two, each line ends where a
//! word does, and only the last may be cut with an ellipsis.

use bevy::prelude::*;
use bevy::text::TextLayoutInfo;
use bevy::ui::ComputedNode;
use bevy::window::WindowResolution;
use bevy_dark_factory::editor::{PaletteCost, PaletteName};
use bevy_dark_factory::harness;
use bevy_dark_factory::tiles::Mode;

use crate::fixtures;

/// The window the review captured: 1100 × 700.
const WINDOW: (u32, u32) = (1100, 700);
/// The interface scale the editor ships at, so the row is as narrow as it is for a person.
const UI_SCALE: f32 = 1.2;
/// Frames a layout needs to settle after the surface is resized: the docks glide into place.
const SETTLE_FRAMES: usize = 120;

/// **Library pieces named the way the review read them** — `Fridge` is the six-letter word that broke, and
/// `Water dispenser` is the two-word name whose second word was cut mid-word.
const PIECES: [(&str, &str); 2] = [("fridge", "Fridge"), ("water_dispenser", "Water dispenser")];

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(&format!("place_break_{name}_{}", std::process::id()))
        .descriptor(PIECES[0].0, "alpha")
        .descriptor(PIECES[1].0, "alpha")
        .build("m");
    let mut app = harness::build_headless_at(&root, "m", None, Mode::Map).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(bevy::ui::UiScale(UI_SCALE));
    // **The window is spawned at the size captured, not at `Window::default`** — `surface::fit_surface_to_window`
    // sizes the surface from the primary window every frame, so a default 1280 x 720 window overrides a later
    // `resize_surface` and the row measured is not the narrow one a person saw.
    app.world_mut().spawn((
        Window { resolution: WindowResolution::new(WINDOW.0, WINDOW.1), ..default() },
        bevy::window::PrimaryWindow,
    ));
    harness::resize_surface(&mut app, WINDOW.0, WINDOW.1).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
    app
}

/// **A drawn name: its words, its box, the lines `bevy_text` laid it into, and how far its layout runs past its box.**
struct Drawn {
    words: String,
    box_width: f32,
    lines: Vec<String>,
    /// Logical px the inked words run past their box: what the clip would cut through a letter.
    past_the_box: f32,
}

/// **Every palette name the list drew**, in the order the list holds them.
///
/// The lines come from `PositionedGlyph::line_index` — the layout's own grouping — rather than from splitting
/// on `\n`, because nothing in the text inserts one: a `NoWrap` box that is too small, and a `Word` box that
/// must break, produce the same `Text`. Only the layout knows which it did.
fn drawn_names(app: &mut App) -> Vec<Drawn> {
    let world = app.world_mut();
    let mut query = world.query_filtered::<(Entity, &Text, &ComputedNode), With<PaletteName>>();
    let names: Vec<(Entity, String, f32, f32)> = query
        .iter(world)
        .map(|(entity, text, node)| (entity, text.0.clone(), node.size().x * node.inverse_scale_factor(), node.inverse_scale_factor()))
        .collect();
    let mut out = Vec::new();
    for (entity, words, box_width, to_logical) in names {
        let Some(info) = world.get::<TextLayoutInfo>(entity) else {
            // Not laid out (a name off screen, or the first frame): its break cannot be measured, and saying
            // so is better than reading zero lines as "whole".
            continue;
        };
        let lines = lines_of(info, &words);
        // **Ink, not the layout's box**: a line's trailing space is laid out past the box on a name that fits.
        let ink_right = words
            .chars()
            .zip(&info.glyphs)
            .filter(|(ch, _)| !ch.is_whitespace())
            .map(|(_, g)| g.position.x + g.atlas_info.rect.width() / 2.0)
            .fold(0.0f32, f32::max);
        let past_the_box = ink_right * to_logical - box_width;
        out.push(Drawn { words, box_width, lines, past_the_box });
    }
    out.sort_by(|a, b| a.words.cmp(&b.words));
    out
}

/// **The words on each drawn line**, read off the glyphs' own line indices.
///
/// A glyph does not carry its character — `FontAtlas` holds the raster, not the text — but `bevy_text` 0.20 lays out
/// **one glyph per character, a space included** (measured: `Water dispenser` is fifteen glyphs, the space the sixth, on
/// the first line). So the `n`th character stands on the `n`th glyph's line. A layout whose glyph count is not the
/// text's is not one this can read, and saying so beats guessing which character a glyph is.
fn lines_of(info: &TextLayoutInfo, words: &str) -> Vec<String> {
    if info.glyphs.is_empty() {
        return Vec::new();
    }
    let chars: Vec<char> = words.chars().collect();
    assert_eq!(
        info.glyphs.len(),
        chars.len(),
        "`{words}` laid out {} glyphs for {} characters, so its lines cannot be read character by character",
        info.glyphs.len(),
        chars.len()
    );
    let count = info.glyphs.iter().map(|g| g.line_index).max().unwrap_or(0) as usize + 1;
    let mut lines = vec![String::new(); count];
    for (ch, glyph) in chars.into_iter().zip(&info.glyphs) {
        lines[glyph.line_index as usize].push(ch);
    }
    lines.into_iter().map(|line| line.trim().to_owned()).filter(|line| !line.is_empty()).collect()
}

/// **No drawn name breaks inside a word.**
#[test]
fn no_place_rows_name_breaks_inside_a_word() {
    let mut app = editor("nobreak");
    let drawn = drawn_names(&mut app);
    assert!(!drawn.is_empty(), "the PLACE list drew no names at {}x{}", WINDOW.0, WINDOW.1);

    let mut broken = Vec::new();
    for name in &drawn {
        for (index, line) in name.lines.iter().enumerate() {
            let last = index + 1 == name.lines.len();
            // **A cut is the rule, and only at the end.**
            if line.ends_with('\u{2026}') || line.ends_with("...") {
                if !last {
                    broken.push(format!("`{}` is cut before its last line: {:?}", name.words, name.lines));
                }
                continue;
            }
            if last {
                continue;
            }
            // **A line that ends mid-word**, which is the break this test forbids: what follows it in the name
            // is not a space, so the layout cut a word in two.
            let rest = name.words.trim_start_matches(line.as_str());
            if rest.chars().next().is_some_and(|c| !c.is_whitespace()) {
                broken.push(format!(
                    "`{}` breaks inside a word: line {index} is `{line}` and the name continues `{rest}`",
                    name.words
                ));
            }
        }
    }
    // **A word that does not fit is cut with `…`, not by the clip** (#693): a whole word laid out past the box is the
    // `Water dispe` the review saw, broken by the panel's edge instead of by the layout.
    for name in &drawn {
        if name.past_the_box > 0.5 {
            broken.push(format!(
                "`{}` is laid out {:.1} px past its {:.1} px box, so the clip cuts a word mid-letter",
                name.words, name.past_the_box, name.box_width
            ));
        }
    }
    assert!(
        broken.is_empty(),
        "**a piece's name in the PLACE list broke inside a word at {}x{}** (#693). The name column takes the \
         row's width and breaks only between words; what does not fit ends in an ellipsis. Measured:\n{}\n\n{}",
        WINDOW.0,
        WINDOW.1,
        drawn
            .iter()
            .map(|d| format!("  `{}` in {:.1} px: {:?}", d.words, d.box_width, d.lines))
            .collect::<Vec<_>>()
            .join("\n"),
        broken.join("\n")
    );
}

/// **`Fridge` is drawn whole**, the name the review photographed as `Fridg` / `e`.
#[test]
fn fridge_is_drawn_whole() {
    let mut app = editor("fridge");
    let drawn = drawn_names(&mut app);
    let fridge = drawn
        .iter()
        .find(|d| d.words.starts_with("Fridge"))
        .unwrap_or_else(|| {
            panic!(
                "the palette drew no `Fridge`; it drew {:?}",
                drawn.iter().map(|d| &d.words).collect::<Vec<_>>()
            )
        });
    assert!(
        fridge.lines.iter().any(|line| line == "Fridge"),
        "`Fridge` is one six-letter word and the column it stands in is what must fit it: it drew as {:?} in a \
         {:.1} px box (#693)",
        fridge.lines,
        fridge.box_width
    );
}

/// **The count is laid out on every row**: it yields width to the name, and does not vanish from the list.
#[test]
fn the_count_is_still_drawn_beside_the_name() {
    let mut app = editor("count");
    let world = app.world_mut();
    let mut query = world.query_filtered::<&ComputedNode, With<PaletteCost>>();
    let widths: Vec<f32> = query.iter(world).map(|node| node.size().x * node.inverse_scale_factor()).collect();
    assert_eq!(widths.len(), 2, "one count per palette row: {widths:?}");
    assert!(
        widths.iter().all(|w| *w > 0.0),
        "every count keeps a laid-out width: it gives way to the name rather than disappearing (#693): {widths:?}"
    );
}
