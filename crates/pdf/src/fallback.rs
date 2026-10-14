//! The embedded fallback font: a system TrueType/OpenType face (맑은 고딕, Nanum Gothic, Noto
//! Sans CJK…) subset into the PDF as a CID font for text the standard 14 fonts can't encode
//! (Korean, Japanese, Chinese, Cyrillic, Greek…). Without one such text prints as `?`.
//!
//! The face is found once per process ([`system_fallback`]): `GRIDCRAFT_PDF_FALLBACK_FONT`
//! names a file (`path` or `path#index` for a collection; empty turns the fallback off),
//! otherwise the user's and the system's font directories are scanned for the best-known
//! CJK-capable face.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use subsetter::GlyphRemapper;

/// A parsed face: the bytes to subset from and the metrics a PDF needs.
pub struct FallbackFont {
    data: Vec<u8>,
    index: u32,
    name: String,
    cff: bool,
    units_per_em: f32,
    glyphs: HashMap<char, u16>,
    advances: Vec<u16>,
    ascent: i16,
    descent: i16,
    cap_height: i16,
    bbox: [i16; 4],
}

impl FallbackFont {
    /// Parses face `index` of a font file (0 unless the file is a collection).
    pub fn from_bytes(data: Vec<u8>, index: u32) -> Option<FallbackFont> {
        let face = ttf_parser::Face::parse(&data, index).ok()?;
        let units_per_em = f32::from(face.units_per_em().max(1));
        let cff = face.tables().cff.is_some();
        let mut glyphs = HashMap::new();
        if let Some(cmap) = face.tables().cmap {
            for st in cmap.subtables {
                if !st.is_unicode() {
                    continue;
                }
                st.codepoints(|cp| {
                    if let Some(c) = char::from_u32(cp)
                        && let Some(g) = st.glyph_index(cp)
                        && g.0 != 0
                    {
                        glyphs.entry(c).or_insert(g.0);
                    }
                });
            }
        }
        if glyphs.is_empty() {
            return None;
        }
        let n = usize::from(face.number_of_glyphs());
        let advances = (0..n).map(|g| face.glyph_hor_advance(ttf_parser::GlyphId(g as u16)).unwrap_or(0)).collect();
        let name = face
            .names()
            .into_iter()
            .filter(|n| n.name_id == ttf_parser::name_id::POST_SCRIPT_NAME || n.name_id == ttf_parser::name_id::FAMILY)
            .filter_map(|n| n.to_string())
            .find(|s| s.is_ascii() && !s.is_empty())
            .unwrap_or_else(|| "Fallback".into());
        let name: String = name.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
        let name = if name.is_empty() { "Fallback".to_string() } else { name };
        let b = face.global_bounding_box();
        Some(FallbackFont {
            name,
            cff,
            units_per_em,
            glyphs,
            advances,
            ascent: face.ascender(),
            descent: face.descender(),
            cap_height: face.capital_height().unwrap_or((face.ascender() as f32 * 0.7) as i16),
            bbox: [b.x_min, b.y_min, b.x_max, b.y_max],
            data,
            index,
        })
    }

    /// Loads a font file; for a collection, the face whose family name mentions `KR` (Korean),
    /// else the first.
    pub fn load(path: &Path) -> Option<FallbackFont> {
        let data = std::fs::read(path).ok()?;
        let index = korean_face(&data);
        FallbackFont::from_bytes(data, index)
    }

    /// The glyph for a character, when the face has one.
    pub fn glyph(&self, c: char) -> Option<u16> {
        self.glyphs.get(&c).copied()
    }

    /// Advance width of a glyph in 1/1000 em.
    pub fn advance_1000(&self, gid: u16) -> u16 {
        let adv = self.advances.get(usize::from(gid)).copied().unwrap_or(0);
        (f32::from(adv) * 1000.0 / self.units_per_em).round().clamp(0.0, 65535.0) as u16
    }

    /// The face's PostScript or family name (ASCII only).
    pub fn name(&self) -> &str {
        &self.name
    }

    fn em1000(&self, v: i16) -> i32 {
        (f32::from(v) * 1000.0 / self.units_per_em).round() as i32
    }
}

/// The index of the Korean face in a collection (Noto/Source Han CJK ship JP, KR, SC, TC…),
/// else 0.
fn korean_face(data: &[u8]) -> u32 {
    let Some(n) = ttf_parser::fonts_in_collection(data) else { return 0 };
    (0..n.min(16))
        .find(|&i| {
            ttf_parser::Face::parse(data, i).ok().is_some_and(|f| {
                f.names()
                    .into_iter()
                    .filter(|n| n.name_id == ttf_parser::name_id::FAMILY)
                    .filter_map(|n| n.to_string())
                    .any(|s| s.contains("KR") || s.contains("Korean"))
            })
        })
        .unwrap_or(0)
}

