//! A workspace's brand fonts: a heading and a body family, each with a regular and a bold file,
//! that animation text is set in. Files are checked once at import: they must be static TrueType
//! or OpenType fonts that draw accented Latin letters and common symbols, and their average letter
//! width is measured so text wraps to fit.

use std::{
    error::Error,
    fs, io,
    io::Read,
    path::{Path, PathBuf},
};

use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::WorkspaceStore;

const MAX_FONT_BYTES: u64 = 20 * 1024 * 1024;
/// Letters and symbols every brand font must draw, so videos in English, Spanish and Portuguese
/// never show missing-glyph boxes.
const REQUIRED: &str = "AaÁáÀàÂâÃãÇçÉéÊêÍíÓóÔôÕõÚúÜüÑñ¿¡€$%&@#0123456789.,;:!?'\"()-+/";
/// Text whose average advance stands for a typical line.
const SAMPLE: &str = "The quick brown fox jumps over the lazy dog, 2026.";
/// The average advance assumed for the system font, as a fraction of the font size.
pub(crate) const SYSTEM_ADVANCE: f64 = 0.52;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FontRole {
    Heading,
    #[default]
    Body,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FontWeight {
    Regular,
    Bold,
}

impl FontRole {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Heading => "heading",
            Self::Body => "body",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "heading" => Some(Self::Heading),
            "body" => Some(Self::Body),
            _ => None,
        }
    }
}

impl FontWeight {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Regular => "regular",
            Self::Bold => "bold",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "regular" => Some(Self::Regular),
            "bold" => Some(Self::Bold),
            _ => None,
        }
    }
}

/// One saved font file and what Arrumo measured about it.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct BrandFont {
    pub role: FontRole,
    pub weight: FontWeight,
    pub family: String,
    pub file_name: String,
    pub sha256: String,
    pub byte_size: i64,
    /// Average letter width as a fraction of the font size.
    pub advance: f64,
}

/// The family names a brief tells the agent, by role, from saved or snapshotted fonts; `null`
/// for a role without one, which draws in a plain sans-serif.
pub(crate) fn families(fonts: &Value) -> Value {
    let fonts: Vec<BrandFont> = serde_json::from_value(fonts.clone()).unwrap_or_default();
    let family = |role: FontRole| {
        fonts
            .iter()
            .find(|font| font.role == role)
            .map(|font| font.family.clone())
    };
    serde_json::json!({
        "heading": family(FontRole::Heading).or_else(|| family(FontRole::Body)),
        "body": family(FontRole::Body),
    })
}

/// Where a font file is kept, named by its digest.
pub(crate) fn font_file(media_root: &Path, workspace_id: i64, sha256: &str) -> PathBuf {
    media_root
        .join(workspace_id.to_string())
        .join("fonts")
        .join(sha256)
}

/// The family name and average advance of a usable font, or why it cannot be used.
/// A font file problem the creator can fix: a stable code and a message fit to show them.
#[derive(Debug)]
pub struct UnusableFont {
    pub code: &'static str,
    pub message: &'static str,
}

const fn unusable(code: &'static str, message: &'static str) -> UnusableFont {
    UnusableFont { code, message }
}

impl std::fmt::Display for UnusableFont {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message)
    }
}

impl Error for UnusableFont {}

fn inspect(bytes: &[u8]) -> Result<(String, f64), UnusableFont> {
    let known = [&[0, 1, 0, 0][..], b"OTTO", b"true"];
    if !known.iter().any(|magic| bytes.starts_with(magic)) {
        return Err(unusable(
            "font_not_font_file",
            "Choose a TrueType (.ttf) or OpenType (.otf) font file",
        ));
    }
    let face = ttf_parser::Face::parse(bytes, 0)
        .map_err(|_| unusable("font_damaged", "The font file is damaged or not a font"))?;
    if face.is_variable() {
        return Err(unusable(
            "font_variable",
            "Choose a static font file, such as the Regular or Bold one, not a variable font",
        ));
    }
    if REQUIRED.chars().any(|c| face.glyph_index(c).is_none()) {
        return Err(unusable(
            "font_missing_characters",
            "The font is missing accented letters or symbols videos need, such as á, ç, ñ, ¿ or €",
        ));
    }
    let name = |id: u16| {
        face.names()
            .into_iter()
            .filter(|name| name.name_id == id)
            .find_map(|name| name.to_string())
            .map(|name| name.trim().to_owned())
            .filter(|name| !name.is_empty())
    };
    let family = name(ttf_parser::name_id::TYPOGRAPHIC_FAMILY)
        .or_else(|| name(ttf_parser::name_id::FAMILY))
        .unwrap_or_else(|| "Font".into());
    let family: String = family
        .chars()
        .filter(|c| !c.is_control())
        .take(120)
        .collect();
    let units = f64::from(face.units_per_em().max(1));
    let widths: Vec<f64> = SAMPLE
        .chars()
        .filter_map(|c| face.glyph_hor_advance(face.glyph_index(c)?))
        .map(|advance| f64::from(advance) / units)
        .collect();
    let advance = widths.iter().sum::<f64>() / widths.len().max(1) as f64;
    if !(0.2..1.5).contains(&advance) {
        return Err(unusable(
            "font_unusual_width",
            "The font's letters have an unusual width Arrumo cannot lay out",
        ));
    }
    Ok((family, (advance * 1000.0).round() / 1000.0))
}

