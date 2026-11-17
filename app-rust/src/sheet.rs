//! Mechanical editing bounds derive from legacy v3 field kinds; these are not rules approval.
use crate::model::*;
use serde_json::{Value, json};
pub const IDENTITY_FIELDS: &[&str] = &[
    "Clan", "Sect", "Nature", "Demeanor", "Tribe", "Kith", "Guild", "Faction", "Title", "Path",
    "Auspice", "Breed", "Rank", "Pack", "Legion", "Cabal", "Essence", "Court", "Seeming", "House",
];
pub const LIST_FIELDS: &[&str] = &[
    "AbilityList",
    "ArcanoiList",
    "ArtList",
    "BackgroundList",
    "BondList",
    "DerangementList",
    "DisciplineList",
    "EdgeList",
    "EquipmentList",
    "FeatureList",
    "FlawList",
    "GiftList",
    "GloryList",
    "GuanxiList",
    "HangoutList",
    "HealthList",
    "HekauList",
    "HonorList",
    "HumanityList",
    "InfluenceList",
    "LoreList",
    "MentalList",
    "MentalNegList",
    "MeritList",
    "MiscellaneousList",
    "NuminaList",
    "PhysicalList",
    "PhysicalNegList",
    "PowerList",
    "RealmList",
    "ReputationList",
    "ResonanceList",
    "RiteList",
    "RitualList",
    "RoteList",
    "SocialList",
    "SocialNegList",
    "SpellList",
    "SphereList",
    "StatusList",
    "TemperList",
    "ThornList",
    "VisageList",
    "WisdomList",
];
pub const RESOURCE_FIELDS: &[&str] = &[
    "Angst",
    "Arete",
    "Ba",
    "Balance",
    "Banality",
    "Blood",
    "Conscience",
    "Conviction",
    "Corpus",
    "Courage",
    "DemonChi",
    "DharmaTraits",
    "Faith",
    "Generation",
    "Glamour",
    "Glory",
    "Gnosis",
    "Honor",
    "Humanity",
    "Hun",
    "Integrity",
    "Joy",
    "Ka",
    "Memory",
    "MentalMax",
    "Mercy",
    "Notoriety",
    "Paradox",
    "PathTraits",
    "Pathos",
    "PhysicalMax",
    "Po",
    "Quintessence",
    "Rage",
    "Sekhem",
    "SelfControl",
    "Instinct",
    "SocialMax",
    "TempAngst",
    "TempArete",
    "TempBa",
    "TempBalance",
    "TempBanality",
    "TempBlood",
    "TempConscience",
    "TempConviction",
    "TempCorpus",
    "TempCourage",
    "TempDemonChi",
    "TempDharmaTraits",
    "TempFaith",
    "TempGlamour",
    "TempGnosis",
    "TempHumanity",
    "TempHun",
    "TempIntegrity",
    "TempJoy",
    "TempKa",
    "TempMemory",
    "TempMercy",
    "TempParadox",
    "TempPathTraits",
    "TempPathos",
    "TempPo",
    "TempQuintessence",
    "TempRage",
    "TempSekhem",
    "TempSelfControl",
    "TempInstinct",
    "TempTorment",
    "TempTrueFaith",
    "TempVision",
    "TempWillpower",
    "TempYangChi",
    "TempYinChi",
    "TempZeal",
    "Torment",
    "TrueFaith",
    "Vision",
    "Willpower",
    "Wisdom",
    "YangChi",
    "YinChi",
    "Zeal",
];

