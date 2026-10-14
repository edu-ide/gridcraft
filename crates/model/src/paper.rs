//! Paper sizes: the one table behind `<pageSetup paperSize>` (ECMA-376 §18.3.1.63), the names
//! Page Setup shows and the page dimensions a print or PDF export uses. Readers, writers and
//! renderers all go through it so a size can never be named without being printable, or
//! printable without surviving a round trip.

/// A paper's dimensions in the unit its standard defines it in (portrait).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Size {
    In(f32, f32),
    Mm(f32, f32),
}

impl Size {
    fn points(self) -> (f32, f32) {
        match self {
            Size::In(w, h) => (w * 72.0, h * 72.0),
            Size::Mm(w, h) => (w * 72.0 / 25.4, h * 72.0 / 25.4),
        }
    }
}

/// `paperSize` id, display name, size. Ids 48–49 are reserved and absent.
const PAPERS: &[(u32, &str, Size)] = &[
    (1, "Letter", Size::In(8.5, 11.0)),
    (2, "Letter Small", Size::In(8.5, 11.0)),
    (3, "Tabloid", Size::In(11.0, 17.0)),
    (4, "Ledger", Size::In(17.0, 11.0)),
    (5, "Legal", Size::In(8.5, 14.0)),
    (6, "Statement", Size::In(5.5, 8.5)),
    (7, "Executive", Size::In(7.25, 10.5)),
    (8, "A3", Size::Mm(297.0, 420.0)),
    (9, "A4", Size::Mm(210.0, 297.0)),
    (10, "A4 Small", Size::Mm(210.0, 297.0)),
    (11, "A5", Size::Mm(148.0, 210.0)),
    (12, "B4", Size::Mm(257.0, 364.0)),
    (13, "B5", Size::Mm(182.0, 257.0)),
    (14, "Folio", Size::In(8.5, 13.0)),
    (15, "Quarto", Size::Mm(215.0, 275.0)),
    (16, "10x14", Size::In(10.0, 14.0)),
    (17, "11x17", Size::In(11.0, 17.0)),
    (18, "Note", Size::In(8.5, 11.0)),
    (19, "Envelope #9", Size::In(3.875, 8.875)),
    (20, "Envelope #10", Size::In(4.125, 9.5)),
    (21, "Envelope #11", Size::In(4.5, 10.375)),
    (22, "Envelope #12", Size::In(4.75, 11.0)),
    (23, "Envelope #14", Size::In(5.0, 11.5)),
    (24, "C", Size::In(17.0, 22.0)),
    (25, "D", Size::In(22.0, 34.0)),
    (26, "E", Size::In(34.0, 44.0)),
    (27, "Envelope DL", Size::Mm(110.0, 220.0)),
    (28, "Envelope C5", Size::Mm(162.0, 229.0)),
    (29, "Envelope C3", Size::Mm(324.0, 458.0)),
    (30, "Envelope C4", Size::Mm(229.0, 324.0)),
    (31, "Envelope C6", Size::Mm(114.0, 162.0)),
    (32, "Envelope C65", Size::Mm(114.0, 229.0)),
    (33, "Envelope B4", Size::Mm(250.0, 353.0)),
    (34, "Envelope B5", Size::Mm(176.0, 250.0)),
    (35, "Envelope B6", Size::Mm(176.0, 125.0)),
    (36, "Envelope Italy", Size::Mm(110.0, 230.0)),
    (37, "Envelope Monarch", Size::In(3.875, 7.5)),
    (38, "Envelope 6 3/4", Size::In(3.625, 6.5)),
    (39, "US Standard Fanfold", Size::In(14.875, 11.0)),
    (40, "German Standard Fanfold", Size::In(8.5, 12.0)),
    (41, "German Legal Fanfold", Size::In(8.5, 13.0)),
    (42, "ISO B4", Size::Mm(250.0, 353.0)),
    (43, "Japanese Postcard", Size::Mm(100.0, 148.0)),
    (44, "9x11", Size::In(9.0, 11.0)),
    (45, "10x11", Size::In(10.0, 11.0)),
    (46, "15x11", Size::In(15.0, 11.0)),
    (47, "Envelope Invite", Size::Mm(220.0, 220.0)),
    (50, "Letter Extra", Size::In(9.275, 12.0)),
    (51, "Legal Extra", Size::In(9.275, 15.0)),
    (52, "Tabloid Extra", Size::In(11.69, 18.0)),
    (53, "A4 Extra", Size::Mm(236.0, 322.0)),
    (54, "Letter Transverse", Size::In(8.275, 11.0)),
    (55, "A4 Transverse", Size::Mm(210.0, 297.0)),
    (56, "Letter Extra Transverse", Size::In(9.275, 12.0)),
    (57, "SuperA", Size::Mm(227.0, 356.0)),
    (58, "SuperB", Size::Mm(305.0, 487.0)),
    (59, "Letter Plus", Size::In(8.5, 12.69)),
    (60, "A4 Plus", Size::Mm(210.0, 330.0)),
    (61, "A5 Transverse", Size::Mm(148.0, 210.0)),
    (62, "B5 Transverse", Size::Mm(182.0, 257.0)),
    (63, "A3 Extra", Size::Mm(322.0, 445.0)),
    (64, "A5 Extra", Size::Mm(174.0, 235.0)),
    (65, "ISO B5 Extra", Size::Mm(201.0, 276.0)),
    (66, "A2", Size::Mm(420.0, 594.0)),
    (67, "A3 Transverse", Size::Mm(297.0, 420.0)),
    (68, "A3 Extra Transverse", Size::Mm(322.0, 445.0)),
    (69, "Japanese Double Postcard", Size::Mm(200.0, 148.0)),
    (70, "A6", Size::Mm(105.0, 148.0)),
    (71, "Japanese Envelope Kaku #2", Size::Mm(240.0, 332.0)),
    (72, "Japanese Envelope Kaku #3", Size::Mm(216.0, 277.0)),
    (73, "Japanese Envelope Chou #3", Size::Mm(120.0, 235.0)),
    (74, "Japanese Envelope Chou #4", Size::Mm(90.0, 205.0)),
    (75, "Letter Rotated", Size::In(11.0, 8.5)),
    (76, "A3 Rotated", Size::Mm(420.0, 297.0)),
    (77, "A4 Rotated", Size::Mm(297.0, 210.0)),
    (78, "A5 Rotated", Size::Mm(210.0, 148.0)),
    (79, "B4 Rotated", Size::Mm(364.0, 257.0)),
    (80, "B5 Rotated", Size::Mm(257.0, 182.0)),
    (81, "Japanese Postcard Rotated", Size::Mm(148.0, 100.0)),
    (82, "Japanese Double Postcard Rotated", Size::Mm(148.0, 200.0)),
    (83, "A6 Rotated", Size::Mm(148.0, 105.0)),
    (84, "Japanese Envelope Kaku #2 Rotated", Size::Mm(332.0, 240.0)),
    (85, "Japanese Envelope Kaku #3 Rotated", Size::Mm(277.0, 216.0)),
    (86, "Japanese Envelope Chou #3 Rotated", Size::Mm(235.0, 120.0)),
    (87, "Japanese Envelope Chou #4 Rotated", Size::Mm(205.0, 90.0)),
    (88, "B6", Size::Mm(128.0, 182.0)),
    (89, "B6 Rotated", Size::Mm(182.0, 128.0)),
    (90, "12x11", Size::In(12.0, 11.0)),
    (91, "Japanese Envelope You #4", Size::Mm(105.0, 235.0)),
    (92, "Japanese Envelope You #4 Rotated", Size::Mm(235.0, 105.0)),
    (93, "PRC 16K", Size::Mm(146.0, 215.0)),
    (94, "PRC 32K", Size::Mm(97.0, 151.0)),
    (95, "PRC 32K Big", Size::Mm(97.0, 151.0)),
    (96, "PRC Envelope #1", Size::Mm(102.0, 165.0)),
    (97, "PRC Envelope #2", Size::Mm(102.0, 176.0)),
    (98, "PRC Envelope #3", Size::Mm(125.0, 176.0)),
    (99, "PRC Envelope #4", Size::Mm(110.0, 208.0)),
    (100, "PRC Envelope #5", Size::Mm(110.0, 220.0)),
    (101, "PRC Envelope #6", Size::Mm(120.0, 230.0)),
    (102, "PRC Envelope #7", Size::Mm(160.0, 230.0)),
    (103, "PRC Envelope #8", Size::Mm(120.0, 309.0)),
    (104, "PRC Envelope #9", Size::Mm(229.0, 324.0)),
    (105, "PRC Envelope #10", Size::Mm(324.0, 458.0)),
    (106, "PRC 16K Rotated", Size::Mm(215.0, 146.0)),
    (107, "PRC 32K Rotated", Size::Mm(151.0, 97.0)),
    (108, "PRC 32K Big Rotated", Size::Mm(151.0, 97.0)),
    (109, "PRC Envelope #1 Rotated", Size::Mm(165.0, 102.0)),
    (110, "PRC Envelope #2 Rotated", Size::Mm(176.0, 102.0)),
    (111, "PRC Envelope #3 Rotated", Size::Mm(176.0, 125.0)),
    (112, "PRC Envelope #4 Rotated", Size::Mm(208.0, 110.0)),
    (113, "PRC Envelope #5 Rotated", Size::Mm(220.0, 110.0)),
    (114, "PRC Envelope #6 Rotated", Size::Mm(230.0, 120.0)),
    (115, "PRC Envelope #7 Rotated", Size::Mm(230.0, 160.0)),
    (116, "PRC Envelope #8 Rotated", Size::Mm(309.0, 120.0)),
    (117, "PRC Envelope #9 Rotated", Size::Mm(324.0, 229.0)),
    (118, "PRC Envelope #10 Rotated", Size::Mm(458.0, 324.0)),
];

