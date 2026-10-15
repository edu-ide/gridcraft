//! Marks and row notes: view state a host paints over the grid (evidence highlights, a source
//! range, a verdict per row). They live on the [`DocState`](crate::DocState) beside the
//! workbook, never in it, so they never reach a saved file, the undo history or the
//! unsaved-changes flag. The commands are in `cmd/marks.rs`.

use gridcraft_core::RangeRef;

/// Items kept from one `marks.set` or `notes.set`; the rest are dropped and counted.
pub const MAX_ITEMS: usize = 10_000;
/// Mark keys kept on a document at once.
pub const MAX_KEYS: usize = 64;
/// The longest key, sheet name or note text kept (characters).
pub const MAX_TEXT: usize = 4_000;
/// Entries kept from a note's lists (unsupported numbers, axes, sources).
pub const MAX_LIST: usize = 100;

/// One painted range: `fill` under the cell text, `stroke` around it on top.
#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    /// The sheet's real name (a loose name is resolved when the mark is set).
    pub sheet: String,
    pub range: RangeRef,
    /// RGBA.
    pub stroke: [u8; 4],
    pub fill: [u8; 4],
    /// Points at 100% zoom, 0.5..=8.
    pub stroke_width: f32,
}

/// The marks set under one key (`evidence`, `source`…), replaced as a whole.
#[derive(Clone, Debug, PartialEq)]
pub struct MarkSet {
    pub key: String,
    pub visible: bool,
    pub items: Vec<Mark>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AiReview {
    pub verdict: String,
    pub reason: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NoteSource {
    pub path: String,
    pub locator: String,
    pub text: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NoteAxis {
    pub axis: String,
    pub verdict: String,
    pub sources: Vec<NoteSource>,
}

/// The note shown when a marked row is clicked.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RowNote {
    /// The sheet's real name.
    pub sheet: String,
    /// 1-based, as the host counts rows.
    pub row: u32,
    pub verdict: String,
    pub sentence: String,
    pub unsupported_numbers: Vec<String>,
    pub ai_review: Option<AiReview>,
    pub axes: Vec<NoteAxis>,
}

impl crate::DocState {
    /// The note on `row` (1-based) of the sheet named `sheet`.
    pub fn note_at(&self, sheet: &str, row: u32) -> Option<&RowNote> {
        self.notes.iter().find(|n| n.row == row && n.sheet == sheet)
    }

    /// The visible marks on the sheet named `sheet`.
    pub fn visible_marks<'a>(&'a self, sheet: &'a str) -> impl Iterator<Item = &'a Mark> + 'a {
        self.marks.iter().filter(|m| m.visible).flat_map(|m| m.items.iter()).filter(move |i| i.sheet == sheet)
    }

    /// A visible mark covers some cell of `row` (0-based) on the sheet named `sheet`.
    pub fn row_marked(&self, sheet: &str, row: u32) -> bool {
        self.visible_marks(sheet).any(|m| m.range.start.row <= row && row <= m.range.end.row)
    }
}

/// Spaces dropped, lower case: `1. 사업장 생산 data` and `1.사업장생산DATA` are one name.
fn squish(name: &str) -> String {
    name.chars().filter(|c| !c.is_whitespace()).flat_map(char::to_lowercase).collect()
}

/// The real name of the sheet `want` means: an exact match first, then one equal once spaces
/// and case are ignored, then one that contains the other that way (references often shorten
/// names). `None` when nothing matches.
pub fn resolve_sheet<'a>(names: &[&'a str], want: &str) -> Option<&'a str> {
    if let Some(n) = names.iter().find(|n| **n == want) {
        return Some(n);
    }
    let w = squish(want);
    if w.is_empty() {
        return None;
    }
    let squished: Vec<(String, &'a str)> = names.iter().map(|n| (squish(n), *n)).collect();
    if let Some((_, n)) = squished.iter().find(|(s, _)| *s == w) {
        return Some(n);
    }
    squished.iter().find(|(s, _)| !s.is_empty() && (s.contains(&w) || w.contains(s.as_str()))).map(|(_, n)| *n)
}

/// `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, `rgb(r, g, b)`, `rgba(r, g, b, a)` (a 0..1) or
/// `transparent` as RGBA. Channels out of range are clamped; anything else is `None`.
pub fn parse_css_color(text: &str) -> Option<[u8; 4]> {
    let t = text.trim();
    if t.eq_ignore_ascii_case("transparent") {
        return Some([0, 0, 0, 0]);
    }
    if let Some(hex) = t.strip_prefix('#') {
        if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let digit = |i: usize| hex.get(i..i + 1).and_then(|d| u8::from_str_radix(d, 16).ok());
        let pair = |i: usize| hex.get(i..i + 2).and_then(|d| u8::from_str_radix(d, 16).ok());
        return match hex.len() {
            3 | 4 => {
                let mut out = [255u8; 4];
                for (i, slot) in out.iter_mut().enumerate().take(hex.len()) {
                    *slot = digit(i)? * 17;
                }
                Some(out)
            }
            6 | 8 => {
                let mut out = [255u8; 4];
                for (i, slot) in out.iter_mut().enumerate().take(hex.len() / 2) {
                    *slot = pair(i * 2)?;
                }
                Some(out)
            }
            _ => None,
        };
    }
    let lower = t.to_ascii_lowercase();
    let (body, with_alpha) = if let Some(b) = lower.strip_prefix("rgba(") {
        (b, true)
    } else if let Some(b) = lower.strip_prefix("rgb(") {
        (b, false)
    } else {
        return None;
    };
    let body = body.strip_suffix(')')?;
    let parts: Vec<&str> = body.split(',').map(str::trim).collect();
    if parts.len() != if with_alpha { 4 } else { 3 } {
        return None;
    }
    let mut out = [255u8; 4];
    for (i, part) in parts.iter().enumerate() {
        let v: f64 = part.parse().ok().filter(|v: &f64| v.is_finite())?;
        let v = if i == 3 { v.clamp(0.0, 1.0) * 255.0 } else { v.clamp(0.0, 255.0) };
        *out.get_mut(i)? = v.round() as u8;
    }
    Some(out)
}

/// `text` cut to [`MAX_TEXT`] characters.
pub(crate) fn cap_text(text: &str) -> String {
    text.chars().take(MAX_TEXT).collect()
}
