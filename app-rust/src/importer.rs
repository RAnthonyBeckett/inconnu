use crate::model::*;
use serde_json::{Map, Value, json};
use std::{fs::File, io::Read, path::Path};
use uuid::Uuid;

pub fn read_preview(path: &Path, encoding: &str) -> Result<Preview> {
    let file = File::open(path)?;
    if !file.metadata()?.is_file()
        || file.metadata()?.len() > grapevine_legacy_reader::MAX_INPUT_BYTES as u64
    {
        return Err(error(
            "INPUT_LIMIT",
            "Choose a regular legacy file no larger than 32 MiB.",
        ));
    }
    let mut bytes = vec![];
    file.take(grapevine_legacy_reader::MAX_INPUT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    let mut preview = normalize(
        &bytes,
        &path.file_name().unwrap_or_default().to_string_lossy(),
        encoding,
    )?;
    preview.menu_candidate = preview
        .menu_reference
        .as_deref()
        .and_then(|r| crate::menus::sibling_reference(path, r));
    Ok(preview)
}

pub fn normalize(bytes: &[u8], name: &str, encoding: &str) -> Result<Preview> {
    let report = grapevine_legacy_reader::inspect(bytes, encoding);
    if !report.succeeded() {
        let detail = report
            .diagnostics
            .iter()
            .find(|d| d.severity == "error")
            .map(|d| format!("{} at {} (byte {:?})", d.code, d.path, d.offset))
            .unwrap_or_else(|| report.status.clone());
        crate::diagnostics::record("legacy_import", "INCOMPLETE_LEGACY_READ");
        return Err(error(
            "INCOMPLETE_LEGACY_READ",
            format!("Import was not committed: {detail}. Existing library data is unchanged."),
        ));
    }
    let xml_attrs = report
        .metadata
        .get("XmlRootAttributes")
        .and_then(Value::as_array);
    let marker = |key: &str, xml_key: &str, default: &str| {
        report
            .metadata
            .get("Game")
            .and_then(|g| g.get(key))
            .and_then(Value::as_str)
            .or_else(|| {
                xml_attrs
                    .and_then(|a| a.iter().find(|a| a["name"] == xml_key))
                    .and_then(|a| a["value"].as_str())
            })
            .unwrap_or(default)
            .to_string()
    };
    let start_marker = marker("STCommentStart", "stcommentstart", "[ST]");
    let end_marker = marker("STCommentEnd", "stcommentend", "[/ST]");
    let mut records = vec![];
    for (section, items) in &report.sections {
        if section == "unmapped" {
            return Err(error(
                "UNMAPPED_RECORD",
                "Unknown root record requires review before import.",
            ));
        }
        for (i, item) in items.iter().enumerate() {
            let (kind, fields) = if let Some(fields) = item.get("fields").and_then(Value::as_object)
            {
                (
                    fields
                        .get("CharacterType")
                        .and_then(Value::as_str)
                        .unwrap_or_else(|| item["type"].as_str().unwrap_or("Unknown"))
                        .to_string(),
                    fields.clone(),
                )
            } else {
                let tag = item["type"].as_str().unwrap_or("unknown");
                let kind = canonical_kind(tag).to_string();
                let xml = &item["xml"];
                let mut fields = xml_fields(xml);
                if matches!(kind.as_str(), "Item" | "Location" | "Rote") {
                    fields.extend(normalized_world_xml_fields(&kind, xml));
                    remove_mapped_world_lists(&mut fields, &kind);
                }
                (kind, fields)
            };
            let text = |key: &str| {
                fields
                    .get(key)
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string()
            };
            let mut record_name = text("Name");
            if record_name.is_empty() {
                record_name = text("Title");
            }
            if record_name.is_empty() {
                record_name = text("CharName");
            }
            if record_name.is_empty() {
                record_name = format!("{kind} {}", i + 1);
            }
            let legacy_id = text("ID");
            let mut public = Map::new();
            let mut private = Map::new();
            for (key, value) in fields {
                if public_field(&key) {
                    let safe = redact_custom(&redact(&value), &start_marker, &end_marker);
                    if safe != value {
                        private.insert(key.clone(), value);
                    }
                    public.insert(key, safe);
                } else {
                    private.insert(key, value);
                }
            }
            public.insert(
                "Name".into(),
                redact_custom(&redact(&json!(record_name)), &start_marker, &end_marker),
            );
            let id = Uuid::new_v5(
                &Uuid::NAMESPACE_URL,
                format!("grapevine/{}/{section}/{i}", report.sha256).as_bytes(),
            )
            .to_string();
            records.push(Record {
                id,
                kind,
                name: record_name,
                player_id: None,
                legacy_id: if legacy_id.is_empty() {
                    None
                } else {
                    Some(legacy_id)
                },
                public_fields: public,
                private_fields: private,
                original: item.clone(),
                source_sha256: report.sha256.clone(),
                st_markers: Some((start_marker.clone(), end_marker.clone())),
                build: None,
                xp_ledger: None,
            });
        }
    }
    // Resolve links only when a legacy player reference identifies exactly one player.
    let players = records
        .iter()
        .filter(|r| r.kind == "Player")
        .map(|r| (r.name.clone(), r.id.clone()))
        .collect::<Vec<_>>();
    let mut warnings=vec!["Original source and unmapped fields are retained privately. No character approval or rule validity is inferred.".into()];
    for record in &mut records {
        if let Some(reference) = record
            .public_fields
            .get("Player")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        {
            let matches = players
                .iter()
                .filter(|(name, _)| name == reference)
                .collect::<Vec<_>>();
            if matches.len() == 1 {
                record.player_id = Some(matches[0].1.clone());
            } else {
                warnings.push(format!("Unresolved player link for {}", record.name));
            }
        }
    }
    if report.format == "grapevine-xml" {
        warnings.push("XML fields are normalized conservatively; complete original trees remain available privately. Unmapped content is retained, not discarded.".into());
    }
    let counts = report
        .sections
        .iter()
        .map(|(k, v)| (k.clone(), v.len()))
        .collect();
    let menu_reference = report
        .metadata
        .get("Game")
        .and_then(|g| g.get("MenuFileName"))
        .and_then(Value::as_str)
        .or_else(|| {
            xml_attrs
                .and_then(|a| a.iter().find(|x| x["name"] == "menupath"))
                .and_then(|x| x["value"].as_str())
        })
        .filter(|s| !s.trim().is_empty())
        .map(str::to_owned);
    Ok(Preview {
        source_format: report.format.clone(),
        menu_reference,
        menu_candidate: None,
        source_name: name.into(),
        source_hash: report.sha256.clone(),
        records,
        counts,
        conflicts: vec![],
        warnings,
        bytes: bytes.to_vec(),
        report: serde_json::to_value(report)?,
    })
}

fn canonical_kind(tag: &str) -> &str {
    match tag {
        "player" => "Player",
        "vampire" => "Vampire",
        "werewolf" => "Werewolf",
        "mortal" => "Mortal",
        "changeling" => "Changeling",
        "wraith" => "Wraith",
        "mage" => "Mage",
        "fera" => "Fera",
        "various" => "Various",
        "mummy" => "Mummy",
        "kueijin" => "KueiJin",
        "hunter" => "Hunter",
        "demon" => "Demon",
        "item" => "Item",
        "rote" => "Rote",
        "location" => "Location",
        "query" => "Query",
        "action" => "Action",
        "plot" => "Plot",
        "rumor" => "Rumor",
        "award" => "Award",
        "template" => "Template",
        _ => tag,
    }
}

fn canonical_field(name: &str) -> String {
    static NAMES: std::sync::LazyLock<Vec<String>> = std::sync::LazyLock::new(|| {
        let schema: Value =
            serde_json::from_str(include_str!("../../compatibility/wire-schema.json"))
                .expect("embedded layout facts");
        schema["versions"]
            .as_object()
            .unwrap()
            .values()
            .flat_map(|p| p.as_object().unwrap().values())
            .flat_map(|r| r["fields"].as_array().unwrap().iter())
            .filter_map(|f| f["name"].as_str().map(String::from))
            .chain(
                [
                    "EMail",
                    "Phone",
                    "Address",
                    "Notes",
                    "Powers",
                    "Description",
                    "ItemType",
                    "ItemSubtype",
                    "Level",
                    "Duration",
                ]
                .into_iter()
                .map(String::from),
            )
            .collect()
    });
    NAMES
        .iter()
        .find(|key| key.eq_ignore_ascii_case(name))
        .cloned()
        .unwrap_or_else(|| name.into())
}

// Exact common list names from VampireClass.cls XML reader, plus existing family aliases.
pub(crate) fn xml_list_key(name: &str) -> Option<&'static str> {
    Some(match name {
        "Physical" => "PhysicalList",
        "Social" => "SocialList",
        "Mental" => "MentalList",
        "Negative Physical" => "PhysicalNegList",
        "Negative Social" => "SocialNegList",
        "Negative Mental" => "MentalNegList",
        "Abilities" => "AbilityList",
        "Influences" => "InfluenceList",
        "Backgrounds" => "BackgroundList",
        "Health Levels" => "HealthList",
        "Status" => "StatusList",
        "Bonds" => "BondList",
        "Beast Traits" | "Miscellaneous" => "MiscellaneousList",
        "Derangements" => "DerangementList",
        "Disciplines" => "DisciplineList",
        "Rituals" => "RitualList",
        "Merits" => "MeritList",
        "Flaws" => "FlawList",
        "Equipment" => "EquipmentList",
        "Locations" => "HangoutList",
        "Arcanoi" => "ArcanoiList",
        "Gifts" => "GiftList",
        "Arts" => "ArtList",
        "Realms" => "RealmList",
        "Spheres" => "SphereList",
        "Edges" => "EdgeList",
        "Hekau" => "HekauList",
        _ => return None,
    })
}

