//! Marks and row notes for a host (an evidence checker): painted ranges and per-row notes that
//! are view state, not workbook content (see `crate::marks`). Registered like queries (no undo
//! step, no journal entry): a recorded script is saved in the workbook, and a mark must not be.

use serde_json::{Value as Json, json};

use super::*;
use crate::marks::{self, AiReview, MAX_ITEMS, MAX_KEYS, MAX_LIST, Mark, MarkSet, NoteAxis, NoteSource, RowNote, cap_text};

/// Reasons kept in a result's `skipped` list.
const MAX_REPORTED: usize = 50;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(query "marks.set", "Set Marks", [], None,
            "{key: \"evidence\", ranges: [{sheet, range: \"B3:D7\" | \"3:5\", stroke: \"#16a34a\", fill: \"rgba(22,163,74,0.18)\", strokeWidth?}]} → applied, missingSheets, skipped, dropped (view only: never saved, never undone)",
            has_doc, set),
        cmd!(query "marks.clear", "Clear Marks", [], None, "{key?} → cleared (all keys when no key)", has_doc, clear),
        cmd!(query "marks.visible", "Show Marks", [], None, "{key, on: bool} → key, on (hidden marks keep their ranges)", has_doc, visible),
        cmd!(query "notes.set", "Set Row Notes", [], None,
            "{notes: [{sheet, row: 1-based, verdict, sentence, unsupportedNumbers: [text], aiReview?: {verdict, reason}, axes?: [{axis, verdict, sources: [{path, locator, text}]}]}]} → applied, missingSheets, skipped, dropped",
            has_doc, notes_set),
        cmd!(query "workbook.sheets", "List Sheets", [], None, "{} → sheets: [name], active: name", has_doc, sheets),
    ]
}

fn sheet_names(s: &Session) -> Result<Vec<String>> {
    Ok(s.doc()?.wb.sheets.iter().map(|sh| sh.name.clone()).collect())
}

/// Resolve each item's sheet; remember the names that match nothing (once each, in order).
struct Resolver {
    names: Vec<String>,
    missing: Vec<String>,
}

impl Resolver {
    fn new(s: &Session) -> Result<Resolver> {
        Ok(Resolver { names: sheet_names(s)?, missing: vec![] })
    }
    fn resolve(&mut self, want: &str) -> Option<String> {
        let names: Vec<&str> = self.names.iter().map(String::as_str).collect();
        let found = marks::resolve_sheet(&names, want).map(str::to_string);
        if found.is_none() && !self.missing.iter().any(|m| m == want) && self.missing.len() < MAX_REPORTED {
            self.missing.push(cap_text(want));
        }
        found
    }
}

fn items<'a>(cmd: &str, p: &'a Json, key: &str) -> Result<&'a [Json]> {
    match p.get(key) {
        Some(Json::Array(a)) => Ok(a),
        Some(_) => Err(bad(cmd, format!("`{key}` must be an array"))),
        None => Err(bad(cmd, format!("missing `{key}`"))),
    }
}

fn key_param(cmd: &str, p: &Json) -> Result<String> {
    let key = str_param(p, "key").map(str::trim).filter(|k| !k.is_empty()).ok_or_else(|| bad(cmd, "missing `key`"))?;
    if key.chars().count() > 200 {
        return Err(bad(cmd, "`key` is longer than 200 characters"));
    }
    Ok(key.to_string())
}

fn mark(item: &Json, resolver: &mut Resolver) -> std::result::Result<Option<Mark>, String> {
    let sheet = item.get("sheet").and_then(Json::as_str).ok_or("missing `sheet`")?;
    let text = item.get("range").and_then(Json::as_str).ok_or("missing `range`")?;
    let range = RangeRef::parse(text).ok_or_else(|| format!("`{}` is not an A1 range", cap_text(text)))?;
    // Cut to the sheet (RangeRef::parse already refuses rows and columns past its edge).
    let range = range.intersection(&RangeRef::all()).ok_or("the range is outside the sheet")?;
    let colour = |name: &str| -> std::result::Result<[u8; 4], String> {
        let t = item.get(name).and_then(Json::as_str).ok_or_else(|| format!("missing `{name}`"))?;
        marks::parse_css_color(t).ok_or_else(|| format!("`{name}` `{}` is not a CSS colour", cap_text(t)))
    };
    let (stroke, fill) = (colour("stroke")?, colour("fill")?);
    let width = item.get("strokeWidth").and_then(Json::as_f64).filter(|w| w.is_finite()).unwrap_or(1.0).clamp(0.5, 8.0) as f32;
    Ok(resolver.resolve(sheet).map(|sheet| Mark { sheet, range, stroke, fill, stroke_width: width }))
}

fn report(applied: usize, resolver: Resolver, skipped: Vec<Json>, dropped: usize) -> Json {
    json!({"applied": applied, "missingSheets": resolver.missing, "skipped": skipped, "dropped": dropped})
}

