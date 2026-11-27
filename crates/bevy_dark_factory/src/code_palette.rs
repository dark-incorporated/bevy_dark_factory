//! **Named code palettes: Monokai, Solarized and GitHub, as gooey syntax themes** (#610).
//!
//! The owner's call on #610 (2026-11-27): *a separate "Code colours" setting listing named syntax palettes (Skin
//! default, plus e.g. Monokai, Solarized, GitHub)*. gooey already takes the code colours apart from the rest of the
//! skin — [`GooeySkin::syntax_theme`](bevy_gooey::GooeySkin), its #116, which the Code colours row writes — so a
//! palette here is only data: the colours its authors publish, by the role the editor colours, and
//! [`CodePalette::theme`] says them in gooey's terms. No new surface was needed upstream.
//!
//! # What a palette keeps, and what the skin keeps
//!
//! gooey draws a role at the hue and chroma a theme names, and searches the lightness from where the theme starts
//! it toward the skin's text until the colour holds the contrast bar on the skin's ground. So a palette keeps its
//! **hues and chroma**, and its **lightness where its own text and ground put it**: a colour a third of the way
//! from the palette's text to its ground starts a third of the way from the skin's text to the skin's ground. The
//! skin keeps the ground, the text and the bar: Monokai on Parchment is Monokai's hues on paper, darkened until
//! they read, not Monokai's dark ground pasted into a light editor.
//!
//! A palette names the seven roles of code — a key, a string, a number, a keyword, a reference, a function and a
//! type. All three palettes' authors draw Markdown's emphasis and strong in the body text's colour, so those keep
//! the skin's own two hues and stay told apart; a comment, an operator and an error are never a theme's (gooey).

use bevy::color::Oklcha;
use bevy::prelude::Color;
use bevy_gooey::{SyntaxInk, SyntaxRole, SyntaxTheme};

/// **gooey's ceiling on an ink's chroma**: [`SyntaxInk::chroma`] takes 0 to 0.2, and a theme past it is a
/// `SkinProblem::BadSize` on `syntax_theme`. A published colour more vivid than this is drawn at this chroma.
const INK_CHROMA_MAX: f32 = 0.2;

/// One palette as its authors publish it, each colour as `0xRRGGBB`: the ground and the body text its colours
/// were chosen against, and the colour it gives each role of code.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CodePalette {
    /// The palette's background.
    pub ground: u32,
    /// The palette's body text.
    pub text: u32,
    /// The colour of each role the palette names.
    pub inks: [(SyntaxRole, u32); 7],
}

// ── Monokai (Wimer Hazenberg), as TextMate's and Sublime Text's `Monokai.tmTheme` publish it ────────────────

const MONOKAI_BACKGROUND: u32 = 0x27_28_22;
const MONOKAI_FOREGROUND: u32 = 0xF8_F8_F2;
/// `keyword`, `storage`.
const MONOKAI_PINK: u32 = 0xF9_26_72;
/// `string`.
const MONOKAI_YELLOW: u32 = 0xE6_DB_74;
/// `constant.numeric`, `constant.language`.
const MONOKAI_PURPLE: u32 = 0xAE_81_FF;
/// `entity.name.function`, `entity.other.attribute-name`.
const MONOKAI_GREEN: u32 = 0xA6_E2_2E;
/// `storage.type`, `support.type`.
const MONOKAI_BLUE: u32 = 0x66_D9_EF;
/// `variable.parameter`.
const MONOKAI_ORANGE: u32 = 0xFD_97_1F;

/// **Monokai**: pink keywords, yellow strings, purple numbers, green names and blue types.
pub const MONOKAI: CodePalette = CodePalette {
    ground: MONOKAI_BACKGROUND,
    text: MONOKAI_FOREGROUND,
    inks: [
        (SyntaxRole::Key, MONOKAI_GREEN),
        (SyntaxRole::String, MONOKAI_YELLOW),
        (SyntaxRole::Number, MONOKAI_PURPLE),
        (SyntaxRole::Keyword, MONOKAI_PINK),
        (SyntaxRole::Reference, MONOKAI_ORANGE),
        (SyntaxRole::Function, MONOKAI_GREEN),
        (SyntaxRole::Type, MONOKAI_BLUE),
    ],
};

// ── Solarized (Ethan Schoonover): its dark ground and body text, and the accents both its modes share ─────────