/// The pre-world-normalization XML projection, retained for safe read-time
/// backfills of records imported by earlier builds.
pub(crate) fn xml_fields(node: &Value) -> Map<String, Value> {
    let mut out = Map::new();
    if let Some(attrs) = node["attributes"].as_array() {
        for a in attrs {
            if let Some(key) = a["name"].as_str() {
                let key = canonical_field(key);
                let value = xml_resource_value(&key, a["value"].clone());
                out.insert(key, value);
            }
        }
    }
    if let Some(children) = node["children"].as_array() {
        for child in children.iter().filter(|c| c.get("tag").is_some()) {
            let tag = child["tag"].as_str().unwrap_or("");
            if tag == "traitlist" {
                let attrs = xml_attributes(child);
                let list_name = attrs.get("name").and_then(Value::as_str).unwrap_or("");
                let key = match list_name {
                    "Physical" => "PhysicalList",
                    "Social" => "SocialList",
                    "Mental" => "MentalList",
                    "Disciplines" => "DisciplineList",
                    "Arcanoi" => "ArcanoiList",
                    "Gifts" => "GiftList",
                    "Arts" => "ArtList",
                    "Realms" => "RealmList",
                    "Spheres" => "SphereList",
                    "Edges" => "EdgeList",
                    "Hekau" => "HekauList",
                    _ => "UnmappedTraitLists",
                };
                let traits=child["children"].as_array().into_iter().flatten().filter(|n|n["tag"]=="trait").map(|n|{let a=xml_attributes(n);json!({"Name":a.get("name").cloned().unwrap_or(json!("")),"Value":a.get("val").cloned().unwrap_or(json!("")),"Note":a.get("note").cloned().unwrap_or(json!(""))})}).collect::<Vec<_>>();
                let value = json!({"Name":list_name,"Traits":traits});
                if let Some(previous) = out.get_mut(key) {
                    if let Some(array) = previous.as_array_mut() {
                        array.push(value);
                    } else {
                        *previous = json!([previous.clone(), value]);
                    }
                } else {
                    out.insert(key.into(), value);
                }
                continue;
            }
            let mut value = child.clone();
            if let Some(parts) = child["children"].as_array()
                && parts.iter().all(|p| p.get("text").is_some())
            {
                value = json!(
                    parts
                        .iter()
                        .filter_map(|p| p["text"].as_str())
                        .collect::<String>()
                );
            }
            let key = canonical_field(tag);
            value = xml_resource_value(&key, value);
            // Repeated trait lists or unknown tags are arrays, never overwrite prior values.
            if let Some(previous) = out.get_mut(&key) {
                if let Some(array) = previous.as_array_mut() {
                    array.push(value);
                } else {
                    *previous = json!([previous.clone(), value]);
                }
            } else {
                out.insert(key, value);
            }
        }
    }
    out
}

