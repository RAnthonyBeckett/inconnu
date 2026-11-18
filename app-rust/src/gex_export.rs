//! Bounded Grapevine v3 interchange projection, never a library backup.
mod story_projection;
use crate::{
    chronicle, importer,
    model::{Record, Result, Role, error},
    printing::{DocLanguage, PrintScope},
    sheet,
};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

pub const MAX_RECORDS: usize = 10_000;
const MAX_BYTES: usize = 32 * 1024 * 1024;
const MAX_TEXT: usize = 65_536;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RisenLegacyExportMode {
    #[default]
    Disallow,
    ExportAsWraith,
    ExportAsVarious,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Target {
    #[default]
    Character,
    Roster,
    Chronicle,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    pub scope: PrintScope,
    pub language: DocLanguage,
    pub normalize_typography: bool,
    pub risen: RisenLegacyExportMode,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            scope: PrintScope::Public,
            language: DocLanguage::English,
            normalize_typography: true,
            risen: RisenLegacyExportMode::Disallow,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Document {
    pub xml: String,
    pub warnings: Vec<String>,
    pub record_count: usize,
}
#[derive(Clone, Debug)]
pub struct Preview {
    pub document: Document,
    pub revision: i64,
    pub options: Options,
    pub target: Target,
    pub character_id: Option<String>,
}
#[derive(Deserialize)]
struct Mapping {
    tag: String,
    attributes: BTreeMap<String, String>,
    text: BTreeMap<String, String>,
    lists: BTreeMap<String, String>,
}
#[derive(Deserialize)]
struct NestedCatalogue {
    children: BTreeMap<String, BTreeMap<String, String>>,
    elements: BTreeMap<String, NestedElement>,
}
#[derive(Deserialize)]
struct NestedElement {
    attributes: BTreeMap<String, Vec<String>>,
    text: bool,
    #[serde(default)]
    field_aliases: Vec<String>,
}
fn nested_catalogue() -> &'static NestedCatalogue {
    static MAP: OnceLock<NestedCatalogue> = OnceLock::new();
    MAP.get_or_init(|| {
        serde_json::from_str(include_str!("../data/legacy_xml_nested.json"))
            .expect("reviewed nested XML mapping")
    })
}
pub(crate) fn xml_container(tag: &str) -> bool {
    nested_catalogue().children.contains_key(tag)
        || nested_catalogue()
            .elements
            .get(tag)
            .is_some_and(|e| !e.text)
}
fn mappings() -> &'static BTreeMap<String, Mapping> {
    static MAP: OnceLock<BTreeMap<String, Mapping>> = OnceLock::new();
    MAP.get_or_init(|| {
        serde_json::from_str(include_str!("../data/legacy_xml_mapping.json"))
            .expect("reviewed XML field mapping")
    })
}
pub(crate) fn list_key(name: &str) -> Option<&'static str> {
    mappings()
        .values()
        .flat_map(|m| &m.lists)
        .find_map(|(key, label)| (label == name).then_some(key.as_str()))
}
fn text<'a>(options: &Options, en: &'a str, pt: &'a str) -> &'a str {
    if options.language == DocLanguage::Portuguese {
        pt
    } else {
        en
    }
}
fn invalid() -> crate::model::AppError {
    error(
        "GEX_LIMIT_OR_STRUCTURE",
        "Legacy XML export exceeds supported bounds or contains an unsupported field structure. / A exportação XML excede os limites ou contém uma estrutura não suportada.",
    )
}
fn bounded_fields(fields: &Map<String, Value>) -> Result<()> {
    let mut stack: Vec<_> = fields.values().map(|v| (v, 0usize)).collect();
    let mut nodes = 0usize;
    let mut bytes = 0usize;
    while let Some((value, depth)) = stack.pop() {
        nodes += 1;
        if nodes > 200_000 || depth > 32 {
            return Err(invalid());
        }
        match value {
            Value::String(s) => {
                bytes = bytes.saturating_add(s.len());
                if s.len() > MAX_TEXT || bytes > MAX_BYTES {
                    return Err(invalid());
                }
            }
            Value::Array(a) => {
                if a.len() > 10_000 {
                    return Err(invalid());
                }
                stack.extend(a.iter().map(|v| (v, depth + 1)));
            }
            Value::Object(m) => {
                if m.len() > 1024 {
                    return Err(invalid());
                }
                stack.extend(m.values().map(|v| (v, depth + 1)));
            }
            _ => {}
        }
    }
    Ok(())
}
fn scalar(value: &Value) -> Result<String> {
    if let Some(flag) = value.get("value").and_then(Value::as_bool) {
        return Ok(if flag { "True" } else { "False" }.into());
    }
    match value {
        Value::String(s) => Ok(s.clone()),
        Value::Number(n) => Ok(n.to_string()),
        Value::Bool(b) => Ok(if *b { "True" } else { "False" }.into()),
        _ => Err(invalid()),
    }
}
fn field<'a>(fields: &'a Map<String, Value>, key: &str, xml: &str) -> Option<&'a Value> {
    fields.get(key).or_else(|| {
        fields
            .iter()
            .find_map(|(k, v)| (k.eq_ignore_ascii_case(xml)).then_some(v))
    })
}
struct Writer<'a> {
    xml: String,
    options: &'a Options,
    markers: Vec<(String, String)>,
    warnings: BTreeSet<&'static str>,
}
impl Writer<'_> {
    fn nested_fields(
        &mut self,
        parent: &str,
        fields: &Map<String, Value>,
        used: &mut BTreeSet<String>,
    ) -> Result<()> {
        if let Some(children) = nested_catalogue().children.get(parent) {
            for (tag, schema) in children {
                let spec = &nested_catalogue().elements[schema];
                for (key, value) in fields {
                    if spec.field_aliases.iter().any(|alias| {
                        alias.eq_ignore_ascii_case(key)
                            || format!("{alias}s").eq_ignore_ascii_case(key)
                    }) {
                        self.nested_node(tag, schema, value, 0)?;
                        used.insert(key.clone());
                    }
                }
            }
        }
        Ok(())
    }
    fn nested_node(&mut self, tag: &str, schema: &str, value: &Value, depth: usize) -> Result<()> {
        if depth > 16 {
            return Err(invalid());
        }
        if let Some(rows) = value.as_array() {
            if rows.len() > 4096 {
                return Err(invalid());
            }
            for row in rows {
                self.nested_node(tag, schema, row, depth + 1)?;
            }
            return Ok(());
        }
        let spec = &nested_catalogue().elements[schema];
        self.push("<")?;
        self.push(tag)?;
        let node_attrs = value.get("attributes").and_then(Value::as_array);
        if node_attrs.is_some_and(|a| a.len() > 128) {
            return Err(invalid());
        }
        for (xml, aliases) in &spec.attributes {
            let found = node_attrs
                .and_then(|attrs| {
                    attrs
                        .iter()
                        .find(|a| a["name"] == *xml)
                        .and_then(|a| a.get("value"))
                })
                .or_else(|| {
                    value.as_object().and_then(|m| {
                        m.iter().find_map(|(k, v)| {
                            aliases
                                .iter()
                                .any(|a| a.eq_ignore_ascii_case(k))
                                .then_some(v)
                        })
                    })
                });
            if let Some(v) = found {
                self.attr(xml, &scalar(v)?)?;
            }
        }
        if node_attrs.is_some_and(|a| {
            a.iter().any(|a| {
                a["name"]
                    .as_str()
                    .is_some_and(|key| !spec.attributes.contains_key(key))
            })
        }) {
            self.warnings.insert("unmapped");
        }
        self.push(">")?;
        if spec.text {
            if let Some(s) = value.as_str() {
                self.escaped(s)?;
            } else if let Some(children) = value.get("children").and_then(Value::as_array) {
                if children.len() > 4096 {
                    return Err(invalid());
                }
                for part in children {
                    if let Some(s) = part.get("text").and_then(Value::as_str) {
                        self.escaped(s)?;
                    } else {
                        self.warnings.insert("unmapped");
                    }
                }
            } else {
                return Err(invalid());
            }
        } else {
            if let Some(children) = value.get("children").and_then(Value::as_array) {
                if children.len() > 4096 {
                    return Err(invalid());
                }
                for child in children {
                    if let Some(child_tag) = child.get("tag").and_then(Value::as_str) {
                        if let Some(child_schema) = nested_catalogue()
                            .children
                            .get(schema)
                            .and_then(|m| m.get(child_tag))
                        {
                            self.nested_node(child_tag, child_schema, child, depth + 1)?;
                        } else {
                            self.warnings.insert("unmapped");
                        }
                    } else if child
                        .get("text")
                        .and_then(Value::as_str)
                        .is_some_and(|s| !s.trim().is_empty())
                    {
                        self.warnings.insert("unmapped");
                    }
                }
            } else if let Some(map) = value.as_object() {
                if map.len() > 1024 {
                    return Err(invalid());
                }
                let mut used = BTreeSet::new();
                // Binary and current typed projections use fields rather than raw XML nodes.
                if let Some(children) = nested_catalogue().children.get(schema) {
                    for (child_tag, child_schema) in children {
                        let child = &nested_catalogue().elements[child_schema];
                        for (key, v) in map {
                            if child.field_aliases.iter().any(|a| {
                                a.eq_ignore_ascii_case(key)
                                    || format!("{a}s").eq_ignore_ascii_case(key)
                            }) {
                                self.nested_node(child_tag, child_schema, v, depth + 1)?;
                                used.insert(key);
                            }
                        }
                    }
                }
            } else {
                return Err(invalid());
            }
        }
        self.push("</")?;
        self.push(tag)?;
        self.push(">\n")
    }
    fn push(&mut self, value: &str) -> Result<()> {
        if self.xml.len().saturating_add(value.len()) > MAX_BYTES {
            return Err(invalid());
        }
        self.xml.push_str(value);
        Ok(())
    }
    fn redact_public(&self, text: &str, normalize: bool) -> Result<String> {
        let mut value = Value::String(text.into());
        for _ in 0..16 {
            let before = value.clone();
            value = importer::redact(&value);
            for (start, end) in &self.markers {
                value = importer::redact_custom(&value, start, end);
                let (start, _) = normalize_text(start, normalize);
                let (end, _) = normalize_text(end, normalize);
                value = importer::redact_custom(&value, &start, &end);
            }
            if value == before {
                return Ok(value.as_str().unwrap_or_default().to_owned());
            }
        }
        Err(invalid())
    }
    fn safe(&mut self, value: &str, normalize: bool) -> Result<String> {
        if value.len() > MAX_TEXT {
            return Err(invalid());
        }
        let mut value = value.to_owned();
        if self.options.scope == PrintScope::Public {
            value = self.redact_public(&value, normalize)?;
        }
        if self.options.scope == PrintScope::PrivateStoryteller {
            for (start, end) in &self.markers {
                if !start.is_empty() && !end.is_empty() {
                    value = replace_marker(&value, start, "[ST]");
                    value = replace_marker(&value, end, "[/ST]");
                    if value.len() > MAX_TEXT * 6 {
                        return Err(invalid());
                    }
                }
            }
        }
        let (out, controls) = normalize_text(&value, normalize);
        if controls {
            self.warnings.insert("controls");
        }
        if self.options.scope == PrintScope::Public {
            self.redact_public(&out, normalize)
        } else {
            Ok(out)
        }
    }
    fn escape_clean(&mut self, value: &str) -> Result<()> {
        for c in value.chars() {
            match c {
                '&' => self.push("&amp;")?,
                '<' => self.push("&lt;")?,
                '>' => self.push("&gt;")?,
                '"' => self.push("&quot;")?,
                '\'' => self.push("&apos;")?,
                '\n' => self.push("&#10;")?,
                '\r' => self.push("&#13;")?,
                '\t' => self.push("&#9;")?,
                _ => {
                    let mut buf = [0; 4];
                    self.push(c.encode_utf8(&mut buf))?;
                }
            }
        }
        Ok(())
    }
    fn escaped(&mut self, value: &str) -> Result<()> {
        let safe = self.safe(value, self.options.normalize_typography)?;
        self.escape_clean(&safe)
    }
    fn attr(&mut self, key: &str, value: &str) -> Result<()> {
        self.push(" ")?;
        self.push(key)?;
        self.push("=\"")?;
        if key == "id" || key == "inconnuprivate" || key == "inconnupublic" {
            let safe = self.safe(value, false)?;
            if safe != value {
                return Err(error(
                    "GEX_IDENTIFIER_REDACTION",
                    "An identifier or visibility declaration cannot be exported unchanged. / Um identificador ou declaração de visibilidade não pode ser exportado sem alterações.",
                ));
            }
            self.escape_clean(value)?;
        } else {
            self.escaped(value)?;
        }
        self.push("\"")
    }
    fn child(&mut self, key: &str, value: &str) -> Result<()> {
        self.push("<")?;
        self.push(key)?;
        self.push(">")?;
        self.escaped(value)?;
        self.push("</")?;
        self.push(key)?;
        self.push(">\n")
    }
    fn list(&mut self, label: &str, value: &Value) -> Result<()> {
        let rows = value
            .get("Traits")
            .and_then(Value::as_array)
            .ok_or_else(invalid)?;
        if rows.len() > 4096 {
            return Err(invalid());
        }
        self.push("<traitlist")?;
        self.attr("name", label)?;
        for (key, xml) in [
            ("Alphabetized", "abc"),
            ("Atomic", "atomic"),
            ("Negative", "negative"),
            ("Display", "display"),
        ] {
            if let Some(value) = value.get(key) {
                self.attr(xml, &scalar(value)?)?;
            }
        }
        self.push(">\n")?;
        for row in rows {
            let name = row
                .get("Name")
                .and_then(Value::as_str)
                .ok_or_else(invalid)?;
            self.push("<trait")?;
            self.attr("name", name)?;
            self.attr(
                "val",
                row.get("Value")
                    .map(scalar)
                    .transpose()?
                    .as_deref()
                    .unwrap_or("1"),
            )?;
            if self.options.scope == PrintScope::PrivateStoryteller
                && let Some(note) = row.get("Note").and_then(Value::as_str)
            {
                self.attr("note", note)?;
            }
            self.push("/>\n")?;
        }
        self.push("</traitlist>\n")
    }
}
fn normalize_text(value: &str, typography: bool) -> (String, bool) {
    let mut out = String::new();
    let mut controls = false;
    for c in value.chars() {
        if !(matches!(c, '\t' | '\n' | '\r')
            || ('\u{20}'..='\u{d7ff}').contains(&c)
            || ('\u{e000}'..='\u{fffd}').contains(&c)
            || ('\u{10000}'..='\u{10ffff}').contains(&c))
        {
            controls = true;
            continue;
        }
        let replacement = if typography {
            match c {
                '\u{2018}' | '\u{2019}' | '\u{201a}' => "'",
                '\u{201c}' | '\u{201d}' | '\u{201e}' => "\"",
                '\u{2013}' | '\u{2014}' => "-",
                '\u{2026}' => "...",
                '\u{a0}' => " ",
                _ => "",
            }
        } else {
            ""
        };
        if replacement.is_empty() {
            out.push(c);
        } else {
            out.push_str(replacement);
        }
    }
    (out, controls)
}
fn replace_marker(value: &str, marker: &str, replacement: &str) -> String {
    if !marker.is_ascii() {
        return value.replace(marker, replacement);
    }
    let mut out = String::new();
    let mut rest = value;
    while let Some(i) = rest
        .as_bytes()
        .windows(marker.len())
        .position(|w| w.eq_ignore_ascii_case(marker.as_bytes()))
    {
        out.push_str(&rest[..i]);
        out.push_str(replacement);
        rest = &rest[i + marker.len()..];
    }
    out.push_str(rest);
    out
}