/// Letter, the paper a sheet without a `paperSize` prints on (ECMA-376 default `1`).
pub const DEFAULT_PAPER: &str = "Letter";

/// The names Page Setup offers, most common first.
pub const COMMON_PAPERS: [&str; 10] = ["Letter", "Legal", "A3", "A4", "A5", "B4", "B5", "Tabloid", "Executive", "Statement"];

/// Case-, space- and punctuation-insensitive key for a paper name.
fn key(name: &str) -> String {
    name.chars().filter(|c| !c.is_whitespace() && !matches!(c, '-' | '_' | '(' | ')')).flat_map(char::to_lowercase).collect()
}

/// A few names other software uses for sizes in the table.
fn alias(k: &str) -> &str {
    match k {
        "usletter" => "letter",
        "uslegal" => "legal",
        "jisb4" | "b4jis" => "b4",
        "jisb5" | "b5jis" => "b5",
        "jisb6" | "b6jis" => "b6",
        "superaa4" => "supera",
        "superba3" => "superb",
        _ => k,
    }
}

/// The name of a `paperSize` id, or `Paper <id>` for an id outside the standard table (kept
/// so the id survives a round trip).
pub fn paper_name(id: u32) -> String {
    PAPERS.iter().find(|(i, _, _)| *i == id).map(|(_, n, _)| (*n).to_string()).unwrap_or_else(|| format!("Paper {id}"))
}

