//! A row note (`notes.set`) opens in place when its marked row (`marks.set`) is clicked, and
//! closes on Escape or a click elsewhere. Rows without a visible mark open nothing.

use egui::{Event, Key, Modifiers, PointerButton, Pos2};
use egui_kittest::kittest::Queryable;
use gridcraft_engine::Session;
use gridcraft_engine::core::CellRef;
use gridcraft_ui_egui::{SheetApp, grid};
use serde_json::json;

type Harness = egui_kittest::Harness<'static, SheetApp>;

fn harness() -> Harness {
    let mut session = Session::new();
    session.new_workbook();
    session
        .execute("range.setValues", json!({"range": "A1", "values": [["Item", "Qty"], ["Red", 1], ["Blue", 2], ["Green", 3], ["Grey", 4]]}))
        .unwrap();
    session
        .execute(
            "marks.set",
            json!({"key": "evidence", "ranges": [{"sheet": "sheet 1", "range": "3:3", "stroke": "#dc2626", "fill": "rgba(220,38,38,0.18)"}]}),
        )
        .unwrap();
    session
        .execute(
            "notes.set",
            json!({"notes": [
                {"sheet": "Sheet1", "row": 3, "verdict": "unsupported", "sentence": "Blue sold 2 units", "unsupportedNumbers": ["2"],
                 "aiReview": {"verdict": "unsupported", "reason": "not in the source"}, "axes": []},
                {"sheet": "Sheet1", "row": 5, "verdict": "supported", "sentence": "Grey has no mark", "unsupportedNumbers": []},
            ]}),
        )
        .unwrap();
    let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1200.0, 800.0)).build_ui_state(
        |ui, app: &mut SheetApp| {
            app.logic(ui.ctx());
            app.ui(ui);
        },
        SheetApp::new(session, Default::default()),
    );
    h.run_steps(4);
    h
}

fn cell_pos(h: &Harness, cell: &str) -> Pos2 {
    let app = h.state();
    let sh = app.session.active().unwrap().wb.active().unwrap();
    let geo = grid::Geo::new(sh, app.grid.rect.unwrap(), app.view().scroll);
    geo.cell_rect(sh, CellRef::parse(cell).unwrap()).center()
}

fn click(h: &mut Harness, cell: &str) {
    let pos = cell_pos(h, cell);
    h.input_mut().events.push(Event::PointerMoved(pos));
    h.input_mut().events.push(Event::PointerButton { pos, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.input_mut().events.push(Event::PointerButton { pos, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.run_steps(2);
}

fn is_open(h: &Harness) -> bool {
    h.state().grid.note_popover.is_some()
}

#[test]
fn clicking_a_marked_row_opens_its_note_and_escape_closes_it() {
    let mut h = harness();
    assert!(!is_open(&h));
    click(&mut h, "B3");
    assert!(is_open(&h), "the note on marked row 3 must open");
    assert!(h.query_by_label_contains("Blue sold 2 units").is_some(), "the sentence is shown");
    assert!(h.query_all_by_label_contains("unsupported").next().is_some(), "the verdict is shown");
    assert!(h.query_by_label_contains("not in the source").is_some(), "the AI review is shown");
    // The click still selects the cell as usual.
    assert_eq!(h.state().session.active().unwrap().selection.active.a1(), "B3");

    h.input_mut().events.push(Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE });
    h.run_steps(2);
    assert!(!is_open(&h), "Escape closes the note");
    assert!(h.query_by_label_contains("Blue sold 2 units").is_none());
}

#[test]
fn a_click_elsewhere_closes_the_note() {
    let mut h = harness();
    click(&mut h, "A3");
    assert!(is_open(&h));
    click(&mut h, "H12");
    assert!(!is_open(&h), "a click on an unmarked cell closes the note");
    assert_eq!(h.state().session.active().unwrap().selection.active.a1(), "H12");
}

#[test]
fn rows_without_a_visible_mark_open_nothing() {
    let mut h = harness();
    // Row 5 has a note but no mark; row 4 a neither; row 2 is next to the mark (1-based rows).
    for cell in ["A5", "A4", "A2"] {
        click(&mut h, cell);
        assert!(!is_open(&h), "{cell} opened a note");
    }
    // Hidden marks are not clickable either; shown again, they are.
    h.state_mut().run("marks.visible", json!({"key": "evidence", "on": false})).unwrap();
    h.run_steps(2);
    click(&mut h, "A3");
    assert!(!is_open(&h), "a hidden mark opened its note");
    h.state_mut().run("marks.visible", json!({"key": "evidence", "on": true})).unwrap();
    h.run_steps(2);
    click(&mut h, "C3");
    assert!(is_open(&h), "the shown mark opens its note again");
    // Cleared notes close an open one.
    h.state_mut().run("notes.set", json!({"notes": []})).unwrap();
    h.run_steps(2);
    assert!(!is_open(&h), "a note that no longer exists stays closed");
}
