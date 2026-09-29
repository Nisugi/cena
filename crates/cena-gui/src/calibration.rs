//! Where each part sits on a doll picture, and how its dots look: the
//! picture's own calibration (`plan/55` §2b).
//!
//! **Kept in the picture** (the author: *"hydra should write in the picture,
//! not make a sidecar toml"*), as `VellumFE` keeps it: TOML in a PNG `tEXt`
//! chunk under the keyword `vellum-meta`
//! (`reference/VellumFE/src/config/png_meta.rs`), so a doll travels as one
//! file and either client reads the other's. A picture with no chunk may
//! have `VellumFE`'s working copy beside it, `<picture>.toml`, which is read
//! in its place.
//!
//! ```toml
//! kind = "doll"
//! [anchors]
//! head = [0.5, 0.09]
//! leftArm = [0.31, 0.36]
//! [dots]
//! opacity = 0.9
//! diameter = 0.07
//! ```
//!
//! An anchor is `[x, y]` in fractions of the drawn picture, so it holds at
//! any size. `VellumFE` writes the keys in lower case (`leftarm`); they are
//! read without regard to case and written in the game's spelling. A part
//! with no anchor sits at the doll's default place.

use std::collections::BTreeMap;
use std::path::Path;

use crate::widget::doll as doll_parts;

/// The keyword of the PNG text chunk that holds it.
pub(crate) const KEYWORD: &str = "vellum-meta";

const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

/// A doll picture's calibration.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Calibration {
    /// Each calibrated part's place, by the game's id.
    pub(crate) anchors: BTreeMap<&'static str, (f32, f32)>,
    /// How opaque a dot is, 0 to 1.
    pub(crate) opacity: f32,
    /// A dot's width, as a fraction of the picture's height.
    pub(crate) diameter: f32,
}

impl Default for Calibration {
    /// `VellumFE`'s: no part calibrated, dots at 0.9 and 0.07.
    fn default() -> Self {
        Self {
            anchors: BTreeMap::new(),
            opacity: 0.9,
            diameter: 0.07,
        }
    }
}

impl Calibration {
    /// Where `part` sits: calibrated, or its default place.
    pub(crate) fn anchor(&self, part: &doll_parts::Part) -> (f32, f32) {
        self.anchors.get(part.id).copied().unwrap_or(part.anchor)
    }

    /// Read from TOML. What it does not know it leaves at the default: a
    /// part it has no id for, a value out of range.
    pub(crate) fn of_toml(text: &str) -> Self {
        let mut read = Self::default();
        let Ok(table) = text.parse::<toml::Table>() else {
            return read;
        };
        if let Some(anchors) = table.get("anchors").and_then(toml::Value::as_table) {
            for (key, value) in anchors {
                let Some(part) = doll_parts::PARTS
                    .iter()
                    .find(|part| part.id.eq_ignore_ascii_case(key))
                else {
                    continue;
                };
                let pair = value
                    .as_array()
                    .map(|pair| pair.iter().filter_map(number).collect::<Vec<_>>());
                if let Some([x, y]) = pair.as_deref()
                    && (0.0..=1.0).contains(x)
                    && (0.0..=1.0).contains(y)
                {
                    #[expect(
                        clippy::cast_possible_truncation,
                        reason = "a fraction of a picture, checked to lie in 0..=1"
                    )]
                    read.anchors.insert(part.id, (*x as f32, *y as f32));
                }
            }
        }
        if let Some(dots) = table.get("dots").and_then(toml::Value::as_table) {
            let number = |key: &str| dots.get(key).and_then(number);
            #[expect(
                clippy::cast_possible_truncation,
                reason = "clamped to a small range before narrowing"
            )]
            if let Some(opacity) = number("opacity") {
                read.opacity = opacity.clamp(0.0, 1.0) as f32;
            }
            #[expect(
                clippy::cast_possible_truncation,
                reason = "clamped to a small range before narrowing"
            )]
            if let Some(diameter) = number("diameter") {
                read.diameter = diameter.clamp(0.01, 0.5) as f32;
            }
        }
        read
    }

    /// The calibration of the picture at `path`: its own chunk, or the
    /// `.toml` beside it, or none.
    pub(crate) fn of_picture(path: &Path) -> Self {
        let text = std::fs::read(path)
            .ok()
            .and_then(|bytes| embedded(&bytes))
            .or_else(|| std::fs::read_to_string(path.with_extension("toml")).ok());
        text.map_or_else(Self::default, |text| Self::of_toml(&text))
    }
}