/// Normalize only fields declared by the legacy Item, Location and Rote XML
/// readers. Unknown attributes and children remain in `xml_fields`, and the
/// caller retains the complete original XML subtree and source bytes.
pub(crate) fn normalized_world_xml_fields(kind: &str, node: &Value) -> Map<String, Value> {
    if !matches!(kind, "Item" | "Location" | "Rote") {
        return Map::new();
    }

    let mut out = Map::new();
    for attribute in node["attributes"].as_array().into_iter().flatten() {
        let Some(source_name) = attribute["name"].as_str() else {
            continue;
        };
        let Some((field, integer)) = world_xml_attribute(kind, source_name) else {
            continue;
        };
        let value = attribute["value"].clone();
        out.insert(
            field.into(),
            if integer {
                xml_world_integer(value)
            } else {
                value
            },
        );
    }

    for child in node["children"].as_array().into_iter().flatten() {
        let Some(tag) = child["tag"].as_str() else {
            continue;
        };
        if tag == "traitlist" {
            let attributes = xml_attributes(child);
            let Some(list_name) = attributes.get("name").and_then(Value::as_str) else {
                continue;
            };
            let Some(field) = world_xml_list_field(kind, list_name) else {
                continue;
            };
            // The legacy readers clear and refill the list for each matching
            // traitlist, so the last repeated list is the effective value.
            out.insert(field.into(), xml_world_trait_list(child, list_name));
        } else if let Some(field) = world_xml_text_field(kind, tag)
            && let Some(value) = xml_child_text(child)
        {
            // The legacy readers assign repeated text tags in sequence; keep
            // the last effective value while the original subtree retains all.
            out.insert(field.into(), value);
        }
    }
    out
}

