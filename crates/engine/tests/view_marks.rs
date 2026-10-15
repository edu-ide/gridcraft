//! Marks (`marks.set`/`marks.clear`/`marks.visible`) and row notes (`notes.set`) are view state
//! a host paints over the grid: they never reach the saved file, the undo history or the
//! unsaved-changes flag, and hostile input is capped without a panic.

use gridcraft_engine::{EngineError, Session, io};
use serde_json::{Value as Json, json};

fn session() -> Session {
    let mut s = Session::new();
    s.new_workbook();
    s.execute("range.setValues", json!({"range": "A1", "values": [["Item", "Qty"], ["Red", 1], ["Blue", 2], ["Green", 3]]})).unwrap();
    s.execute("home.insertSheet", json!({"name": "1. 사업장 생산 data"})).unwrap();
    s.execute("sheet.activate", json!({"sheet": 0})).unwrap();
    s
}

fn xlsx(s: &Session) -> Vec<u8> {
    io::save_bytes(&s.doc().unwrap().wb, "book.xlsx").unwrap()
}

fn row_marks() -> Json {
    json!({"key": "evidence", "ranges": [
        {"sheet": "Sheet1", "range": "A2:B3", "stroke": "#16a34a", "fill": "rgba(22,163,74,0.18)"},
        {"sheet": "Sheet1", "range": "4:4", "stroke": "#dc2626", "fill": "rgba(220,38,38,0.18)", "strokeWidth": 2},
    ]})
}