/// Known CJK-capable faces, best first (file name, lower case).
const CANDIDATES: &[&str] = &[
    "malgun.ttf",
    "nanumgothic.ttf",
    "nanumbarungothic.ttf",
    "notosanscjk-regular.ttc",
    "notosanscjkkr-regular.otf",
    "notosanskr-regular.ttf",
    "notosanskr-regular.otf",
    "notosanscjk-vf.otf.ttc",
    "sourcehansans-regular.ttc",
    "sourcehansanskr-regular.otf",
    "applesdgothicneo.ttc",
    "pingfang.ttc",
    "msgothic.ttc",
    "meiryo.ttc",
    "droidsansfallbackfull.ttf",
    "droidsansfallback.ttf",
    "wqy-microhei.ttc",
    "wqy-zenhei.ttc",
    "unifont.otf",
    "unifont.ttf",
];

fn font_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        dirs.push(home.join(".local/share/fonts"));
        dirs.push(home.join(".fonts"));
        dirs.push(home.join("Library/Fonts"));
    }
    for d in ["/usr/share/fonts", "/usr/local/share/fonts", "/Library/Fonts", "/System/Library/Fonts"] {
        dirs.push(PathBuf::from(d));
    }
    if let Some(win) = std::env::var_os("WINDIR").map(PathBuf::from) {
        dirs.push(win.join("Fonts"));
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        dirs.push(local.join("Microsoft/Windows/Fonts"));
    }
    dirs
}

/// Walks `dir` (a few levels, bounded) noting the best-ranked candidate file names.
fn scan(dir: &Path, depth: u32, budget: &mut u32, found: &mut BTreeMap<usize, PathBuf>) {
    if depth > 5 || *budget == 0 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        if *budget == 0 {
            return;
        }
        *budget -= 1;
        let p = e.path();
        if p.is_dir() {
            scan(&p, depth + 1, budget, found);
        } else if let Some(name) = p.file_name().and_then(|n| n.to_str())
            && let Some(rank) = CANDIDATES.iter().position(|c| c.eq_ignore_ascii_case(name))
        {
            found.entry(rank).or_insert(p);
        }
    }
}

fn discover() -> Option<FallbackFont> {
    if let Some(spec) = std::env::var_os("GRIDCRAFT_PDF_FALLBACK_FONT") {
        let spec = spec.to_string_lossy();
        let spec = spec.trim();
        if spec.is_empty() {
            return None;
        }
        return match spec.rsplit_once('#').and_then(|(p, i)| i.parse::<u32>().ok().map(|i| (p, i))) {
            Some((path, index)) => std::fs::read(path).ok().and_then(|d| FallbackFont::from_bytes(d, index)),
            None => FallbackFont::load(Path::new(spec)),
        };
    }
    let mut found = BTreeMap::new();
    let mut budget = 20_000u32;
    for d in font_dirs() {
        scan(&d, 0, &mut budget, &mut found);
    }
    found.values().find_map(|p| FallbackFont::load(p))
}

/// The process-wide fallback face, found on first use. `None` when no CJK-capable font is
/// installed (text outside WinAnsi then prints as `?`).
pub fn system_fallback() -> Option<Arc<FallbackFont>> {
    static FONT: OnceLock<Option<Arc<FallbackFont>>> = OnceLock::new();
    FONT.get_or_init(|| discover().map(Arc::new)).clone()
}

/// The glyphs a document uses from its fallback face, numbered as the embedded subset
/// numbers them (1… in first-use order; 0 is `.notdef`).
#[derive(Default)]
pub(crate) struct Subset {
    remapper: GlyphRemapper,
    /// Subset glyph id → (advance in 1/1000 em, the character it shows).
    entries: BTreeMap<u16, (u16, char)>,
}