fn world_xml_attribute(kind: &str, source_name: &str) -> Option<(&'static str, bool)> {
    let field = match (kind, source_name) {
        ("Item", "name") => ("Name", false),
        ("Item", "type") => ("ItemType", false),
        ("Item", "subtype") => ("ItemSubtype", false),
        ("Item", "level") => ("Level", true),
        ("Item", "bonus") => ("Bonus", true),
        ("Item", "damage") => ("DamageType", false),
        ("Item", "amount") => ("DamageAmount", true),
        ("Item", "conceal") => ("Concealability", false),
        ("Item", "lastmodified") => ("LastModified", false),
        ("Location", "name") => ("Name", false),
        ("Location", "type") => ("LocType", false),
        ("Location", "level") => ("Level", true),
        ("Location", "owner") => ("Owner", false),
        ("Location", "access") => ("Access", false),
        ("Location", "securitytraits") => ("SecTraits", true),
        ("Location", "securityretests") => ("SecRetests", true),
        ("Location", "gauntlet") => ("Gauntlet", true),
        ("Location", "affinity") => ("Affinity", false),
        ("Location", "totem") => ("Totem", false),
        ("Location", "lastmodified") => ("LastModified", false),
        ("Rote", "name") => ("Name", false),
        ("Rote", "level") => ("Level", true),
        ("Rote", "duration") => ("Duration", false),
        ("Rote", "lastmodified") => ("LastModified", false),
        _ => return None,
    };
    Some(field)
}

fn world_xml_list_field(kind: &str, list_name: &str) -> Option<&'static str> {
    match (kind, list_name) {
        ("Item", "Tempers") => Some("TemperList"),
        ("Item", "Negatives") => Some("NegativeList"),
        ("Item", "Abilities") => Some("AbilityList"),
        ("Item", "Availability") => Some("Availability"),
        ("Location", "Links") => Some("LinkList"),
        ("Rote", "Spheres") => Some("SphereList"),
        _ => None,
    }
}

