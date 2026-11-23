//! **Every code-colour preset holds gooey's own rules, on the editor's skin, at every theme** (#525,
//! `feedback-1795094391027`; gooey's #116).
//!
//! gooey derives a `Role::CodeSyntax` colour by searching the lightness until it holds the contrast bar on the
//! skin's own ground — 4.5:1, or 7:1 where the skin's text already clears it (the high contrast theme). A theme
//! chooses hue, chroma and where the search starts; it cannot choose whether the bar is met. `GooeySkin::problems`
//! is where that is checked, and it reports a theme that names a role it may not colour, a hue that is not a
//! number, a chroma past sRGB, or a shift outside its range as `SkinProblem::BadSize { field: "syntax_theme" }`
//! — and every derived colour that misses the bar as `LowContrast`.
//!
//! # What these assert
//!
//! The editor's **compiled** skin for each of its three themes (`style::compiled_for`), with each of the five
//! presets laid on, is a skin `problems()` answers empty for. That is the editor's own skin and its own grounds,
//! not gooey's presets: a parchment ground and a high-contrast one fail the same search in different places, and a
//! preset that cleared the bar on gooey's `dark()` alone would be no evidence for either.
//!
//! **The positive control** is that the bar is real: a skin whose `text` and `surface` are the same colour cannot
//! give any role a colour at the bar, so `problems()` reports it. Without that, "every preset is clean" would also
//! be true of a `problems()` that reports nothing.

use bevy::prelude::*;
use bevy_dark_factory::settings::SyntaxTheme;
use bevy_dark_factory::style;
use bevy_gooey::{GooeySkin, SkinProblem, SyntaxRole};

/// The editor's three themes, in the order a person meets them.
const THEMES: [bevy_dark_factory::settings::Theme; 3] = [
    bevy_dark_factory::settings::Theme::ToolDark,
    bevy_dark_factory::settings::Theme::Parchment,
    bevy_dark_factory::settings::Theme::HighContrast,
];

/// The editor's compiled skin for `theme`, with `preset` laid on — the shape `style::apply_style` writes.
fn skin(theme: bevy_dark_factory::settings::Theme, preset: SyntaxTheme) -> GooeySkin {
    let mut skin = style::compiled_for(theme).skin;
    skin.syntax_theme = preset.theme();
    skin
}

/// **Every preset, on every theme, is a skin gooey reports nothing wrong with.** This is the acceptance the issue
/// names ("the same contrast bar applies"), measured on the editor's own skins rather than gooey's.
#[test]
fn every_preset_is_clean_on_every_theme() {
    for theme in THEMES {
        for preset in SyntaxTheme::ALL {
            let skin = skin(theme, preset);
            let problems = skin.problems();
            let code: Vec<String> = problems.iter().filter(|p| mentions_code(p)).map(|p| format!("{p:?}")).collect();
            assert!(problems.is_empty(), "{theme:?} with {preset:?}: {problems:?}");
            assert!(code.is_empty(), "{theme:?} with {preset:?} names the code colours: {code:?}");
        }
    }
}

/// Whether a problem is about a code colour — a `LowContrast` whose pair is a syntax one, or the theme's own
/// `BadSize`.
fn mentions_code(problem: &SkinProblem) -> bool {
    match problem {
        SkinProblem::LowContrast { pair, .. } => pair.contains("syntax"),
        SkinProblem::BadSize { field } => *field == "syntax_theme",
        _ => false,
    }
}

/// **The bar is real**: a skin whose ground and text are one colour cannot give a syntax role a colour that clears
/// it, so `problems()` reports the code colours. This is the positive control for the test above.
#[test]
fn a_skin_with_no_contrast_is_reported_on_its_code_colours() {
    let mut skin = style::compiled_for(bevy_dark_factory::settings::Theme::ToolDark).skin;
    // The ground and the text are each other, so every derived colour is on the bar's floor by construction.
    skin.text = skin.solid_surface();
    let problems = skin.problems();
    assert!(problems.iter().any(mentions_code), "a skin with text on its own ground reports its code colours: {problems:?}");
}

/// **`Skin` is exactly the colours the editor drew before the setting**: laying the preset on changes nothing,
/// and the derived colours are the ones a skin with no theme gives. A person who never opens the row sees no
/// pixel move.
#[test]
fn the_skin_preset_is_the_unthemed_colours() {
    for theme in THEMES {
        let plain = style::compiled_for(theme).skin;
        let chosen = skin(theme, SyntaxTheme::Skin);
        for role in SyntaxRole::ALL {
            assert_eq!(plain.syntax(role), chosen.syntax(role), "{theme:?}, {role:?}: the shipped preset moved a colour");
        }
    }
}

/// **Two presets give two colours for one role**, so a preset is not an alias of another: measured on the editor's
/// skin, per role, so a table where two names meant one theme would fail here.
#[test]
fn warm_and_cool_differ_for_the_roles_a_theme_colours() {
    let warm = skin(bevy_dark_factory::settings::Theme::ToolDark, SyntaxTheme::Warm);
    let cool = skin(bevy_dark_factory::settings::Theme::ToolDark, SyntaxTheme::Cool);
    let differing = SyntaxRole::ALL.into_iter().filter(|role| warm.syntax(*role) != cool.syntax(*role)).count();
    assert!(differing >= 6, "`warm` and `cool` tell {differing} roles apart; the hues they state must reach the colours");
}