fn set(s: &mut Session, p: &Json) -> Result<Json> {
    let key = key_param("marks.set", p)?;
    let all = items("marks.set", p, "ranges")?;
    let d = s.doc()?;
    if !d.marks.iter().any(|m| m.key == key) && d.marks.len() >= MAX_KEYS {
        return Err(bad("marks.set", format!("at most {MAX_KEYS} mark keys; clear one first")));
    }
    let mut resolver = Resolver::new(s)?;
    let (mut kept, mut skipped) = (Vec::new(), Vec::new());
    for (i, item) in all.iter().take(MAX_ITEMS).enumerate() {
        match mark(item, &mut resolver) {
            Ok(Some(m)) => kept.push(m),
            Ok(None) => {}
            Err(reason) => {
                if skipped.len() < MAX_REPORTED {
                    skipped.push(json!({"index": i, "reason": reason}));
                }
            }
        }
    }
    let applied = kept.len();
    let d = s.doc_mut()?;
    match d.marks.iter_mut().find(|m| m.key == key) {
        Some(set) => set.items = kept,
        None => d.marks.push(MarkSet { key, visible: true, items: kept }),
    }
    Ok(report(applied, resolver, skipped, all.len().saturating_sub(MAX_ITEMS)))
}

fn clear(s: &mut Session, p: &Json) -> Result<Json> {
    let d = s.doc_mut()?;
    let before = d.marks.len();
    match str_param(p, "key") {
        Some(key) => d.marks.retain(|m| m.key != key),
        None => d.marks.clear(),
    }
    Ok(json!({"cleared": before - d.marks.len()}))
}

fn visible(s: &mut Session, p: &Json) -> Result<Json> {
    let key = key_param("marks.visible", p)?;
    let on = p.get("on").and_then(Json::as_bool).ok_or_else(|| bad("marks.visible", "missing `on` (true or false)"))?;
    let d = s.doc_mut()?;
    let set = d.marks.iter_mut().find(|m| m.key == key).ok_or_else(|| bad("marks.visible", format!("no marks under `{key}`")))?;
    set.visible = on;
    Ok(json!({"key": key, "on": on}))
}

fn text(item: &Json, key: &str) -> String {
    item.get(key).and_then(Json::as_str).map(cap_text).unwrap_or_default()
}

fn list<'a>(item: &'a Json, key: &str) -> impl Iterator<Item = &'a Json> + 'a {
    item.get(key).and_then(Json::as_array).map(|a| a.as_slice()).unwrap_or_default().iter().take(MAX_LIST)
}

fn note(item: &Json, resolver: &mut Resolver) -> std::result::Result<Option<RowNote>, String> {
    let sheet = item.get("sheet").and_then(Json::as_str).ok_or("missing `sheet`")?;
    let row = item
        .get("row")
        .and_then(Json::as_f64)
        .filter(|r| r.is_finite() && r.fract() == 0.0 && *r >= 1.0 && *r <= gridcraft_core::MAX_ROWS as f64)
        .ok_or("`row` must be a whole number from 1 to 1048576")? as u32;
    let unsupported_numbers =
        list(item, "unsupportedNumbers").filter_map(|n| n.as_str().map(cap_text).or_else(|| n.as_f64().map(|f| f.to_string()))).collect();
    let ai_review = item.get("aiReview").filter(|a| a.is_object()).map(|a| AiReview { verdict: text(a, "verdict"), reason: text(a, "reason") });
    let axes = list(item, "axes")
        .map(|a| NoteAxis {
            axis: text(a, "axis"),
            verdict: text(a, "verdict"),
            sources: list(a, "sources").map(|s| NoteSource { path: text(s, "path"), locator: text(s, "locator"), text: text(s, "text") }).collect(),
        })
        .collect();
    Ok(resolver.resolve(sheet).map(|sheet| RowNote {
        sheet,
        row,
        verdict: text(item, "verdict"),
        sentence: text(item, "sentence"),
        unsupported_numbers,
        ai_review,
        axes,
    }))
}

fn notes_set(s: &mut Session, p: &Json) -> Result<Json> {
    let all = items("notes.set", p, "notes")?;
    let mut resolver = Resolver::new(s)?;
    let (mut kept, mut skipped) = (Vec::new(), Vec::new());
    for (i, item) in all.iter().take(MAX_ITEMS).enumerate() {
        match note(item, &mut resolver) {
            Ok(Some(n)) => kept.push(n),
            Ok(None) => {}
            Err(reason) => {
                if skipped.len() < MAX_REPORTED {
                    skipped.push(json!({"index": i, "reason": reason}));
                }
            }
        }
    }
    let applied = kept.len();
    s.doc_mut()?.notes = kept;
    Ok(report(applied, resolver, skipped, all.len().saturating_sub(MAX_ITEMS)))
}

fn sheets(s: &mut Session, _: &Json) -> Result<Json> {
    let d = s.doc()?;
    let names: Vec<&str> = d.wb.sheets.iter().map(|sh| sh.name.as_str()).collect();
    Ok(json!({"sheets": names, "active": d.wb.active().map(|sh| sh.name.as_str())}))
}