fn world_xml_text_field(kind: &str, tag: &str) -> Option<&'static str> {
    match (kind, tag) {
        ("Item", "powers") => Some("Powers"),
        ("Item", "appearance") => Some("Appearance"),
        ("Item", "notes") => Some("Notes"),
        ("Location", "where") => Some("Where"),
        ("Location", "appearance") => Some("Appearance"),
        ("Location", "security") => Some("Security"),
        ("Location", "umbra") => Some("Umbra"),
        ("Location", "notes") => Some("Notes"),
        ("Rote", "description") => Some("Description"),
        ("Rote", "grades") => Some("Grades"),
        _ => None,
    }
}

fn xml_child_text(node: &Value) -> Option<Value> {
    let parts = node["children"].as_array()?;
    parts
        .iter()
        .all(|part| part.get("text").is_some())
        .then(|| {
            json!(
                parts
                    .iter()
                    .filter_map(|part| part["text"].as_str())
                    .collect::<String>()
            )
        })
}

fn xml_world_integer(value: Value) -> Value {
    value
        .as_str()
        .and_then(|raw| raw.trim().parse::<i16>().ok())
        .map(|number| json!(number))
        .unwrap_or(value)
}

fn xml_world_long(value: Value) -> Value {
    value
        .as_str()
        .and_then(|raw| raw.trim().parse::<i32>().ok())
        .map(|number| json!(number))
        .unwrap_or(value)
}

fn xml_world_boolean(value: Option<&Value>, default: bool) -> bool {
    value
        .and_then(Value::as_str)
        .map(|raw| {
            matches!(
                raw.to_ascii_lowercase().as_str(),
                "yes" | "y" | "true" | "t"
            )
        })
        .unwrap_or(default)
}

fn xml_world_trait_list(node: &Value, list_name: &str) -> Value {
    let attributes = xml_attributes(node);
    let mut list = Map::new();
    list.insert("Name".into(), json!(list_name));
    list.insert(
        "Alphabetized".into(),
        json!(
            xml_world_boolean(attributes.get("alphabetized"), true)
                && xml_world_boolean(attributes.get("abc"), true)
        ),
    );
    list.insert(
        "Atomic".into(),
        json!(xml_world_boolean(attributes.get("atomic"), false)),
    );
    list.insert(
        "Negative".into(),
        json!(xml_world_boolean(attributes.get("negative"), false)),
    );
    if let Some(display) = attributes.get("display") {
        list.insert("Display".into(), xml_world_long(display.clone()));
    }

    let traits = node["children"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|child| child["tag"] == "trait")
        .map(|child| {
            let attributes = xml_attributes(child);
            json!({
                "Name": attributes.get("name").cloned().unwrap_or_else(|| json!("")),
                // Legacy trait totals are strings, with a missing value read as "1".
                "Value": attributes.get("val").cloned().unwrap_or_else(|| json!("1")),
                "Note": attributes.get("note").cloned().unwrap_or_else(|| json!("")),
            })
        })
        .collect::<Vec<_>>();
    list.insert("Traits".into(), json!(traits));
    Value::Object(list)
}

fn remove_mapped_world_lists(fields: &mut Map<String, Value>, kind: &str) {
    let Some(value) = fields.get_mut("UnmappedTraitLists") else {
        return;
    };
    let is_mapped = |list: &Value| {
        list["Name"]
            .as_str()
            .is_some_and(|name| world_xml_list_field(kind, name).is_some())
    };
    match value {
        Value::Array(lists) => {
            lists.retain(|list| !is_mapped(list));
            if lists.len() == 1 {
                *value = lists.pop().unwrap();
            }
        }
        Value::Object(_) if is_mapped(value) => {
            fields.remove("UnmappedTraitLists");
            return;
        }
        _ => {}
    }
    if value.as_array().is_some_and(Vec::is_empty) {
        fields.remove("UnmappedTraitLists");
    }
}

fn xml_attributes(node: &Value) -> Map<String, Value> {
    node["attributes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|a| {
            a["name"]
                .as_str()
                .map(|key| (key.into(), a["value"].clone()))
        })
        .collect()
}

fn find_custom_marker(haystack: &str, marker: &str) -> Option<usize> {
    if marker.is_ascii() {
        // UTF-8 continuation and leading bytes cannot match ASCII, so every match
        // starts and ends on a character boundary and is safe to slice below.
        haystack
            .as_bytes()
            .windows(marker.len())
            .position(|candidate| candidate.eq_ignore_ascii_case(marker.as_bytes()))
    } else {
        haystack.find(marker)
    }
}

