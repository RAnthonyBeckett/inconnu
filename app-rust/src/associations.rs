//! Explicit local links and unresolved legacy character labels.
//! Legacy labels are never resolved by name or treated as ownership proof.
use crate::{
    model::{Language, Record, Result, error},
    plots::{attr, children},
    world, world_card,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

pub(crate) const FIELD: &str = "AppAssociationsV1";
const LINK_LIMIT: usize = 256;
const LEGACY_LIMIT: usize = 4096;
const TEXT_LIMIT: usize = 16_000;
const ID_LIMIT: usize = 256;
const HTML_LIMIT: usize = 16 * 1024 * 1024;
const EQUIPMENT_XML_NAMES: &[&str] = &["Equipment"];
const ROTE_XML_NAMES: &[&str] = &["Rotes"];
const LOCATION_XML_NAMES: &[&str] = &["Locations"];
const FERA_LOCATION_XML_NAMES: &[&str] = &["Locations", "Location"];
const NO_XML_NAMES: &[&str] = &[];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum Kind {
    Item,
    Rote,
    Location,
}

impl Kind {
    pub fn record_kind(self) -> &'static str {
        match self {
            Self::Item => "Item",
            Self::Rote => "Rote",
            Self::Location => "Location",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Link {
    pub target_id: String,
    pub kind: Kind,
    pub label_at_binding: String,
    pub quantity: u16,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub share_challenge_bonus: bool,
    pub note: String,
}

#[derive(Clone, Debug)]
pub struct Resolved {
    pub link: Link,
    pub current: Option<world::Edit>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Legacy {
    pub kind: Kind,
    pub label: String,
    pub value: String,
    pub note: String,
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub character_id: String,
    pub character_name: String,
    pub revision: i64,
    pub linked: Vec<Resolved>,
    pub legacy: Vec<Legacy>,
    /// Count of malformed, unknown, or over-limit source values kept in original data.
    pub retained: usize,
}

pub(crate) fn validate(links: &[Link]) -> Result<()> {
    if links.len() > LINK_LIMIT {
        return Err(error(
            "ASSOCIATION_LIMIT",
            "Keep no more than 256 explicit links on one character.",
        ));
    }
    let mut targets = HashSet::with_capacity(links.len());
    for link in links {
        if link.target_id.trim().is_empty()
            || link.target_id.len() > ID_LIMIT
            || link.target_id != link.target_id.trim()
            || link.target_id.contains('\0')
            || link.label_at_binding.trim().is_empty()
            || link.label_at_binding.len() > TEXT_LIMIT
            || link.label_at_binding.contains('\0')
            || link.note.len() > TEXT_LIMIT
            || link.note.contains('\0')
        {
            return Err(error(
                "ASSOCIATION_FIELDS_INVALID",
                "Enter a target ID and label; keep labels and notes within the supported text limit.",
            ));
        }
        let quantity_valid = match link.kind {
            Kind::Item => (1..=999).contains(&link.quantity),
            Kind::Rote | Kind::Location => link.quantity == 1 && !link.share_challenge_bonus,
        };
        if !quantity_valid {
            return Err(error(
                "ASSOCIATION_QUANTITY_INVALID",
                "Item quantity must be 1 through 999; rote and location quantity is 1.",
            ));
        }
        if !targets.insert(link.target_id.to_ascii_lowercase()) {
            return Err(error(
                "ASSOCIATION_DUPLICATE",
                "A target record can appear only once in the explicit links.",
            ));
        }
    }
    Ok(())
}

/// Project existing name-based lists as unresolved labels for review.
/// Persisted canonical fields take precedence over XML; raw XML is read only
/// when the matching EquipmentList, RoteList or HangoutList field is absent.
pub(crate) fn legacy(record: &Record) -> (Vec<Legacy>, usize) {
    if !is_character_kind(&record.kind) {
        return (Vec::new(), 0);
    }

    let mut result = Vec::new();
    let mut retained = 0;
    let rote_names = if record.kind == "Mage" {
        ROTE_XML_NAMES
    } else {
        NO_XML_NAMES
    };
    let location_names = if record.kind == "Fera" {
        FERA_LOCATION_XML_NAMES
    } else {
        LOCATION_XML_NAMES
    };
    for (field, kind, xml_names) in [
        ("EquipmentList", Kind::Item, EQUIPMENT_XML_NAMES),
        ("RoteList", Kind::Rote, rote_names),
        ("HangoutList", Kind::Location, location_names),
    ] {
        if let Some(value) = record
            .private_fields
            .get(field)
            .or_else(|| record.public_fields.get(field))
        {
            project_persisted_list(value, kind, xml_names, &mut result, &mut retained);
        } else if let Some(xml) = record.original.get("xml") {
            project_xml_list(xml, kind, xml_names, &mut result, &mut retained);
        }
    }
    (result, retained)
}

fn is_character_kind(kind: &str) -> bool {
    matches!(
        kind,
        "Vampire"
            | "Werewolf"
            | "Mortal"
            | "Changeling"
            | "Wraith"
            | "Mage"
            | "Fera"
            | "Various"
            | "Mummy"
            | "KueiJin"
            | "Hunter"
            | "Demon"
    )
}

fn project_persisted_list(
    value: &Value,
    kind: Kind,
    expected_names: &[&str],
    output: &mut Vec<Legacy>,
    retained: &mut usize,
) {
    match value {
        Value::Object(_) => project_list_object(value, kind, expected_names, output, retained),
        Value::Array(lists) => {
            for list in lists {
                if list.get("Traits").and_then(Value::as_array).is_some() {
                    project_list_object(list, kind, expected_names, output, retained);
                } else {
                    *retained += 1;
                }
            }
        }
        _ => *retained += 1,
    }
}

fn project_list_object(
    list: &Value,
    kind: Kind,
    expected_names: &[&str],
    output: &mut Vec<Legacy>,
    retained: &mut usize,
) {
    let Some(object) = list.as_object() else {
        *retained += 1;
        return;
    };
    if object.keys().any(|key| {
        ![
            "Name",
            "Alphabetized",
            "Atomic",
            "Negative",
            "Display",
            "Traits",
        ]
        .contains(&key.as_str())
    }) {
        *retained += 1;
    }
    if object
        .get("Name")
        .and_then(Value::as_str)
        .is_some_and(|name| !expected_names.contains(&name))
    {
        *retained += 1;
    }
    let Some(rows) = object.get("Traits").and_then(Value::as_array) else {
        *retained += 1;
        return;
    };
    for row in rows {
        project_persisted_row(row, kind, output, retained);
    }
}

fn project_persisted_row(row: &Value, kind: Kind, output: &mut Vec<Legacy>, retained: &mut usize) {
    let Some(object) = row.as_object() else {
        *retained += 1;
        return;
    };
    if object
        .keys()
        .any(|key| !["Name", "Value", "Note"].contains(&key.as_str()))
    {
        *retained += 1;
    }
    let Some(label) = object.get("Name").and_then(Value::as_str) else {
        *retained += 1;
        return;
    };
    if label.trim().is_empty() || label.len() > TEXT_LIMIT || label.contains('\0') {
        *retained += 1;
        return;
    }
    let value = persisted_text(object.get("Value"), "1", retained);
    let note = persisted_text(object.get("Note"), "", retained);
    push_legacy(
        Legacy {
            kind,
            label: label.to_owned(),
            value,
            note,
        },
        output,
        retained,
    );
}

fn persisted_text(value: Option<&Value>, default: &str, retained: &mut usize) -> String {
    match value {
        None => default.to_owned(),
        Some(Value::String(text)) if text.len() <= TEXT_LIMIT && !text.contains('\0') => {
            text.clone()
        }
        Some(Value::Number(number)) => {
            *retained += 1;
            number.to_string()
        }
        Some(_) => {
            *retained += 1;
            String::new()
        }
    }
}

fn project_xml_list(
    node: &Value,
    kind: Kind,
    names: &[&str],
    output: &mut Vec<Legacy>,
    retained: &mut usize,
) {
    let lists = children(node)
        .filter(|child| child["tag"] == "traitlist")
        .filter(|child| {
            attr(child, "name")
                .and_then(Value::as_str)
                .is_some_and(|name| names.contains(&name))
        })
        .collect::<Vec<_>>();
    if lists.len() > 1 {
        // LinkedTraitList.InputFromFile clears the list each time, so only the
        // final matching list is effective in the legacy reader.
        *retained += lists.len() - 1;
    }
    if let Some(list) = lists.last() {
        project_xml_list_node(list, kind, output, retained);
    }
}

fn project_xml_list_node(list: &Value, kind: Kind, output: &mut Vec<Legacy>, retained: &mut usize) {
    if has_unknown_attributes(
        list,
        &[
            "name",
            "abc",
            "alphabetized",
            "atomic",
            "negative",
            "display",
        ],
    ) {
        *retained += 1;
    }
    for child in children(list) {
        match child["tag"].as_str() {
            Some("trait") => project_xml_row(child, kind, output, retained),
            Some(_) => *retained += 1,
            None if child["text"]
                .as_str()
                .is_some_and(|text| !text.trim().is_empty()) =>
            {
                *retained += 1;
            }
            None => {}
        }
    }
}

fn project_xml_row(row: &Value, kind: Kind, output: &mut Vec<Legacy>, retained: &mut usize) {
    if has_unknown_attributes(row, &["name", "val", "note"]) {
        *retained += 1;
    }
    let Some(label) = attr(row, "name").and_then(Value::as_str) else {
        *retained += 1;
        return;
    };
    if label.trim().is_empty() || label.len() > TEXT_LIMIT || label.contains('\0') {
        *retained += 1;
        return;
    }
    let value = xml_text_attribute(row, "val", "1", retained);
    let note = xml_text_attribute(row, "note", "", retained);
    push_legacy(
        Legacy {
            kind,
            label: label.to_owned(),
            value,
            note,
        },
        output,
        retained,
    );
}

fn xml_text_attribute(node: &Value, name: &str, default: &str, retained: &mut usize) -> String {
    match attr(node, name) {
        None => default.to_owned(),
        Some(Value::String(text)) if text.len() <= TEXT_LIMIT && !text.contains('\0') => {
            text.clone()
        }
        Some(Value::Number(number)) => {
            *retained += 1;
            number.to_string()
        }
        Some(_) => {
            *retained += 1;
            String::new()
        }
    }
}

fn has_unknown_attributes(node: &Value, known: &[&str]) -> bool {
    node["attributes"].as_array().is_some_and(|attributes| {
        attributes.iter().any(|attribute| {
            attribute["name"]
                .as_str()
                .is_none_or(|name| !known.contains(&name))
        })
    })
}

fn push_legacy(entry: Legacy, output: &mut Vec<Legacy>, retained: &mut usize) {
    if output.len() >= LEGACY_LIMIT {
        *retained += 1;
    } else {
        output.push(entry);
    }
}

pub fn html(snapshot: &Snapshot, language: Language, notes: bool) -> Result<String> {
    let pt = language == Language::Portuguese;
    let text = |english, translated| if pt { translated } else { english };
    let mut output = format!(
        "<!doctype html><html lang=\"{}\"><meta charset=\"utf-8\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'\"><title>{}</title><style>body{{font:16px system-ui;margin:2rem;color:#111}}h1,h2,h3{{overflow-wrap:anywhere}}p,dd,td{{white-space:pre-wrap;overflow-wrap:anywhere}}section{{break-inside:avoid;margin:1.5rem 0}}.card{{border-top:1px solid #888;padding:.7rem 0;break-inside:avoid}}dt{{font-weight:700}}dd{{margin:.2rem 0 .8rem}}table{{width:100%;border-collapse:collapse;table-layout:fixed}}td,th{{border:1px solid #888;padding:.35rem;overflow-wrap:anywhere}}thead{{display:table-header-group}}tr{{break-inside:avoid}}.muted{{color:#444}}@media print{{body{{margin:0}}}}</style><body><p><strong>{}</strong></p><h1>{}</h1><p>{}</p>",
        if pt { "pt-BR" } else { "en" },
        world_card::escape(text(
            "Private equipment report",
            "Relatório privado de equipamentos"
        )),
        world_card::escape(text(
            "Private Storyteller report",
            "Relatório privado do Narrador"
        )),
        world_card::escape(&snapshot.character_name),
        world_card::escape(text(
            "Explicit local links are shown separately from unresolved imported labels. This report does not establish ownership or transfer authority.",
            "Vínculos locais explícitos aparecem separados dos rótulos importados sem resolução. Este relatório não estabelece propriedade nem autoridade de transferência."
        )),
    );
    let mut omitted = 0usize;

    output.push_str(&format!(
        "<section><h2>{}</h2>",
        world_card::escape(text("Linked records", "Registros vinculados"))
    ));
    let mut linked_count = 0usize;
    for resolved in &snapshot.linked {
        if let Some(current) = &resolved.current {
            let fragment = render_resolved(resolved, current, language, notes);
            if output.len().saturating_add(fragment.len()) > HTML_LIMIT {
                omitted += 1;
            } else {
                output.push_str(&fragment);
                linked_count += 1;
            }
        }
    }
    if linked_count == 0 {
        output.push_str(&format!(
            "<p>{}</p>",
            world_card::escape(text("None", "Nenhum"))
        ));
    }
    output.push_str("</section>");

    output.push_str(&format!(
        "<section><h2>{}</h2>",
        world_card::escape(text(
            "Unresolved explicit links",
            "Vínculos explícitos não resolvidos"
        ))
    ));
    let mut unresolved_count = 0usize;
    for resolved in &snapshot.linked {
        if resolved.current.is_none() {
            let fragment = render_unresolved(resolved, language, notes);
            if output.len().saturating_add(fragment.len()) > HTML_LIMIT {
                omitted += 1;
            } else {
                output.push_str(&fragment);
                unresolved_count += 1;
            }
        }
    }
    if unresolved_count == 0 {
        output.push_str(&format!(
            "<p>{}</p>",
            world_card::escape(text("None", "Nenhum"))
        ));
    }
    output.push_str("</section>");

    output.push_str(&format!(
        "<section><h2>{}</h2><p class=\"muted\">{}</p>",
        world_card::escape(text(
            "Imported labels, unresolved",
            "Rótulos importados, não resolvidos"
        )),
        world_card::escape(text(
            "Legacy labels are preserved for review; no target record was inferred.",
            "Rótulos legados são preservados para revisão; nenhum registro de destino foi inferido."
        ))
    ));
    let mut legacy_count = 0usize;
    for entry in &snapshot.legacy {
        if entry.label.len() > TEXT_LIMIT
            || entry.value.len() > TEXT_LIMIT
            || entry.note.len() > TEXT_LIMIT
        {
            omitted += 1;
            continue;
        }
        let fragment = render_legacy(entry, language, notes);
        if output.len().saturating_add(fragment.len()) > HTML_LIMIT {
            omitted += 1;
        } else {
            output.push_str(&fragment);
            legacy_count += 1;
        }
    }
    if legacy_count == 0 {
        output.push_str(&format!(
            "<p>{}</p>",
            world_card::escape(text("None", "Nenhum"))
        ));
    }
    if snapshot.retained > 0 {
        let message = if pt {
            format!(
                "{} valores de origem malformados, desconhecidos ou acima do limite foram mantidos fora deste relatório.",
                snapshot.retained
            )
        } else {
            format!(
                "{} malformed, unknown or over-limit source values were retained outside this report.",
                snapshot.retained
            )
        };
        output.push_str(&format!(
            "<p class=\"muted\">{}</p>",
            world_card::escape(&message)
        ));
    }
    if omitted > 0 {
        let message = if pt {
            format!(
                "{} itens foram omitidos para manter o relatório impresso dentro do limite de tamanho.",
                omitted
            )
        } else {
            format!(
                "{} entries were omitted to keep the printable report within its size limit.",
                omitted
            )
        };
        output.push_str(&format!(
            "<p class=\"muted\">{}</p>",
            world_card::escape(&message)
        ));
    }
    output.push_str("</section></body></html>");
    Ok(output)
}

fn render_resolved(
    resolved: &Resolved,
    current: &world::Edit,
    language: Language,
    notes: bool,
) -> String {
    let mut output = format!(
        "<article class=\"card\"><h3>{} — {}</h3>",
        world_card::escape(world_card::label(
            resolved.link.kind.record_kind(),
            language
        )),
        world_card::escape(&current.name)
    );
    if !resolved.link.label_at_binding.is_empty() && resolved.link.label_at_binding != current.name
    {
        output.push_str(&format!(
            "<p>{}: {}</p>",
            world_card::escape(if language == Language::Portuguese {
                "Rótulo ao vincular"
            } else {
                "Label at binding"
            }),
            world_card::escape(&resolved.link.label_at_binding)
        ));
    }
    if resolved.link.kind == Kind::Item {
        output.push_str(&format!(
            "<p>{}: {}</p>",
            world_card::escape(if language == Language::Portuguese {
                "Quantidade"
            } else {
                "Quantity"
            }),
            resolved.link.quantity
        ));
    }
    output.push_str(&render_edit(resolved.link.kind, current, language, notes));
    if notes && !resolved.link.note.is_empty() {
        output.push_str(&format!(
            "<p><strong>{}:</strong> {}</p>",
            world_card::escape(if language == Language::Portuguese {
                "Nota do vínculo"
            } else {
                "Link note"
            }),
            world_card::escape(&resolved.link.note)
        ));
    }
    output.push_str("</article>");
    output
}

fn render_unresolved(resolved: &Resolved, language: Language, notes: bool) -> String {
    let mut output = format!(
        "<article class=\"card\"><h3>{} — {}</h3>",
        world_card::escape(world_card::label(
            resolved.link.kind.record_kind(),
            language
        )),
        world_card::escape(&resolved.link.label_at_binding)
    );
    if resolved.link.kind == Kind::Item {
        output.push_str(&format!(
            "<p>{}: {}</p>",
            world_card::escape(if language == Language::Portuguese {
                "Quantidade"
            } else {
                "Quantity"
            }),
            resolved.link.quantity
        ));
    }
    if notes && !resolved.link.note.is_empty() {
        output.push_str(&format!(
            "<p><strong>{}:</strong> {}</p>",
            world_card::escape(if language == Language::Portuguese {
                "Nota do vínculo"
            } else {
                "Link note"
            }),
            world_card::escape(&resolved.link.note)
        ));
    }
    output.push_str("</article>");
    output
}

fn render_legacy(entry: &Legacy, language: Language, notes: bool) -> String {
    let mut output = format!(
        "<article class=\"card\"><h3>{} — {}</h3>",
        world_card::escape(world_card::label(entry.kind.record_kind(), language)),
        world_card::escape(&entry.label)
    );
    if !entry.value.is_empty() {
        output.push_str(&format!(
            "<p>{}: {}</p>",
            world_card::escape(if language == Language::Portuguese {
                "Valor legado"
            } else {
                "Legacy value"
            }),
            world_card::escape(&entry.value)
        ));
    }
    if notes && !entry.note.is_empty() {
        output.push_str(&format!(
            "<p><strong>{}:</strong> {}</p>",
            world_card::escape(if language == Language::Portuguese {
                "Nota privada"
            } else {
                "Private note"
            }),
            world_card::escape(&entry.note)
        ));
    }
    output.push_str("</article>");
    output
}

fn render_edit(kind: Kind, edit: &world::Edit, language: Language, notes: bool) -> String {
    let record_kind = kind.record_kind();
    let mut output = String::from("<dl>");
    for field in world::text_fields(record_kind)
        .iter()
        .chain(world::integer_fields(record_kind))
    {
        if *field == "Notes" && !notes {
            continue;
        }
        let Some(value) = edit.fields.get(*field) else {
            continue;
        };
        if !world::value_supported(record_kind, field, value) {
            continue;
        }
        let rendered = match value {
            Value::String(text) => text.clone(),
            Value::Number(number) => number.to_string(),
            _ => continue,
        };
        output.push_str(&format!(
            "<dt>{}</dt><dd>{}</dd>",
            world_card::escape(world_card::label(field, language)),
            world_card::escape(&rendered)
        ));
    }
    output.push_str("</dl>");
    for field in world::list_fields(record_kind) {
        let Some(list) = edit.fields.get(*field) else {
            continue;
        };
        if !world::value_supported(record_kind, field, list) {
            continue;
        }
        output.push_str(&format!(
            "<h4>{}</h4><table><thead><tr><th>{}</th><th>{}</th>{}</tr></thead><tbody>",
            world_card::escape(world_card::label(field, language)),
            world_card::escape(if language == Language::Portuguese {
                "Nome"
            } else {
                "Name"
            }),
            world_card::escape(if language == Language::Portuguese {
                "Valor"
            } else {
                "Value"
            }),
            if notes {
                format!(
                    "<th>{}</th>",
                    world_card::escape(if language == Language::Portuguese {
                        "Nota privada"
                    } else {
                        "Private note"
                    })
                )
            } else {
                String::new()
            }
        ));
        for row in list["Traits"].as_array().into_iter().flatten() {
            let name = row["Name"].as_str().unwrap_or_default();
            let value = row["Value"].as_str().unwrap_or_default();
            output.push_str(&format!(
                "<tr><td>{}</td><td>{}</td>{}</tr>",
                world_card::escape(name),
                world_card::escape(value),
                if notes {
                    format!(
                        "<td>{}</td>",
                        world_card::escape(row["Note"].as_str().unwrap_or_default())
                    )
                } else {
                    String::new()
                }
            ));
        }
        output.push_str("</tbody></table>");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Map, json};

    fn fixture_record(kind: &str, original: Value) -> Record {
        Record {
            id: "character-id-secret".into(),
            kind: kind.into(),
            name: "Character".into(),
            player_id: None,
            legacy_id: None,
            public_fields: Map::new(),
            private_fields: Map::new(),
            original,
            source_sha256: "source-hash-secret".into(),
            st_markers: None,
            build: None,
            xp_ledger: None,
        }
    }

    fn xml_trait_list(name: &str, trait_name: &str, value: &str, note: &str) -> Value {
        json!({
            "tag":"traitlist",
            "attributes":[
                {"name":"name","value":name},
                {"name":"abc","value":"yes"},
                {"name":"atomic","value":"no"},
                {"name":"negative","value":"no"},
                {"name":"display","value":"4"}
            ],
            "children":[{
                "tag":"trait",
                "attributes":[
                    {"name":"name","value":trait_name},
                    {"name":"val","value":value},
                    {"name":"note","value":note}
                ],
                "children":[]
            }]
        })
    }

    #[test]
    fn xml_legacy_lists_use_serializer_names_and_remain_unresolved() {
        let mut record = fixture_record(
            "Mage",
            json!({"xml": {
                "tag":"mage",
                "attributes":[],
                "children":[
                    xml_trait_list("Equipment", "Silver knife", "2", "Combat note"),
                    xml_trait_list("Rotes", "Mind 2", "1", "Level 2"),
                    xml_trait_list("Locations", "Old haven", "1", "Entrance")
                ]
            }}),
        );
        let (entries, retained) = legacy(&record);
        assert_eq!(retained, 0);
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].kind, Kind::Item);
        assert_eq!(entries[0].label, "Silver knife");
        assert_eq!(entries[0].value, "2");
        assert_eq!(entries[0].note, "Combat note");
        assert_eq!(entries[1].kind, Kind::Rote);
        assert_eq!(entries[2].kind, Kind::Location);
        assert_eq!(entries[2].label, "Old haven");

        record
            .private_fields
            .insert(
                "EquipmentList".into(),
                json!({"Name":"Equipment","Traits":[{"Name":"Saved label","Value":"1","Note":"Saved note"}]}),
            );
        let (with_saved_list, retained) = legacy(&record);
        assert_eq!(retained, 0);
        assert!(
            with_saved_list
                .iter()
                .any(|entry| entry.label == "Saved label")
        );
        assert!(
            !with_saved_list
                .iter()
                .any(|entry| entry.label == "Silver knife")
        );

        let fera = fixture_record(
            "Fera",
            json!({"xml": {
                "tag":"fera",
                "attributes":[],
                "children":[xml_trait_list("Location", "Old den", "1", "Entrance")]
            }}),
        );
        let (fera_legacy, retained) = legacy(&fera);
        assert_eq!(retained, 0);
        assert_eq!(fera_legacy.len(), 1);
        assert_eq!(fera_legacy[0].kind, Kind::Location);
        assert_eq!(fera_legacy[0].label, "Old den");
    }

    #[test]
    fn invalid_legacy_rows_are_counted_and_retained_outside_projection() {
        let record = fixture_record(
            "Mage",
            json!({"xml": {
                "tag":"mage",
                "attributes":[],
                "children":[{
                    "tag":"traitlist",
                    "attributes":[{"name":"name","value":"Equipment"}],
                    "children":[
                        {"tag":"trait","attributes":[{"name":"name","value":"Dagger"},{"name":"unexpected","value":"keep"}],"children":[]},
                        {"tag":"unknown","children":[]}
                    ]
                }]
            }}),
        );
        let (legacy, retained) = legacy(&record);
        assert_eq!(legacy.len(), 1);
        assert_eq!(legacy[0].label, "Dagger");
        assert_eq!(legacy[0].value, "1");
        assert_eq!(retained, 2);
        assert_eq!(
            record.original["xml"]["children"][0]["children"][0]["attributes"][1]["value"],
            "keep"
        );
    }

    #[test]
    fn link_validation_enforces_explicit_unique_targets_and_kind_quantities() {
        let base = Link {
            target_id: "item-a".into(),
            kind: Kind::Item,
            label_at_binding: "Knife".into(),
            quantity: 1,
            share_challenge_bonus: false,
            note: String::new(),
        };
        assert!(validate(std::slice::from_ref(&base)).is_ok());
        let mut duplicate = base.clone();
        duplicate.target_id = "ITEM-A".into();
        assert!(validate(&[base.clone(), duplicate]).is_err());
        let mut invalid_rote = base;
        invalid_rote.kind = Kind::Rote;
        invalid_rote.quantity = 2;
        assert!(validate(&[invalid_rote]).is_err());
    }

    #[test]
    fn private_report_escapes_data_hides_ids_and_notes_by_default() {
        let item = world::Edit {
            name: "<Crown>".into(),
            fields: serde_json::from_value(json!({
                "ItemType":"<artifact>",
                "Powers":"<script>literal</script>",
                "Notes":"PRIVATE_NOTE_SECRET",
                "AbilityList":{"Name":"Abilities","Traits":[{"Name":"Skill","Value":"2","Note":"TRAIT_NOTE_SECRET","unknown":"HIDDEN_UNKNOWN"}]},
                "UnknownField":"HIDDEN_FIELD"
            }))
            .unwrap(),
        };
        let snapshot = Snapshot {
            character_id: "character-id-secret".into(),
            character_name: "Alice <Keeper>".into(),
            revision: 987654,
            linked: vec![
                Resolved {
                    link: Link {
                        target_id: "item-id-secret".into(),
                        kind: Kind::Item,
                        label_at_binding: "Old Crown".into(),
                        quantity: 2,
                        share_challenge_bonus: false,
                        note: "LINK_NOTE_SECRET".into(),
                    },
                    current: Some(item),
                },
                Resolved {
                    link: Link {
                        target_id: "missing-id-secret".into(),
                        kind: Kind::Rote,
                        label_at_binding: "Lost Rote <Name>".into(),
                        quantity: 1,
                        share_challenge_bonus: false,
                        note: "UNRESOLVED_NOTE_SECRET".into(),
                    },
                    current: None,
                },
            ],
            legacy: vec![Legacy {
                kind: Kind::Location,
                label: "Legacy Haven".into(),
                value: "1".into(),
                note: "LEGACY_NOTE_SECRET".into(),
            }],
            retained: 3,
        };

        for language in [Language::English, Language::Portuguese] {
            let report = html(&snapshot, language, false).unwrap();
            assert!(report.contains("&lt;script&gt;literal&lt;/script&gt;"));
            assert!(report.contains("Lost Rote &lt;Name&gt;"));
            assert!(report.contains("Legacy Haven"));
            assert!(!report.contains("<script>"));
            for secret in [
                "item-id-secret",
                "missing-id-secret",
                "character-id-secret",
                "source-hash-secret",
                "987654",
                "PRIVATE_NOTE_SECRET",
                "TRAIT_NOTE_SECRET",
                "LINK_NOTE_SECRET",
                "UNRESOLVED_NOTE_SECRET",
                "LEGACY_NOTE_SECRET",
                "HIDDEN_UNKNOWN",
                "HIDDEN_FIELD",
            ] {
                assert!(!report.contains(secret), "report leaked {secret}");
            }
            let private = html(&snapshot, language, true).unwrap();
            assert!(private.contains("PRIVATE_NOTE_SECRET"));
            assert!(private.contains("TRAIT_NOTE_SECRET"));
            assert!(private.contains("LINK_NOTE_SECRET"));
            assert!(private.contains("LEGACY_NOTE_SECRET"));
            assert!(!private.contains("HIDDEN_UNKNOWN") && !private.contains("item-id-secret"));
        }
    }
}