/// The `paperSize` id of a paper name (the inverse of [`paper_name`]); `None` for an unknown name.
pub fn paper_id(name: &str) -> Option<u32> {
    let k = key(name);
    let k = alias(&k);
    if let Some(n) = k.strip_prefix("paper")
        && let Ok(id) = n.parse::<u32>()
    {
        return Some(id);
    }
    PAPERS.iter().find(|(_, n, _)| key(n) == k).map(|(i, _, _)| *i)
}

/// Portrait size in points of a paper name; `None` when the name (or `Paper <id>`) has no
/// known dimensions.
pub fn paper_size_pt(name: &str) -> Option<(f32, f32)> {
    let id = paper_id(name)?;
    PAPERS.iter().find(|(i, _, _)| *i == id).map(|(_, _, s)| s.points())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_and_names_are_unique_and_symmetric() {
        let mut ids = std::collections::BTreeSet::new();
        let mut names = std::collections::BTreeSet::new();
        for (id, name, size) in PAPERS {
            assert!(ids.insert(*id), "duplicate id {id}");
            assert!(names.insert(key(name)), "duplicate name {name}");
            assert_eq!(paper_id(name), Some(*id), "{name}");
            assert_eq!(paper_name(*id), *name);
            let (w, h) = size.points();
            assert!(w > 100.0 && h > 100.0 && w < 4000.0 && h < 4000.0, "{name}: {w}×{h}");
            assert_eq!(paper_size_pt(name), Some((w, h)));
        }
        assert_eq!(PAPERS.len(), 116, "ids 1–118 minus the two reserved");
        assert!(!ids.contains(&48) && !ids.contains(&49));
    }

    #[test]
    fn ecma_sizes() {
        let close = |a: (f32, f32), b: (f32, f32)| (a.0 - b.0).abs() < 0.02 && (a.1 - b.1).abs() < 0.02;
        assert!(close(paper_size_pt("Letter").unwrap(), (612.0, 792.0)));
        assert!(close(paper_size_pt("A4").unwrap(), (595.276, 841.890)));
        assert!(close(paper_size_pt("A3").unwrap(), (841.890, 1190.551)));
        assert!(close(paper_size_pt("A5").unwrap(), (419.528, 595.276)));
        assert!(close(paper_size_pt("Statement").unwrap(), (396.0, 612.0)));
        assert!(close(paper_size_pt("A6").unwrap(), (297.638, 419.528)));
        assert!(close(paper_size_pt("Envelope DL").unwrap(), (311.811, 623.622)));
        assert_eq!(paper_id("Letter"), Some(1));
        assert_eq!(paper_id("A4"), Some(9));
        assert_eq!(paper_id("A3"), Some(8));
        assert_eq!(paper_id("A5"), Some(11));
        assert_eq!(paper_id("Statement"), Some(6));
        assert_eq!(paper_id("A6"), Some(70));
        assert_eq!(paper_id("Folio"), Some(14));
        assert_eq!(paper_id("Envelope C5"), Some(28));
        assert_eq!(paper_id("A2"), Some(66));
    }

    #[test]
    fn names_are_forgiving_and_unknown_ids_round_trip() {
        assert_eq!(paper_id(" a4 "), Some(9));
        assert_eq!(paper_id("US Letter"), Some(1));
        assert_eq!(paper_id("envelope-#10"), Some(20));
        assert_eq!(paper_id("B5 (JIS)"), Some(13));
        assert_eq!(paper_id("Napkin"), None);
        assert_eq!(paper_size_pt("Napkin"), None);
        assert_eq!(paper_name(200), "Paper 200");
        assert_eq!(paper_id("Paper 200"), Some(200));
        assert_eq!(paper_size_pt("Paper 200"), None);
        for n in COMMON_PAPERS {
            assert!(paper_size_pt(n).is_some(), "{n}");
        }
    }
}