/// Write `calibration` into the picture at `path`, **in the picture**: its
/// `vellum-meta` chunk replaced, or put after the header, and nothing else
/// of the file touched, the pixels byte for byte. What the picture's old
/// calibration held beyond anchors and dots, or a `VellumFE` `.toml`
/// beside it, is carried over, so a calibration made in `VellumFE` loses
/// nothing. Written as every file Hydra keeps is, to a temp file renamed
/// into place (`cena_session::store::save_bytes`).
///
/// # Errors
///
/// Why not: the picture cannot be read or written, or is not a PNG.
pub(crate) fn write(path: &Path, calibration: &Calibration) -> Result<(), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let old = embedded(&bytes)
        .or_else(|| std::fs::read_to_string(path.with_extension("toml")).ok())
        .and_then(|text| text.parse::<toml::Table>().ok())
        .unwrap_or_default();
    let text = toml::to_string(&calibration.over_table(old))
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let written =
        with_chunk(&bytes, &text).ok_or_else(|| format!("{} is not a PNG", path.display()))?;
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    cena_session::store::save_bytes(dir, path, &written)
        .map_err(|e| format!("{}: {e}", path.display()))
}

impl Calibration {
    /// `old` with this calibration's anchors and dots in it.
    fn over_table(&self, mut old: toml::Table) -> toml::Table {
        let round = |value: f32, places: f64| (f64::from(value) * places).round() / places;
        old.insert("kind".to_owned(), toml::Value::from("doll"));
        let mut anchors = toml::Table::new();
        for part in doll_parts::PARTS {
            if let Some(&(x, y)) = self.anchors.get(part.id) {
                let pair = vec![
                    toml::Value::from(round(x, 10_000.0)),
                    toml::Value::from(round(y, 10_000.0)),
                ];
                anchors.insert(part.id.to_owned(), toml::Value::Array(pair));
            }
        }
        old.insert("anchors".to_owned(), toml::Value::Table(anchors));
        let mut dots = old
            .remove("dots")
            .and_then(|dots| dots.as_table().cloned())
            .unwrap_or_default();
        dots.insert("opacity".to_owned(), round(self.opacity, 100.0).into());
        dots.insert("diameter".to_owned(), round(self.diameter, 1_000.0).into());
        old.insert("dots".to_owned(), toml::Value::Table(dots));
        old
    }
}

/// `bytes` with `text` as its calibration chunk: any old one dropped, the
/// new one after the header. `None` when the bytes are not a PNG.
pub(crate) fn with_chunk(bytes: &[u8], text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(bytes.len() + text.len() + 64);
    out.extend_from_slice(&PNG_SIGNATURE);
    let mut put = false;
    for (kind, data) in chunks(bytes)? {
        let ours = kind == *b"tEXt"
            && data
                .iter()
                .position(|&b| b == 0)
                .is_some_and(|nul| &data[..nul] == KEYWORD.as_bytes());
        if ours {
            continue;
        }
        push_chunk(&mut out, kind, data)?;
        if !put && kind == *b"IHDR" {
            let mut chunk = KEYWORD.as_bytes().to_vec();
            chunk.push(0);
            chunk.extend_from_slice(text.as_bytes());
            push_chunk(&mut out, *b"tEXt", &chunk)?;
            put = true;
        }
    }
    put.then_some(out)
}

/// One chunk onto `out`: its length, type, data and CRC.
fn push_chunk(out: &mut Vec<u8>, kind: [u8; 4], data: &[u8]) -> Option<()> {
    out.extend_from_slice(&u32::try_from(data.len()).ok()?.to_be_bytes());
    out.extend_from_slice(&kind);
    out.extend_from_slice(data);
    out.extend_from_slice(&crc32(&[&kind, data]).to_be_bytes());
    Some(())
}

