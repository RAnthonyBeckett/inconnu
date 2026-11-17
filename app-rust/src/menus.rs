//! Read-only menu source previews; raw legacy costs/notes are not rules policy.
use crate::model::*;
use encoding_rs::Encoding;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs::File, io::Read, path::Path};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MenuRow {
    pub kind: String,
    pub name: String,
    pub path: Vec<String>,
    pub attributes: BTreeMap<String, String>,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MenuPreview {
    pub source_format: String,
    pub encoding: String,
    pub description: String,
    pub root_attributes: BTreeMap<String, String>,
    #[serde(skip)]
    pub(crate) original_bytes: Vec<u8>,
    pub source_name: String,
    pub sha256: String,
    pub rows: Vec<MenuRow>,
}
pub fn read(path: &Path) -> Result<Option<MenuPreview>> {
    read_with_encoding(path, "windows-1252")
}
pub fn read_with_encoding(path: &Path, encoding: &str) -> Result<Option<MenuPreview>> {
    let file = File::open(path)?;
    if !file.metadata()?.is_file()
        || file.metadata()?.len() > grapevine_legacy_reader::MAX_INPUT_BYTES as u64
    {
        return Err(error(
            "INPUT_LIMIT",
            "Choose a regular legacy source no larger than32MiB.",
        ));
    }
    let mut bytes = vec![];
    file.take(grapevine_legacy_reader::MAX_INPUT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    inspect_with_encoding(
        &bytes,
        &path.file_name().unwrap_or_default().to_string_lossy(),
        encoding,
    )
}
pub fn inspect(bytes: &[u8], name: &str) -> Result<Option<MenuPreview>> {
    inspect_with_encoding(bytes, name, "windows-1252")
}
pub fn inspect_with_encoding(
    bytes: &[u8],
    name: &str,
    encoding: &str,
) -> Result<Option<MenuPreview>> {
    if bytes.len() > grapevine_legacy_reader::MAX_INPUT_BYTES {
        return Err(error(
            "INPUT_LIMIT",
            "Source exceeds the bounded input limit.",
        ));
    }
    if bytes.starts_with(b"\x04\x00GVBM") {
        return binary(bytes, name, encoding).map(Some);
    }
    let Ok(text) = std::str::from_utf8(bytes) else {
        return Ok(None);
    };
    let Ok(doc) = roxmltree::Document::parse(text.trim_start_matches('\u{feff}')) else {
        return Ok(None);
    };
    let root = doc.root_element();
    if root.tag_name().name() != "grapevinemenus" {
        return Ok(None);
    }
    if root.attribute("version") != Some("3") {
        return Err(error(
            "UNSUPPORTED_MENU_VERSION",
            "This menu version has not been validated; nothing was imported.",
        ));
    }
    let count = root.children().filter(|n| n.has_tag_name("menu")).count();
    if root.attribute("size").and_then(|s| s.parse::<usize>().ok()) != Some(count) {
        return Err(error(
            "MENU_COUNT_MISMATCH",
            "Declared menu count does not match the source; nothing was imported.",
        ));
    }
    let mut rows = vec![];
    for n in root
        .descendants()
        .filter(|n| n.is_element() && *n != root && n.tag_name().name() != "description")
    {
        if rows.len() >= 100000 {
            return Err(error(
                "MENU_COUNT_LIMIT",
                "Menu source has too many entries.",
            ));
        }
        let path = n
            .ancestors()
            .filter_map(|a| a.attribute("name"))
            .map(str::to_owned)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        rows.push(MenuRow {
            kind: n.tag_name().name().into(),
            name: n.attribute("name").unwrap_or("").into(),
            path,
            attributes: n
                .attributes()
                .map(|a| (a.name().into(), a.value().into()))
                .collect(),
        });
    }
    Ok(Some(MenuPreview {
        source_format: "menu-xml-v3".into(),
        encoding: "utf-8".into(),
        description: root
            .children()
            .find(|n| n.has_tag_name("description"))
            .map(|n| {
                n.descendants()
                    .filter_map(|x| x.text().filter(|_| x.is_text()))
                    .collect::<String>()
            })
            .unwrap_or_default(),
        root_attributes: root
            .attributes()
            .map(|a| (a.name().into(), a.value().into()))
            .collect(),
        original_bytes: bytes.to_vec(),
        source_name: name.into(),
        sha256: Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
        rows,
    }))
}
/// Only an existing sibling basename is suggested; obsolete absolute paths stay evidence.
pub fn sibling_reference(source: &Path, reference: &str) -> Option<std::path::PathBuf> {
    let name = reference.rsplit(['\\', '/']).next()?;
    if name.is_empty() || name == "." || name == ".." {
        return None;
    }
    let candidate = source.parent()?.join(name);
    candidate.is_file().then_some(candidate)
}

struct Wire<'a> {
    bytes: &'a [u8],
    pos: usize,
    encoding: &'static Encoding,
}
impl Wire<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8]> {
        let end = self
            .pos
            .checked_add(n)
            .filter(|&e| e <= self.bytes.len())
            .ok_or_else(|| {
                error(
                    "MENU_TRUNCATED",
                    format!("Menu data ends before byte{}. No menu was saved.", self.pos),
                )
            })?;
        let out = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(out)
    }
    fn i16(&mut self) -> Result<i16> {
        Ok(i16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn count(&mut self) -> Result<usize> {
        let n = self.i16()?;
        usize::try_from(n).map_err(|_| {
            error(
                "MENU_INVALID_COUNT",
                "Menu contains a negative length or count.",
            )
        })
    }
    fn string(&mut self) -> Result<String> {
        let n = self.count()?;
        let encoding = self.encoding;
        let raw = self.take(n)?;
        let (text, _, errors) = encoding.decode(raw);
        if errors {
            return Err(error(
                "MENU_TEXT_DECODE",
                "Menu bytes do not decode in the selected code page.",
            ));
        }
        Ok(text.into_owned())
    }
    fn boolean(&mut self) -> Result<String> {
        match self.i16()? {
            0 => Ok("no".into()),
            -1 => Ok("yes".into()),
            _ => Err(error("MENU_INVALID_BOOL", "Invalid legacy menu Boolean.")),
        }
    }
}
fn binary(bytes: &[u8], name: &str, encoding: &str) -> Result<MenuPreview> {
    let encoding = Encoding::for_label(encoding.as_bytes())
        .ok_or_else(|| error("MENU_ENCODING", "Unknown menu text encoding."))?;
    let mut r = Wire {
        bytes,
        pos: 0,
        encoding,
    };
    if r.string()? != "GVBM" {
        return Err(error("MENU_HEADER", "Not a binary menu header."));
    }
    let version = f64::from_le_bytes(r.take(8)?.try_into().unwrap());
    if version != 3.0 {
        return Err(error(
            "UNSUPPORTED_MENU_VERSION",
            "Only validated version3 binary menus can be loaded; original bytes were not changed.",
        ));
    }
    let description = r.string()?;
    let count = r.count()?;
    let mut rows = vec![];
    for _ in 0..count {
        let menu = r.string()?;
        let mut attributes = BTreeMap::from([
            ("name".into(), menu.clone()),
            ("category".into(), r.i32()?.to_string()),
        ]);
        for key in ["abc", "negative", "autonote", "required"] {
            attributes.insert(key.into(), r.boolean()?);
        }
        attributes.insert("display".into(), r.i32()?.to_string());
        let nodes = r.count()?;
        if rows.len() + nodes + 1 > 100000 {
            return Err(error("MENU_COUNT_LIMIT", "Menu exceeds the entry limit."));
        }
        rows.push(MenuRow {
            kind: "menu".into(),
            name: menu.clone(),
            path: vec![menu.clone()],
            attributes,
        });
        for _ in 0..nodes {
            let item = r.string()?;
            let cost = r.string()?;
            let note = r.string()?;
            let kind = match cost.as_str() {
                ":" => "submenu",
                "+" => "include",
                _ => "item",
            };
            let mut attributes = BTreeMap::from([
                ("name".into(), item.clone()),
                ("raw_cost".into(), cost.clone()),
                ("raw_note".into(), note.clone()),
            ]);
            if kind == "item" {
                attributes.insert("cost".into(), cost);
                attributes.insert("note".into(), note);
            } else {
                attributes.insert("link".into(), note);
            }
            rows.push(MenuRow {
                kind: kind.into(),
                name: item.clone(),
                path: vec![menu.clone(), item],
                attributes,
            });
        }
    }
    if r.pos != bytes.len() {
        return Err(error(
            "MENU_TRAILING_DATA",
            "Menu has unparsed trailing bytes; it was not saved.",
        ));
    }
    Ok(MenuPreview {
        source_name: name.into(),
        sha256: Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
        rows,
        source_format: "gvbm-v3".into(),
        encoding: encoding.name().into(),
        description,
        root_attributes: BTreeMap::from([
            ("version".into(), "3".into()),
            ("size".into(), count.to_string()),
        ]),
        original_bytes: bytes.to_vec(),
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuSummary {
    pub sha256: String,
    pub source_name: String,
    pub source_format: String,
    pub rows: usize,
}