/// `base03`, the dark mode's background.
const SOLARIZED_BASE03: u32 = 0x00_2B_36;
/// `base0`, the dark mode's body text.
const SOLARIZED_BASE0: u32 = 0x83_94_96;
/// `Statement`.
const SOLARIZED_GREEN: u32 = 0x85_99_00;
/// `Constant`: a string.
const SOLARIZED_CYAN: u32 = 0x2A_A1_98;
/// A number, as Solarized's own VS Code port draws `constant.numeric`.
const SOLARIZED_MAGENTA: u32 = 0xD3_36_82;
/// `Identifier`, `Function`, and a YAML key (`entity.name.tag`).
const SOLARIZED_BLUE: u32 = 0x26_8B_D2;
/// `Type`.
const SOLARIZED_YELLOW: u32 = 0xB5_89_00;
/// `Underlined`: what points somewhere else.
const SOLARIZED_VIOLET: u32 = 0x6C_71_C4;

/// **Solarized**: green keywords, cyan strings, magenta numbers, blue names and yellow types.
pub const SOLARIZED: CodePalette = CodePalette {
    ground: SOLARIZED_BASE03,
    text: SOLARIZED_BASE0,
    inks: [
        (SyntaxRole::Key, SOLARIZED_BLUE),
        (SyntaxRole::String, SOLARIZED_CYAN),
        (SyntaxRole::Number, SOLARIZED_MAGENTA),
        (SyntaxRole::Keyword, SOLARIZED_GREEN),
        (SyntaxRole::Reference, SOLARIZED_VIOLET),
        (SyntaxRole::Function, SOLARIZED_BLUE),
        (SyntaxRole::Type, SOLARIZED_YELLOW),
    ],
};

// ── GitHub: the light theme github.com highlights code with (Primer's `prettylights`) ───────────────────────

const GITHUB_CANVAS: u32 = 0xFF_FF_FF;
const GITHUB_FOREGROUND: u32 = 0x24_29_2E;
/// `pl-k`: a keyword.
const GITHUB_RED: u32 = 0xD7_3A_49;
/// `pl-s`: a string.
const GITHUB_NAVY: u32 = 0x03_2F_62;
/// `pl-c1`: a constant, a number.
const GITHUB_BLUE: u32 = 0x00_5C_C5;
/// `pl-en`: an entity's name, a function or a type.
const GITHUB_PURPLE: u32 = 0x6F_42_C1;
/// `pl-ent`: a tag, a YAML key.
const GITHUB_GREEN: u32 = 0x22_86_3A;
/// `pl-v`: a variable.
const GITHUB_ORANGE: u32 = 0xE3_62_09;

/// **GitHub**: red keywords, navy strings, blue numbers, green keys and purple names.
pub const GITHUB: CodePalette = CodePalette {
    ground: GITHUB_CANVAS,
    text: GITHUB_FOREGROUND,
    inks: [
        (SyntaxRole::Key, GITHUB_GREEN),
        (SyntaxRole::String, GITHUB_NAVY),
        (SyntaxRole::Number, GITHUB_BLUE),
        (SyntaxRole::Keyword, GITHUB_RED),
        (SyntaxRole::Reference, GITHUB_ORANGE),
        (SyntaxRole::Function, GITHUB_PURPLE),
        (SyntaxRole::Type, GITHUB_PURPLE),
    ],
};

/// A published `0xRRGGBB` colour in Oklch.
fn oklch(hex: u32) -> Oklcha {
    let [_, r, g, b] = hex.to_be_bytes();
    Oklcha::from(Color::srgb_u8(r, g, b))
}

impl CodePalette {
    /// **This palette as a gooey theme**: each role at its colour's own hue and chroma (at most gooey's ceiling),
    /// its lightness search started at the share of the way from the palette's text to its ground that the colour
    /// sits at. gooey moves it toward the skin's text from there until it holds the bar.
    pub fn theme(&self) -> SyntaxTheme {
        let text = oklch(self.text).lightness;
        let ground = oklch(self.ground).lightness;
        SyntaxTheme {
            inks: self
                .inks
                .iter()
                .map(|&(role, hex)| {
                    let ink = oklch(hex);
                    SyntaxInk::new(role, ink.hue)
                        .chroma(ink.chroma.min(INK_CHROMA_MAX))
                        .shift(((ink.lightness - text) / (ground - text)).clamp(0.0, 1.0))
                })
                .collect(),
        }
    }
}