fn read_font(row: &rusqlite::Row<'_>) -> rusqlite::Result<BrandFont> {
    let text = |index: usize| row.get::<_, String>(index);
    let invalid = || rusqlite::Error::InvalidQuery;
    Ok(BrandFont {
        role: FontRole::parse(&text(0)?).ok_or_else(invalid)?,
        weight: FontWeight::parse(&text(1)?).ok_or_else(invalid)?,
        family: text(2)?,
        file_name: text(3)?,
        sha256: text(4)?,
        byte_size: row.get(5)?,
        advance: row.get(6)?,
    })
}

impl WorkspaceStore {
    pub fn brand_fonts(&self, workspace_id: i64) -> rusqlite::Result<Vec<BrandFont>> {
        let mut statement = self.0.prepare(
            "SELECT role, weight, family, file_name, sha256, byte_size, advance FROM brand_fonts
             WHERE workspace_id = ?1 ORDER BY role = 'body', weight = 'bold'",
        )?;
        statement.query_map([workspace_id], read_font)?.collect()
    }

    /// Checks a font file and saves it as the workspace's `role` font in `weight`, replacing
    /// the one there. Runs that already used the old file keep it.
    pub fn import_brand_font(
        &self,
        media_root: &Path,
        workspace_id: i64,
        role: FontRole,
        weight: FontWeight,
        source: &Path,
    ) -> Result<BrandFont, Box<dyn Error>> {
        let exists: Option<i64> = self
            .0
            .query_row(
                "SELECT id FROM workspaces WHERE id = ?1",
                [workspace_id],
                |row| row.get(0),
            )
            .optional()?;
        exists.ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Workspace not found"))?;
        let file_name: String = source
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("font")
            .chars()
            .filter(|c| !c.is_control())
            .take(255)
            .collect();
        let mut bytes = Vec::new();
        fs::File::open(source)?
            .take(MAX_FONT_BYTES + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_FONT_BYTES {
            return Err(unusable("font_too_large", "Font files can be at most 20 MB").into());
        }
        let (family, advance) = inspect(&bytes)?;
        let sha256: String = Sha256::digest(&bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        let destination = font_file(media_root, workspace_id, &sha256);
        if !destination.is_file() {
            fs::create_dir_all(
                destination
                    .parent()
                    .ok_or_else(|| io::Error::other("Bad path"))?,
            )?;
            let partial = destination.with_extension("partial");
            fs::write(&partial, &bytes)?;
            fs::rename(&partial, &destination)?;
        }
        let font = BrandFont {
            role,
            weight,
            family,
            file_name,
            sha256,
            byte_size: bytes.len() as i64,
            advance,
        };
        self.0.execute(
            "INSERT INTO brand_fonts (workspace_id, role, weight, family, file_name, sha256, byte_size, advance)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT (workspace_id, role, weight) DO UPDATE SET family = excluded.family,
                 file_name = excluded.file_name, sha256 = excluded.sha256,
                 byte_size = excluded.byte_size, advance = excluded.advance",
            params![
                workspace_id,
                role.as_str(),
                weight.as_str(),
                font.family,
                font.file_name,
                font.sha256,
                font.byte_size,
                font.advance
            ],
        )?;
        Ok(font)
    }

    /// Forgets the workspace's `role` font in `weight`. Returns whether there was one.
    pub fn remove_brand_font(
        &self,
        workspace_id: i64,
        role: FontRole,
        weight: FontWeight,
    ) -> rusqlite::Result<bool> {
        Ok(self.0.execute(
            "DELETE FROM brand_fonts WHERE workspace_id = ?1 AND role = ?2 AND weight = ?3",
            params![workspace_id, role.as_str(), weight.as_str()],
        )? == 1)
    }

    /// The bytes of one of the workspace's saved font files, for showing a specimen.
    pub fn brand_font_bytes(
        &self,
        media_root: &Path,
        workspace_id: i64,
        sha256: &str,
    ) -> Result<Vec<u8>, Box<dyn Error>> {
        let saved: bool = self.0.query_row(
            "SELECT EXISTS(SELECT 1 FROM brand_fonts WHERE workspace_id = ?1 AND sha256 = ?2)",
            params![workspace_id, sha256],
            |row| row.get(0),
        )?;
        if !saved {
            return Err(io::Error::new(io::ErrorKind::NotFound, "Font not found").into());
        }
        Ok(fs::read(font_file(media_root, workspace_id, sha256))?)
    }

    /// The fonts a run's snapshot chose, so re-renders match the first render.
    pub(crate) fn run_font_set(
        &self,
        media_root: &Path,
        workspace_id: i64,
        run_id: i64,
    ) -> Result<FontSet, Box<dyn Error>> {
        let snapshot: String = self.0.query_row(
            "SELECT snapshot_json FROM generation_runs WHERE workspace_id = ?1 AND id = ?2",
            params![workspace_id, run_id],
            |row| row.get(0),
        )?;
        let snapshot: Value = serde_json::from_str(&snapshot)?;
        Ok(FontSet::from_fonts(
            media_root,
            workspace_id,
            &serde_json::from_value::<Vec<BrandFont>>(snapshot["brand_fonts"].clone())
                .unwrap_or_default(),
        ))
    }

    /// The workspace's fonts as they are now, for library animations.
    pub(crate) fn current_font_set(
        &self,
        media_root: &Path,
        workspace_id: i64,
    ) -> rusqlite::Result<FontSet> {
        Ok(FontSet::from_fonts(
            media_root,
            workspace_id,
            &self.brand_fonts(workspace_id)?,
        ))
    }
}

/// The font files animation text is drawn with. A missing bold uses the regular file of the
/// same role, a missing heading uses the body font, and a missing body uses the system font.
#[derive(Debug, Clone, Default)]
pub(crate) struct FontSet {
    files: Vec<(FontRole, FontWeight, PathBuf, f64)>,
}

impl FontSet {
    pub(crate) fn from_fonts(media_root: &Path, workspace_id: i64, fonts: &[BrandFont]) -> Self {
        let files = fonts
            .iter()
            .filter(|font| {
                font.sha256.len() == 64 && font.sha256.chars().all(|c| c.is_ascii_hexdigit())
            })
            .map(|font| {
                (
                    font.role,
                    font.weight,
                    font_file(media_root, workspace_id, &font.sha256),
                    font.advance,
                )
            })
            .filter(|(_, _, path, _)| path.is_file())
            .collect();
        Self { files }
    }

    fn find(&self, role: FontRole, weight: FontWeight) -> Option<(&Path, f64)> {
        let exact = |role, weight| {
            self.files
                .iter()
                .find(|entry| entry.0 == role && entry.1 == weight)
                .map(|entry| (entry.2.as_path(), entry.3))
        };
        let in_role = |role| exact(role, weight).or_else(|| exact(role, FontWeight::Regular));
        match role {
            FontRole::Heading => in_role(FontRole::Heading).or_else(|| in_role(FontRole::Body)),
            FontRole::Body => in_role(FontRole::Body),
        }
    }

    /// The brand file for `role` in `weight`, or `None` for the system font.
    pub(crate) fn file(&self, role: FontRole, weight: FontWeight) -> Option<&Path> {
        self.find(role, weight).map(|(path, _)| path)
    }

    pub(crate) fn advance(&self, role: FontRole, weight: FontWeight) -> f64 {
        self.find(role, weight)
            .map_or(SYSTEM_ADVANCE, |(_, advance)| advance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn system_font() -> Option<PathBuf> {
        [
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
        ]
        .iter()
        .map(PathBuf::from)
        .find(|path| path.is_file())
    }

    #[test]
    fn fonts_are_checked_saved_by_digest_and_fall_back_by_role() {
        let Some(source) = system_font() else {
            return;
        };
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("studio.sqlite");
        let root = crate::asset_root(&database);
        let store = WorkspaceStore::open(&database).unwrap();
        let workspace = store.create("Studio").unwrap().id;
        let font = store
            .import_brand_font(
                &root,
                workspace,
                FontRole::Body,
                FontWeight::Regular,
                &source,
            )
            .unwrap();
        assert_eq!(font.family, "DejaVu Sans");
        assert!((0.4..0.7).contains(&font.advance), "{}", font.advance);
        assert!(font_file(&root, workspace, &font.sha256).is_file());

        let not_font = temp.path().join("notes.ttf");
        fs::write(&not_font, b"plain text").unwrap();
        assert!(
            store
                .import_brand_font(
                    &root,
                    workspace,
                    FontRole::Body,
                    FontWeight::Bold,
                    &not_font
                )
                .is_err()
        );
        assert!(
            store
                .import_brand_font(
                    &root,
                    workspace + 1,
                    FontRole::Body,
                    FontWeight::Bold,
                    &source
                )
                .is_err()
        );

        let fonts = store.current_font_set(&root, workspace).unwrap();
        let regular = font_file(&root, workspace, &font.sha256);
        assert_eq!(
            fonts.file(FontRole::Heading, FontWeight::Bold),
            Some(regular.as_path())
        );
        assert_eq!(
            fonts.advance(FontRole::Heading, FontWeight::Bold),
            font.advance
        );
        assert!(
            FontSet::default()
                .file(FontRole::Body, FontWeight::Regular)
                .is_none()
        );

        assert!(
            store
                .remove_brand_font(workspace, FontRole::Body, FontWeight::Regular)
                .unwrap()
        );
        assert!(store.brand_fonts(workspace).unwrap().is_empty());
        assert!(regular.is_file(), "files stay for runs that used them");
    }
}