#[test]
fn marks_and_notes_leave_the_saved_file_byte_for_byte() {
    let mut s = session();
    let before = xlsx(&s);
    // The writer is deterministic (a second save of the same workbook is the same bytes), so
    // any difference below is the marks leaking into the file.
    assert_eq!(before, xlsx(&s), "control: two saves of one workbook");
    let r = s.execute("marks.set", row_marks()).unwrap();
    assert_eq!(r["applied"], 2, "{r}");
    s.execute(
        "notes.set",
        json!({"notes": [{"sheet": "Sheet1", "row": 2, "verdict": "supported", "sentence": "Red is 1", "unsupportedNumbers": []}]}),
    )
    .unwrap();
    s.execute("marks.visible", json!({"key": "evidence", "on": false})).unwrap();
    assert_eq!(before, xlsx(&s), "marks and notes reached the xlsx");

    // And through the real Save command, to a file.
    let dir = std::env::temp_dir().join(format!("gridcraft-marks-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (a, b) = (dir.join("a.xlsx"), dir.join("b.xlsx"));
    s.execute("marks.clear", json!({})).unwrap();
    s.execute("file.saveAs", json!({"path": a.to_string_lossy()})).unwrap();
    s.execute("marks.set", row_marks()).unwrap();
    s.execute("file.saveAs", json!({"path": b.to_string_lossy()})).unwrap();
    let (a_bytes, b_bytes) = (std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(a_bytes, b_bytes, "file.saveAs with marks differs from without");
}

#[test]
fn marks_and_notes_leave_undo_and_dirty_alone() {
    let mut s = Session::new();
    s.new_workbook();
    let d = s.doc().unwrap();
    let (undo, revision, dirty) = (d.undo.len(), d.revision, d.is_dirty());
    assert!(!dirty);
    for (id, p) in [
        ("marks.set", row_marks()),
        ("notes.set", json!({"notes": [{"sheet": "Sheet1", "row": 3, "verdict": "x", "sentence": "y", "unsupportedNumbers": ["12"]}]})),
        ("marks.visible", json!({"key": "evidence", "on": false})),
        ("marks.visible", json!({"key": "evidence", "on": true})),
        ("marks.clear", json!({"key": "evidence"})),
        ("marks.clear", json!({})),
        ("workbook.sheets", json!({})),
    ] {
        s.execute(id, p).unwrap_or_else(|e| panic!("{id}: {e}"));
        let d = s.doc().unwrap();
        assert_eq!((d.undo.len(), d.revision, d.is_dirty()), (undo, revision, false), "{id} touched history or dirty");
    }
    assert!(s.execute("history.list", json!({})).unwrap()["undo"].as_array().unwrap().is_empty());
    assert!(s.journal.iter().all(|(id, _)| !id.starts_with("marks.") && !id.starts_with("notes.")), "marks never reach a recorded script");

    // A workbook with unsaved changes keeps them, and its saved copy, as they were.
    s.execute("cell.set", json!({"cell": "A1", "input": "5"})).unwrap();
    let d = s.doc().unwrap();
    let (saved, undo) = (d.saved.clone(), d.undo.len());
    s.execute("marks.set", row_marks()).unwrap();
    let d = s.doc().unwrap();
    assert!(d.is_dirty());
    assert!(std::sync::Arc::ptr_eq(&saved, &d.saved));
    assert_eq!(d.undo.len(), undo);
}

#[test]
fn sheet_names_match_exactly_then_loosely() {
    let mut s = session();
    s.execute("home.insertSheet", json!({"name": "Sheet10"})).unwrap();
    s.execute("sheet.activate", json!({"sheet": "Sheet1"})).unwrap();
    let r = s
        .execute(
            "marks.set",
            json!({"key": "k", "ranges": [
                {"sheet": "1.사업장생산DATA", "range": "B3:D7", "stroke": "#000", "fill": "#ff000033"},
                {"sheet": "sheet1", "range": "A1", "stroke": "#000", "fill": "#fff"},
                {"sheet": "Sheet10", "range": "A1", "stroke": "#000", "fill": "#fff"},
            ]}),
        )
        .unwrap();
    assert_eq!(r["applied"], 3, "{r}");
    assert_eq!(r["missingSheets"], json!([]), "{r}");
    let marks = &s.doc().unwrap().marks;
    let sheets: Vec<&str> = marks.iter().flat_map(|m| m.items.iter().map(|i| i.sheet.as_str())).collect();
    // `sheet1` is Sheet1 (case only), never Sheet10, which contains it.
    assert_eq!(sheets, ["1. 사업장 생산 data", "Sheet1", "Sheet10"]);

    let names = s.execute("workbook.sheets", json!({})).unwrap();
    assert_eq!(names, json!({"sheets": ["Sheet1", "Sheet10", "1. 사업장 생산 data"], "active": "Sheet1"}));
}

#[test]
fn missing_sheets_are_reported_once_each() {
    let mut s = session();
    let r = s
        .execute(
            "marks.set",
            json!({"key": "k", "ranges": [
                {"sheet": "Nope", "range": "A1", "stroke": "#000", "fill": "#fff"},
                {"sheet": "Nope", "range": "A2", "stroke": "#000", "fill": "#fff"},
                {"sheet": "Sheet1", "range": "A3", "stroke": "#000", "fill": "#fff"},
            ]}),
        )
        .unwrap();
    assert_eq!((r["applied"].as_u64(), r["missingSheets"].clone()), (Some(1), json!(["Nope"])), "{r}");
    let r =
        s.execute("notes.set", json!({"notes": [{"sheet": "Gone", "row": 1, "verdict": "v", "sentence": "s", "unsupportedNumbers": []}]})).unwrap();
    assert_eq!((r["applied"].as_u64(), r["missingSheets"].clone()), (Some(0), json!(["Gone"])), "{r}");
}

#[test]
fn whole_rows_and_a1_ranges_are_cut_to_the_sheet() {
    let mut s = session();
    s.execute(
        "marks.set",
        json!({"key": "k", "ranges": [
            {"sheet": "Sheet1", "range": "3:5", "stroke": "#000", "fill": "#fff"},
            {"sheet": "Sheet1", "range": "7:7", "stroke": "#000", "fill": "#fff"},
            {"sheet": "Sheet1", "range": "B3:D7", "stroke": "#000", "fill": "#fff"},
        ]}),
    )
    .unwrap();
    let items: Vec<String> = s.doc().unwrap().marks[0].items.iter().map(|i| i.range.a1()).collect();
    assert_eq!(items, ["3:5", "7:7", "B3:D7"]);
    let rows: Vec<(u32, u32, u32)> = s.doc().unwrap().marks[0].items.iter().map(|i| (i.range.start.row, i.range.end.row, i.range.end.col)).collect();
    assert_eq!(rows[0], (2, 4, gridcraft_engine::core::MAX_COLS - 1), "whole rows end at the sheet's last column");
}

#[test]
fn set_replaces_one_key_and_clear_and_visible_act_on_keys() {
    let mut s = session();
    let one = |range: &str| json!({"sheet": "Sheet1", "range": range, "stroke": "#000", "fill": "#fff"});
    s.execute("marks.set", json!({"key": "a", "ranges": [one("A1"), one("A2")]})).unwrap();
    s.execute("marks.set", json!({"key": "b", "ranges": [one("B1")]})).unwrap();
    s.execute("marks.set", json!({"key": "a", "ranges": [one("C1")]})).unwrap();
    let doc = s.doc().unwrap();
    let a = doc.marks.iter().find(|m| m.key == "a").unwrap();
    assert_eq!(a.items.iter().map(|i| i.range.a1()).collect::<Vec<_>>(), ["C1"], "set replaces the key's marks");
    assert!(doc.marks.iter().any(|m| m.key == "b"), "other keys stay");

    s.execute("marks.visible", json!({"key": "b", "on": false})).unwrap();
    assert!(!s.doc().unwrap().marks.iter().find(|m| m.key == "b").unwrap().visible);
    assert!(s.doc().unwrap().marks.iter().find(|m| m.key == "b").unwrap().items.len() == 1, "hiding keeps the data");
    s.execute("marks.visible", json!({"key": "b", "on": true})).unwrap();
    assert!(s.doc().unwrap().marks.iter().find(|m| m.key == "b").unwrap().visible);
    assert!(s.execute("marks.visible", json!({"key": "zzz", "on": true})).is_err(), "an unknown key is an error");

    s.execute("marks.clear", json!({"key": "a"})).unwrap();
    assert_eq!(s.doc().unwrap().marks.iter().map(|m| m.key.as_str()).collect::<Vec<_>>(), ["b"]);
    s.execute("marks.clear", json!({})).unwrap();
    assert!(s.doc().unwrap().marks.is_empty());

    // Visibility set before any marks sticks to the key when they arrive.
    s.execute("marks.set", json!({"key": "c", "ranges": [one("A1")]})).unwrap();
    s.execute("marks.visible", json!({"key": "c", "on": false})).unwrap();
    s.execute("marks.set", json!({"key": "c", "ranges": [one("A2")]})).unwrap();
    assert!(!s.doc().unwrap().marks.iter().find(|m| m.key == "c").unwrap().visible, "replacing the marks keeps them hidden");
}

#[test]
fn notes_are_found_by_sheet_and_row() {
    let mut s = session();
    let r = s
        .execute(
            "notes.set",
            json!({"notes": [
                {"sheet": "1.사업장생산DATA", "row": 3, "verdict": "unsupported", "sentence": "생산량 120톤", "unsupportedNumbers": ["120"],
                 "aiReview": {"verdict": "unsupported", "reason": "출처에 없음"},
                 "axes": [{"axis": "수치", "verdict": "unsupported", "sources": [{"path": "a.xlsx", "locator": "B3", "text": "110"}]}]},
                {"sheet": "Sheet1", "row": 2, "verdict": "supported", "sentence": "Red", "unsupportedNumbers": []},
            ]}),
        )
        .unwrap();
    assert_eq!(r["applied"], 2, "{r}");
    let d = s.doc().unwrap();
    let n = d.note_at("1. 사업장 생산 data", 3).unwrap();
    assert_eq!((n.verdict.as_str(), n.sentence.as_str(), n.unsupported_numbers.clone()), ("unsupported", "생산량 120톤", vec!["120".to_string()]));
    assert_eq!(n.ai_review.as_ref().map(|a| a.reason.as_str()), Some("출처에 없음"));
    assert_eq!(n.axes.first().and_then(|a| a.sources.first()).map(|s| s.locator.as_str()), Some("B3"));
    assert!(d.note_at("Sheet1", 2).is_some());
    assert!(d.note_at("Sheet1", 3).is_none());
    // Replaced, not added to.
    s.execute("notes.set", json!({"notes": []})).unwrap();
    assert!(s.doc().unwrap().note_at("Sheet1", 2).is_none());
}

#[test]
fn hostile_input_is_capped_without_a_panic() {
    let mut s = session();
    // Tens of thousands of items: only the first 10,000 are kept.
    let many: Vec<Json> =
        (0..30_000).map(|i| json!({"sheet": "Sheet1", "range": format!("A{}", i % 1000 + 1), "stroke": "#000", "fill": "#fff"})).collect();
    let r = s.execute("marks.set", json!({"key": "many", "ranges": many})).unwrap();
    assert_eq!(r["applied"], 10_000, "{r}");
    assert_eq!(r["dropped"], 20_000, "{r}");
    let notes: Vec<Json> =
        (0..30_000).map(|i| json!({"sheet": "Sheet1", "row": i + 1, "verdict": "v", "sentence": "s", "unsupportedNumbers": []})).collect();
    let r = s.execute("notes.set", json!({"notes": notes})).unwrap();
    assert_eq!((r["applied"].as_u64(), r["dropped"].as_u64()), (Some(10_000), Some(20_000)), "{r}");

    // Giant and broken ranges, NaN-ish widths (JSON carries NaN as null), bad colours, wrong types.
    let r = s
        .execute(
            "marks.set",
            json!({"key": "bad", "ranges": [
                {"sheet": "Sheet1", "range": "A1:XFD1048576", "stroke": "#000", "fill": "#fff", "strokeWidth": null},
                {"sheet": "Sheet1", "range": "1:1048576", "stroke": "#000", "fill": "#fff", "strokeWidth": 1e308},
                {"sheet": "Sheet1", "range": "A1:ZZZZ99999999", "stroke": "#000", "fill": "#fff"},
                {"sheet": "Sheet1", "range": "0:5", "stroke": "#000", "fill": "#fff"},
                {"sheet": "Sheet1", "range": "A1", "stroke": "rgba(1,2,3,NaN)", "fill": "#fff"},
                {"sheet": "Sheet1", "range": "A1", "stroke": "#12345", "fill": "#fff"},
                {"sheet": "Sheet1", "range": 5, "stroke": "#000", "fill": "#fff"},
                {"sheet": 3, "range": "A1", "stroke": "#000", "fill": "#fff"},
                "not an object",
                {"sheet": "Sheet1", "range": "A1", "stroke": "#000", "fill": "#fff", "strokeWidth": -4},
            ]}),
        )
        .unwrap();
    assert_eq!(r["applied"], 3, "{r}");
    assert_eq!(r["skipped"].as_array().map(Vec::len), Some(7), "{r}");
    let bad = s.doc().unwrap().marks.iter().find(|m| m.key == "bad").unwrap();
    assert!(bad.items.iter().all(|i| i.stroke_width.is_finite() && (0.5..=8.0).contains(&i.stroke_width)));

    for p in [
        json!({}),
        json!({"key": 1}),
        json!({"key": "k", "ranges": "A1"}),
        json!({"key": "x".repeat(100_000), "ranges": []}),
        json!({"notes": {"sheet": "Sheet1"}}),
        json!({"notes": [{"sheet": "Sheet1", "row": null}, {"sheet": "Sheet1", "row": -1}, {"sheet": "Sheet1", "row": 1e300}, {"sheet": "Sheet1", "row": 2.5}]}),
        json!({"key": "k", "on": "yes"}),
    ] {
        for id in ["marks.set", "marks.clear", "marks.visible", "notes.set", "workbook.sheets"] {
            let r = s.execute(id, p.clone());
            assert!(!matches!(r, Err(EngineError::Internal(..))), "{id} {p}: {r:?}");
        }
    }
    // Too many keys: the 65th is refused instead of growing without bound.
    s.execute("marks.clear", json!({})).unwrap();
    for i in 0..64 {
        s.execute("marks.set", json!({"key": format!("k{i}"), "ranges": []})).unwrap();
    }
    assert!(s.execute("marks.set", json!({"key": "one-more", "ranges": []})).is_err());
    assert!(s.execute("marks.set", json!({"key": "k3", "ranges": []})).is_ok(), "an existing key can still be replaced");
}

#[test]
fn css_colours_parse_to_rgba() {
    use gridcraft_engine::marks::parse_css_color as c;
    assert_eq!(c("#f59e0b"), Some([0xf5, 0x9e, 0x0b, 255]));
    assert_eq!(c("#FFF"), Some([255, 255, 255, 255]));
    assert_eq!(c("#ff000080"), Some([255, 0, 0, 128]));
    assert_eq!(c("rgba(250, 204, 21, 0.28)"), Some([250, 204, 21, 71]));
    assert_eq!(c("rgb(1,2,3)"), Some([1, 2, 3, 255]));
    assert_eq!(c("transparent"), Some([0, 0, 0, 0]));
    assert_eq!(c("rgba(300,0,0,2)"), Some([255, 0, 0, 255]), "out-of-range channels are clamped");
    for bad in ["", "#", "#12345", "red-ish", "rgba(1,2)", "rgba(a,b,c,d)", "rgba(1,2,3,NaN)", "#gggggg"] {
        assert_eq!(c(bad), None, "{bad}");
    }
}