/// PNG's CRC-32 over `parts` in order. Bit by bit: it runs once per save.
fn crc32(parts: &[&[u8]]) -> u32 {
    let mut crc = 0xffff_ffff_u32;
    for byte in parts.iter().flat_map(|part| part.iter()) {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

/// A TOML number, written with a point or without.
fn number(value: &toml::Value) -> Option<f64> {
    value.as_float().or_else(|| {
        value
            .as_integer()
            .and_then(|whole| i32::try_from(whole).ok())
            .map(f64::from)
    })
}

/// The calibration text a PNG carries, if it is a PNG and carries one.
pub(crate) fn embedded(bytes: &[u8]) -> Option<String> {
    chunks(bytes)?.into_iter().find_map(|(kind, data)| {
        if kind != *b"tEXt" {
            return None;
        }
        let nul = data.iter().position(|&b| b == 0)?;
        (&data[..nul] == KEYWORD.as_bytes())
            .then(|| String::from_utf8(data[nul + 1..].to_vec()).ok())
            .flatten()
    })
}

/// A PNG's chunks, as (type, data); `None` when the bytes are not one.
fn chunks(bytes: &[u8]) -> Option<Vec<([u8; 4], &[u8])>> {
    if bytes.get(..8)? != PNG_SIGNATURE {
        return None;
    }
    let mut out = Vec::new();
    let mut at = 8;
    while at + 12 <= bytes.len() {
        let len = usize::try_from(u32::from_be_bytes(bytes[at..at + 4].try_into().ok()?)).ok()?;
        let kind: [u8; 4] = bytes[at + 4..at + 8].try_into().ok()?;
        let end = at.checked_add(8)?.checked_add(len)?;
        if end + 4 > bytes.len() {
            return None;
        }
        out.push((kind, &bytes[at + 8..end]));
        at = end + 4;
        if kind == *b"IEND" {
            break;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::{Calibration, embedded};

    #[test]
    fn vellums_lower_case_keys_are_read_and_the_rest_defaults() {
        let read = Calibration::of_toml(
            "kind = \"doll\"\n[anchors]\nleftarm = [0.7763, 0.2857]\nwing = [0.1, 0.1]\n\
             head = [2.0, 0.5]\n[dots]\nopacity = 0.5\ndiameter = 9.0\n",
        );
        assert_eq!(read.anchors.get("leftArm"), Some(&(0.7763, 0.2857)));
        assert_eq!(
            read.anchors.len(),
            1,
            "no wing, and no head off the picture"
        );
        assert!((read.opacity - 0.5).abs() < f32::EPSILON);
        assert!((read.diameter - 0.5).abs() < f32::EPSILON, "clamped");
    }

    /// A PNG with `VellumFE`'s chunk after its header, as `VellumFE`
    /// writes it: read, and the `.toml` beside it passed over.
    #[test]
    fn the_chunk_in_the_picture_is_read_first() {
        let mut png = Vec::new();
        image::RgbaImage::new(1, 1)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let meta = "[anchors]
head = [0.25, 0.75]
";
        let mut data = super::KEYWORD.as_bytes().to_vec();
        data.push(0);
        data.extend_from_slice(meta.as_bytes());
        let mut chunk = u32::try_from(data.len()).unwrap().to_be_bytes().to_vec();
        chunk.extend_from_slice(b"tEXt");
        chunk.extend_from_slice(&data);
        chunk.extend_from_slice(&[0; 4]);
        // After the signature (8) and the header chunk (4 + 4 + 13 + 4).
        png.splice(33..33, chunk);
        assert_eq!(embedded(&png).as_deref(), Some(meta));

        let dir = std::env::temp_dir().join(format!("cena-calibration-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("doll.png");
        std::fs::write(&path, &png).unwrap();
        std::fs::write(
            dir.join("doll.toml"),
            "[anchors]
head = [0.9, 0.9]
",
        )
        .unwrap();
        let read = Calibration::of_picture(&path);
        assert_eq!(
            read.anchors.get("head"),
            Some(&(0.25, 0.75)),
            "the picture's own"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Written into the picture: read back the same, the old chunk replaced
    /// and never doubled, the pixels and every other chunk as they were,
    /// what `VellumFE` kept beside the picture carried in, and the image
    /// still an image.
    #[test]
    fn a_calibration_is_written_into_the_picture_and_nothing_else_changes() {
        let dir =
            std::env::temp_dir().join(format!("cena-calibration-write-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("doll.png");
        image::RgbaImage::from_pixel(3, 2, image::Rgba([10, 20, 30, 255]))
            .save(&path)
            .unwrap();
        std::fs::write(
            dir.join("doll.toml"),
            "priority = 4\n[dots]\nwound_color = \"#e02020\"\n",
        )
        .unwrap();
        let before = std::fs::read(&path).unwrap();

        let mut calibration = Calibration::default();
        calibration.anchors.insert("leftArm", (0.25, 0.5));
        calibration.opacity = 0.6;
        super::write(&path, &calibration).unwrap();
        calibration.anchors.insert("head", (0.5, 0.125));
        super::write(&path, &calibration).unwrap();

        let after = std::fs::read(&path).unwrap();
        assert_eq!(Calibration::of_picture(&path), calibration);
        let text = embedded(&after).unwrap();
        assert!(text.contains("leftArm"), "the game's spelling: {text}");
        assert!(
            text.contains("priority = 4") && text.contains("wound_color"),
            "{text}"
        );
        let ours = |bytes: &[u8]| {
            super::chunks(bytes)
                .unwrap()
                .iter()
                .filter(|(kind, data)| {
                    kind == b"tEXt" && data.starts_with(super::KEYWORD.as_bytes())
                })
                .count()
        };
        assert_eq!(ours(&after), 1, "replaced, not doubled");
        let others = |bytes: &[u8]| -> Vec<([u8; 4], Vec<u8>)> {
            super::chunks(bytes)
                .unwrap()
                .into_iter()
                .filter(|(kind, _)| kind != b"tEXt")
                .map(|(kind, data)| (kind, data.to_vec()))
                .collect()
        };
        assert_eq!(
            others(&after),
            others(&before),
            "every other chunk as it was"
        );
        assert!(image::load_from_memory(&after).is_ok(), "still a picture");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn crc_matches_the_standard_check_value() {
        assert_eq!(super::crc32(&[b"123456789"]), 0xcbf4_3926);
    }

    #[test]
    fn a_picture_without_the_chunk_has_none() {
        let mut png = Vec::new();
        image::RgbaImage::new(1, 1)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        assert_eq!(embedded(&png), None);
        assert_eq!(embedded(b"not a png"), None);
    }
}