/// Serializes explicit fields only. Even private export never emits originals, app receipts or audits.
pub fn serialize(
    records: &[Record],
    profile: Option<&chronicle::Profile>,
    options: &Options,
    role: Role,
) -> Result<Document> {
    if options.scope == PrintScope::PrivateStoryteller && role != Role::Storyteller {
        return Err(error(
            "ACCESS_DENIED",
            "Private XML requires the local Storyteller view. / XML privado exige a visão do Narrador.",
        ));
    }
    if records.len() > MAX_RECORDS {
        return Err(invalid());
    }
    let markers: BTreeSet<_> = records
        .iter()
        .filter_map(|r| r.st_markers.clone())
        .collect();
    if markers.len() > 64 {
        return Err(invalid());
    }
    let mut w = Writer {
        xml: String::new(),
        options,
        markers: markers.into_iter().collect(),
        warnings: BTreeSet::new(),
    };
    w.push("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<grapevine version=\"3\"")?;
    if options.scope == PrintScope::PrivateStoryteller {
        w.push(" stcommentstart=\"[ST]\" stcommentend=\"[/ST]\"")?;
    }
    if let Some(p) = profile {
        w.attr("chronicle", &p.title)?;
        if options.scope == PrintScope::PrivateStoryteller || p.publish_details {
            w.attr("website", &p.website)?;
            w.attr("usualtime", &p.meeting_time)?;
        }
    }
    w.push(">\n")?;
    if let Some(p) = profile
        && (options.scope == PrintScope::PrivateStoryteller || p.publish_details)
    {
        w.child("description", &p.description)?;
        w.child("usualplace", &p.meeting_place)?;
    }
    let mut count = 0;
    let mut omissions = Vec::new();
    for record in records {
        if record.build.is_some() {
            return Err(error(
                "GEX_BUILD_COPY",
                "Build copies cannot be transferred as live legacy characters. / Cópias de construção não podem ser transferidas como personagens ativos.",
            ));
        }
        if options.scope == PrintScope::Public
            && !sheet::is_character_kind(&record.kind)
            && !crate::world::is_world_kind(&record.kind)
        {
            w.warnings.insert("private-records");
            continue;
        }
        let kind = if record.kind == "Risen" {
            w.warnings.insert("risen");
            match options.risen {
                RisenLegacyExportMode::Disallow => {
                    return Err(error(
                        "GEX_RISEN_MODE_REQUIRED",
                        "Legacy XML has no native Risen tag. Explicitly choose Export as Wraith or Export as Various. / XML legado não possui tag Risen nativa. Escolha explicitamente Exportar como Wraith ou Various.",
                    ));
                }
                RisenLegacyExportMode::ExportAsWraith => "Wraith",
                RisenLegacyExportMode::ExportAsVarious => "Various",
            }
        } else {
            record.kind.as_str()
        };
        let mapping=mappings().get(kind).ok_or_else(||error("GEX_UNSUPPORTED_KIND","This record kind has no reviewed legacy XML mapping. / Este tipo de registro não possui mapeamento XML revisado."))?;
        bounded_fields(&record.public_fields)?;
        if options.scope == PrintScope::PrivateStoryteller {
            bounded_fields(&record.private_fields)?;
        }
        let mut fields = record
            .public_fields
            .iter()
            .filter(|(k, _)| !k.starts_with("App") && !k.starts_with("_App"))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<Map<_, _>>();
        if options.scope == PrintScope::PrivateStoryteller {
            fields.extend(
                record
                    .private_fields
                    .iter()
                    .filter(|(k, _)| !k.starts_with("App") && !k.starts_with("_App"))
                    .map(|(k, v)| (k.clone(), v.clone())),
            );
            if let Some(notes) = record.private_fields.get("AppNotes") {
                fields.insert("Notes".into(), notes.clone());
            }
            story_projection::apply(record, &mut fields)?;
        }
        if record.kind == "Query" {
            if let Some(inventory) = fields.get("Inventory").and_then(Value::as_i64) {
                let table = match inventory {
                    1 => "chars",
                    2 => "players",
                    4 => "items",
                    128 => "rotes",
                    8 => "locs",
                    16 => "actions",
                    32 => "plots",
                    64 => "rumors",
                    _ => {
                        return Err(error(
                            "GEX_QUERY_TABLE",
                            "The query inventory has no exact legacy XML table. / O inventário da consulta não possui tabela XML legada exata.",
                        ));
                    }
                };
                fields.insert("InvStr".into(), Value::String(table.into()));
            }
            if let Some(flag) = fields.get("MatchAll").and_then(|v| {
                v.as_bool()
                    .or_else(|| v.get("value").and_then(Value::as_bool))
            }) {
                fields.insert(
                    "MatchAll".into(),
                    Value::String(if flag { "all" } else { "any" }.into()),
                );
            }
        }
        if fields.len() > 1024 {
            return Err(invalid());
        }
        let projected = Record {
            id: String::new(),
            kind: record.kind.clone(),
            name: record.name.clone(),
            player_id: None,
            legacy_id: None,
            public_fields: fields.clone(),
            private_fields: Map::new(),
            original: Value::Null,
            source_sha256: String::new(),
            st_markers: None,
            build: None,
            xp_ledger: None,
        };
        // Includes supported lists conservatively backfilled from older XML imports.
        let lists = sheet::extract(&projected).lists;
        let mut used = BTreeSet::new();
        w.push("<")?;
        w.push(&mapping.tag)?;
        let visible: Vec<_> = record
            .public_fields
            .keys()
            .filter(|k| {
                sheet::LIST_FIELDS.contains(&k.as_str())
                    || sheet::RESOURCE_FIELDS.contains(&k.as_str())
                    || sheet::IDENTITY_FIELDS.contains(&k.as_str())
            })
            .cloned()
            .collect();
        w.attr("inconnupublic", &serde_json::to_string(&visible)?)?;
        if options.scope == PrintScope::PrivateStoryteller {
            // Legacy consumers ignore this additive attribute. Inconnu honors it only to
            // narrow visibility, so a private transfer does not publish private-only fields.
            let hidden: Vec<_> = record
                .private_fields
                .keys()
                .filter(|k| {
                    !record.public_fields.contains_key(*k)
                        && (mapping.attributes.values().any(|v| v == *k)
                            || mapping.text.values().any(|v| v == *k)
                            || mapping.lists.contains_key(*k)
                            || sheet::LIST_FIELDS.contains(&k.as_str()))
                })
                .cloned()
                .collect();
            if !hidden.is_empty() {
                w.attr("inconnuprivate", &serde_json::to_string(&hidden)?)?;
                w.warnings.insert("visibility");
            }
        }
        let name = fields
            .get("Name")
            .and_then(Value::as_str)
            .unwrap_or(&record.name);
        if mapping.attributes.contains_key("name") {
            w.attr("name", name)?;
            used.insert("Name".to_owned());
        }
        for (xml, key) in &mapping.attributes {
            if xml == "name" {
                continue;
            }
            if xml == "ethnos" && record.kind == "Risen" {
                w.attr(xml, "Risen")?;
                used.insert(key.clone());
                continue;
            }
            if xml == "id"
                && let Some(id) = &record.legacy_id
            {
                w.attr(xml, id)?;
                used.insert(key.clone());
                continue;
            }
            if options.scope == PrintScope::Public
                && matches!(
                    key.as_str(),
                    "EMail" | "Email" | "Phone" | "Address" | "Notes" | "Narrator" | "LastModified"
                )
            {
                continue;
            }
            if let Some(value) = field(&fields, key, xml) {
                w.attr(xml, &scalar(value)?)?;
                used.insert(key.clone());
            }
        }
        w.push(">\n")?;
        for (xml, key) in &mapping.text {
            if options.scope == PrintScope::Public
                && matches!(
                    key.as_str(),
                    "Notes"
                        | "Address"
                        | "Biography"
                        | "Other"
                        | "DarkPassions"
                        | "Life"
                        | "Death"
                        | "Haunt"
                        | "Regret"
                        | "Passions"
                        | "Fetters"
                        | "Oaths"
                )
            {
                continue;
            }
            if let Some(value) = field(&fields, key, xml)
                .or_else(|| (key == "Notes").then(|| fields.get("AppNotes")).flatten())
            {
                w.child(xml, &scalar(value)?)?;
                used.insert(key.clone());
            }
        }
        for (key, value) in &lists {
            let label = mapping
                .lists
                .get(key)
                .or_else(|| mappings().values().find_map(|m| m.lists.get(key)));
            if let Some(label) = label {
                if !mapping.lists.contains_key(key) {
                    w.warnings.insert("extra-lists");
                }
                w.list(label, value)?;
                used.insert(key.clone());
            } else {
                w.warnings.insert("unmapped");
            }
        }
        // World and other legacy record lists do not belong to sheet::LIST_FIELDS.
        for (key, label) in &mapping.lists {
            if !used.contains(key)
                && let Some(value) = fields.get(key)
            {
                w.list(label, value)?;
                used.insert(key.clone());
            }
        }
        if options.scope == PrintScope::PrivateStoryteller {
            w.nested_fields(&mapping.tag, &fields, &mut used)?;
        }
        let omitted: Vec<_> = fields
            .keys()
            .filter(|key| {
                !used.contains(*key)
                    && !key.starts_with("App")
                    && !key.starts_with("_App")
                    && !matches!(
                        key.as_str(),
                        "Notes" | "CharacterType" | "UnmappedTraitLists"
                    )
            })
            .map(|k| {
                k.chars()
                    .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .take(128)
                    .collect::<String>()
            })
            .collect();
        if !omitted.is_empty() {
            w.warnings.insert("unmapped");
            omissions.push(format!(
                "{} {} ({}): {}",
                text(
                    options,
                    "Omitted fields for record",
                    "Campos omitidos do registro"
                ),
                count + 1,
                kind,
                omitted.join(", ")
            ));
        }
        w.push("</")?;
        w.push(&mapping.tag)?;
        w.push(">\n")?;
        count += 1;
    }
    w.push("</grapevine>\n")?;
    let mut warnings=vec![text(options,"Legacy interchange projection: original XML, internal receipts, audits and application-only data are excluded. Use a library backup for full preservation.","Projeção de intercâmbio legado: XML original, recibos internos, auditorias e dados exclusivos do aplicativo são excluídos. Use um backup para preservação completa.").into()];
    for key in &w.warnings {
        warnings.push(match *key {
        "risen"=>text(options,"Risen exported using the explicitly selected Wraith/Various tag; legacy XML has no native Risen tag.","Risen exportado com a tag Wraith/Various escolhida explicitamente; XML legado não possui tag Risen nativa."),
        "extra-lists"=>text(options,"Additional trait lists are preserved in XML, but an original recipient may ignore lists outside that legacy character type.","Listas adicionais foram preservadas no XML, mas o destinatário original pode ignorar listas fora desse tipo de personagem."),
        "controls"=>text(options,"XML-invalid control characters were removed.","Caracteres de controle inválidos em XML foram removidos."),
        "private-records"=>text(options,"Player/contact and private story records were excluded from the public export.","Registros de jogador/contato e histórias privadas foram excluídos da exportação pública."),
        "visibility"=>text(options,"Private field visibility is preserved by Inconnu metadata; other legacy importers may ignore it. Keep this transfer private and review recipient visibility.","A visibilidade privada é preservada por metadados do Inconnu; outros importadores podem ignorá-los. Mantenha esta transferência privada e revise a visibilidade no destino."),
        _=>text(options,"Some fields or nested legacy structures have no reviewed export mapping and were omitted. Review this projection before transferring; keep a library backup.","Alguns campos ou estruturas legadas não possuem mapeamento revisado e foram omitidos. Revise esta projeção antes de transferir e mantenha um backup."),
    }.into());
    }
    warnings.extend(omissions);
    Ok(Document {
        xml: w.xml,
        warnings,
        record_count: count,
    })
}