pub fn editable_list(value: &Value) -> bool {
    value.as_object().is_some_and(|v| {
        v.get("Traits")
            .and_then(Value::as_array)
            .is_some_and(|rows| {
                rows.iter().all(|r| {
                    ["Name", "Value", "Note"]
                        .iter()
                        .all(|key| r.get(key).is_none_or(Value::is_string))
                        && r.is_object()
                })
            })
    })
}
pub fn extract(record: &Record) -> SheetEdit {
    let mut fields = record.public_fields.clone();
    fields.extend(record.private_fields.clone());
    let mut lists: std::collections::BTreeMap<String, Value> = fields
        .iter()
        .filter(|(key, value)| LIST_FIELDS.contains(&key.as_str()) && editable_list(value))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    // Earlier XML imports retained common lists under UnmappedTraitLists. Project only
    // unique recognized lists, preserving the original evidence and explicit newer fields.
    if let Some(unmapped) = fields.get("UnmappedTraitLists") {
        let candidates: Vec<&Value> = unmapped
            .as_array()
            .map(|rows| rows.iter().collect())
            .unwrap_or_else(|| vec![unmapped]);
        for list in &candidates {
            if let Some(key) = list["Name"]
                .as_str()
                .and_then(crate::importer::xml_list_key)
                && !fields.contains_key(key)
                && editable_list(list)
                && candidates
                    .iter()
                    .filter(|other| {
                        other["Name"]
                            .as_str()
                            .and_then(crate::importer::xml_list_key)
                            == Some(key)
                    })
                    .count()
                    == 1
            {
                lists.insert(key.into(), (*list).clone());
            }
        }
    }
    let identity = fields
        .iter()
        .filter(|(key, _)| IDENTITY_FIELDS.contains(&key.as_str()))
        .filter_map(|(key, value)| value.as_str().map(|v| (key.clone(), v.to_owned())))
        .collect::<std::collections::BTreeMap<_, _>>();
    let resources = fields
        .iter()
        .filter(|(key, _)| RESOURCE_FIELDS.contains(&key.as_str()))
        .filter_map(|(k, v)| v.as_i64().map(|value| (k.clone(), value)))
        .collect::<std::collections::BTreeMap<_, _>>();
    let visibility = identity
        .keys()
        .chain(lists.keys())
        .chain(resources.keys())
        .map(|key| (key.clone(), record.public_fields.contains_key(key)))
        .collect();
    SheetEdit {
        identity,
        visibility,
        lists,
        resources,
        player_link: PlayerLink::Keep,
    }
}
pub fn validate(edit: &SheetEdit) -> Result<()> {
    if edit.identity.len() > IDENTITY_FIELDS.len()
        || edit
            .identity
            .iter()
            .any(|(key, value)| !IDENTITY_FIELDS.contains(&key.as_str()) || value.len() > 16000)
        || edit.visibility.keys().any(|key| {
            !edit.identity.contains_key(key)
                && !edit.lists.contains_key(key)
                && !edit.resources.contains_key(key)
        })
    {
        return Err(error(
            "INVALID_SHEET_IDENTITY",
            "Choose supported identity fields and explicit sheet sections.",
        ));
    }
    if edit.lists.len() > LIST_FIELDS.len() || edit.resources.len() > RESOURCE_FIELDS.len() {
        return Err(error("INVALID_SHEET", "Too many editable fields."));
    }
    for (key, list) in &edit.lists {
        if !LIST_FIELDS.contains(&key.as_str()) || !editable_list(list) {
            return Err(error(
                "INVALID_TRAIT_LIST",
                "Unsupported trait structure; original data is retained.",
            ));
        }
        let rows = list["Traits"].as_array().unwrap();
        if rows.len() > 4096 || serde_json::to_vec(list)?.len() > 2 * 1024 * 1024 {
            return Err(error("TRAIT_LIMIT", "Trait list exceeds editing limits."));
        }
        if rows.iter().any(|r| {
            ["Name", "Value", "Note"].iter().any(|key| {
                r.get(key)
                    .and_then(Value::as_str)
                    .is_some_and(|s| s.len() > 16000)
            })
        }) {
            return Err(error("TRAIT_LIMIT", "Trait text exceeds editing limits."));
        }
    }
    for (key, value) in &edit.resources {
        if !RESOURCE_FIELDS.contains(&key.as_str()) || i16::try_from(*value).is_err() {
            return Err(error(
                "INVALID_RESOURCE",
                "Choose a supported resource and a value within legacy storage bounds.",
            ));
        }
    }
    Ok(())
}
pub fn empty_list(key: &str) -> Value {
    json!({"Name":key,"Alphabetized":{"value":false,"vb_raw":0},"Atomic":{"value":false,"vb_raw":0},"Negative":{"value":false,"vb_raw":0},"Display":0,"Traits":[]})
}

/// Keep the existing field visibility and redact staff-only content in shared data.
pub(crate) fn write_field(record: &mut Record, key: &str, value: Value) {
    if record.public_fields.contains_key(key) {
        let mut safe = crate::importer::redact(&value);
        if let Some((start, end)) = &record.st_markers {
            safe = crate::importer::redact_custom(&safe, start, end);
        }
        record.public_fields.insert(key.into(), safe.clone());
        if safe != value {
            record.private_fields.insert(key.into(), value);
        } else {
            record.private_fields.remove(key);
        }
    } else {
        record.private_fields.insert(key.into(), value);
    }
}

pub fn is_character_kind(kind: &str) -> bool {
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
            | "Risen"
    )
}