pub fn redact_custom(value: &Value, start: &str, end: &str) -> Value {
    if start.is_empty() || end.is_empty() {
        return match value {
            Value::String(_) => json!(""),
            Value::Array(a) => {
                Value::Array(a.iter().map(|v| redact_custom(v, start, end)).collect())
            }
            Value::Object(m) => Value::Object(
                m.iter()
                    .map(|(k, v)| (k.clone(), redact_custom(v, start, end)))
                    .collect(),
            ),
            _ => value.clone(),
        };
    }
    match value {
        Value::String(s) => {
            let mut rest = s.as_str();
            let mut out = String::new();
            while let Some(pos) = find_custom_marker(rest, start) {
                out.push_str(&rest[..pos]);
                rest = &rest[pos + start.len()..];
                if let Some(close) = find_custom_marker(rest, end) {
                    rest = &rest[close + end.len()..];
                } else {
                    return json!(out);
                }
            }
            if find_custom_marker(rest, end).is_some() {
                return json!("");
            }
            out.push_str(rest);
            json!(out)
        }
        Value::Array(a) => Value::Array(a.iter().map(|v| redact_custom(v, start, end)).collect()),
        Value::Object(m) => Value::Object(
            m.iter()
                .map(|(k, v)| (k.clone(), redact_custom(v, start, end)))
                .collect(),
        ),
        _ => value.clone(),
    }
}

fn public_field(name: &str) -> bool {
    matches!(
        name,
        "Name"
            | "CharacterType"
            | "RaceCode"
            | "Player"
            | "Status"
            | "Nature"
            | "Demeanor"
            | "Clan"
            | "Sect"
            | "Guild"
            | "Faction"
            | "Tribe"
            | "Kith"
            | "Title"
            | "PhysicalList"
            | "SocialList"
            | "MentalList"
            | "DisciplineList"
            | "ArcanoiList"
            | "GiftList"
            | "ArtList"
            | "RealmList"
            | "SphereList"
            | "EdgeList"
            | "HekauList"
            | "PowerList"
    )
}

/// Conservative redaction: a malformed ST block hides the rest of that string.
pub fn redact(value: &Value) -> Value {
    match value {
        Value::String(s) => {
            let mut rest = s.as_str();
            let mut result = String::new();
            while let Some(start) = rest.to_ascii_lowercase().find("[st]") {
                result.push_str(&rest[..start]);
                rest = &rest[start + 4..];
                if let Some(end) = rest.to_ascii_lowercase().find("[/st]") {
                    rest = &rest[end + 5..];
                } else {
                    return json!(result);
                }
            }
            // A closing marker without an opening marker indicates ambiguous secret text.
            if rest.to_ascii_lowercase().contains("[/st]") {
                return json!("");
            }
            result.push_str(rest);
            json!(result)
        }
        Value::Array(a) => Value::Array(a.iter().map(redact).collect()),
        Value::Object(m) => Value::Object(
            m.iter()
                .filter(|(k, _)| {
                    let ordinary = !matches!(
                        k.to_ascii_lowercase().as_str(),
                        "note" | "notes" | "legacyxml"
                    );
                    ordinary
                        && if m.contains_key("Traits") {
                            matches!(
                                k.as_str(),
                                "Name"
                                    | "Traits"
                                    | "Alphabetized"
                                    | "Atomic"
                                    | "Negative"
                                    | "Display"
                            )
                        } else if m.contains_key("Name") && m.contains_key("Value") {
                            matches!(k.as_str(), "Name" | "Value")
                        } else {
                            true
                        }
                })
                .map(|(k, v)| (k.clone(), redact(v)))
                .collect(),
        ),
        _ => value.clone(),
    }
}

fn xml_resource_value(key: &str, value: Value) -> Value {
    if crate::sheet::RESOURCE_FIELDS.contains(&key)
        && let Some(number) = value.as_str().and_then(|s| s.parse::<i16>().ok())
    {
        json!(number)
    } else {
        value
    }
}