impl Subset {
    pub(crate) fn add(&mut self, font: &FallbackFont, c: char, gid: u16) -> u16 {
        let new = self.remapper.remap(gid);
        self.entries.entry(new).or_insert((font.advance_1000(gid), c));
        new
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// The objects embedding the subset: `(font dict, CID font dict, descriptor, font file stream,
/// ToUnicode stream)`, each referring to the ids `first..first+5`.
pub(crate) fn font_objects(font: &FallbackFont, subset: &Subset, first: u32) -> Vec<(u32, Vec<u8>)> {
    let (type0, cid, desc, file, tounicode) = (first, first + 1, first + 2, first + 3, first + 4);
    let tag = subset_tag(subset);
    let base = format!("{tag}+{}", font.name());
    let program = subsetter::subset(&font.data, font.index, &subset.remapper).ok();
    let widths: Vec<String> = subset.entries.values().map(|(w, _)| w.to_string()).collect();
    let w_array = match subset.entries.keys().next() {
        Some(firstgid) => format!("{firstgid} [{}]", widths.join(" ")),
        None => String::new(),
    };
    let (subtype, file_key, cid_map) =
        if font.cff { ("CIDFontType0", "FontFile3", "") } else { ("CIDFontType2", "FontFile2", " /CIDToGIDMap /Identity") };
    let mut out = Vec::new();
    out.push((
        type0,
        format!("<< /Type /Font /Subtype /Type0 /BaseFont /{base} /Encoding /Identity-H /DescendantFonts [{cid} 0 R] /ToUnicode {tounicode} 0 R >>")
            .into_bytes(),
    ));
    out.push((
        cid,
        format!(
            "<< /Type /Font /Subtype /{subtype} /BaseFont /{base} /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> /FontDescriptor {desc} 0 R /DW 1000 /W [{w_array}]{cid_map} >>"
        )
        .into_bytes(),
    ));
    let file_ref = if program.is_some() { format!(" /{file_key} {file} 0 R") } else { String::new() };
    out.push((
        desc,
        format!(
            "<< /Type /FontDescriptor /FontName /{base} /Flags 4 /FontBBox [{} {} {} {}] /ItalicAngle 0 /Ascent {} /Descent {} /CapHeight {} /StemV 80{file_ref} >>",
            font.em1000(font.bbox[0]),
            font.em1000(font.bbox[1]),
            font.em1000(font.bbox[2]),
            font.em1000(font.bbox[3]),
            font.em1000(font.ascent),
            font.em1000(font.descent),
            font.em1000(font.cap_height),
        )
        .into_bytes(),
    ));
    if let Some(bytes) = program {
        let extra = if font.cff { " /Subtype /OpenType" } else { "" };
        let mut o = format!("<< /Length {}{extra} >>\nstream\n", bytes.len()).into_bytes();
        o.extend_from_slice(&bytes);
        o.extend_from_slice(b"\nendstream");
        out.push((file, o));
    } else {
        out.push((file, b"null".to_vec()));
    }
    let cmap = to_unicode(subset);
    let mut o = format!("<< /Length {} >>\nstream\n", cmap.len()).into_bytes();
    o.extend_from_slice(cmap.as_bytes());
    o.extend_from_slice(b"\nendstream");
    out.push((tounicode, o));
    out
}

/// Six capitals naming the subset (ISO 32000-1 9.6.4), derived from its glyphs.
fn subset_tag(subset: &Subset) -> String {
    let mut h: u32 = 0x811c_9dc5;
    for (g, (_, c)) in &subset.entries {
        for b in g.to_le_bytes().into_iter().chain((*c as u32).to_le_bytes()) {
            h = (h ^ u32::from(b)).wrapping_mul(0x0100_0193);
        }
    }
    (0..6)
        .map(|i| {
            let v = (h >> (i * 5)) & 31;
            char::from(b'A' + (v % 26) as u8)
        })
        .collect()
}

/// A ToUnicode CMap so text extraction and search see the characters, not glyph numbers.
fn to_unicode(subset: &Subset) -> String {
    use std::fmt::Write as _;
    let mut s = String::from(
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
    );
    let entries: Vec<(&u16, &(u16, char))> = subset.entries.iter().collect();
    for chunk in entries.chunks(100) {
        let _ = writeln!(s, "{} beginbfchar", chunk.len());
        for (gid, (_, c)) in chunk {
            let mut units = [0u16; 2];
            let enc = c.encode_utf16(&mut units);
            let hex: String = enc.iter().map(|u| format!("{u:04X}")).collect();
            let _ = writeln!(s, "<{gid:04X}> <{hex}>");
        }
        s.push_str("endbfchar\n");
    }
    s.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    s
}

/// A page's share of the document's subset.
pub(crate) type SharedSubset = Arc<Mutex<Subset>>;

pub(crate) fn lock(s: &SharedSubset) -> std::sync::MutexGuard<'_, Subset> {
    s.lock().unwrap_or_else(|e| e.into_inner())
}
