//! Portable player projection. The nightly state is a separate local scratchpad.
use crate::{
    challenge_math::Breakdown,
    chronicle::PublicProfile,
    house_rules,
    model::{RecordView, Result, error},
    printing::{PrintScope, PublishedReference, Reference, Section, SheetDocument},
    sheet,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub const FORMAT: &str = "grapevine-mobile-sheet-v1";
pub const EXTENSION: &str = ".inconnu-sheet.json";
const MAX_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bundle {
    pub format: String,
    pub character: Character,
    pub chronicle_rules: ChronicleRules,
    pub challenge_pools: Vec<Breakdown>,
    pub simple_mechanics: Vec<Section>,
    pub source_references: Vec<Reference>,
    pub published_source_references: Vec<PublishedReference>,
    pub tonight_play_state: TonightPlayState,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Character {
    pub name: String,
    pub kind: String,
    pub status: String,
    pub player_name: String,
    pub identity: BTreeMap<String, String>,
    pub trait_lists: BTreeMap<String, Vec<Trait>>,
    pub permanent_resources: BTreeMap<String, i64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trait {
    pub name: String,
    pub value: Value,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChronicleRules {
    pub owbn_bylaws_enabled: bool,
    pub allow_non_binding_sources: bool,
    pub house_rules_pack: Option<house_rules::Summary>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TonightPlayState {
    pub temp_physical_lost: i64,
    pub temp_social_lost: i64,
    pub temp_mental_lost: i64,
    pub current_temp_resources: BTreeMap<String, i64>,
    pub current_health_damage: Vec<String>,
}

fn invalid() -> crate::model::AppError {
    error(
        "MOBILE_SHEET_INVALID",
        "A mobile sheet requires a bounded public projection of a saved character.",
    )
}

impl Bundle {
    pub(crate) fn project(
        view: &RecordView,
        document: SheetDocument,
        mechanics: Vec<Section>,
        profile: PublicProfile,
        markers: &[(String, String)],
    ) -> Result<Self> {
        if document.scope != PrintScope::Public || view.build.is_some() {
            return Err(invalid());
        }
        let identity = sheet::IDENTITY_FIELDS
            .iter()
            .filter_map(|key| {
                view.fields
                    .get(*key)
                    .and_then(Value::as_str)
                    .filter(|value| !value.trim().is_empty())
                    .map(|value| ((*key).into(), value.into()))
            })
            .collect();
        let trait_lists = sheet::LIST_FIELDS
            .iter()
            .filter_map(|key| {
                let rows = view.fields.get(*key)?["Traits"].as_array()?;
                let rows: Vec<_> = rows
                    .iter()
                    .filter_map(|row| {
                        let name = row["Name"].as_str()?.trim();
                        let value = &row["Value"];
                        if name.is_empty()
                            || !(value.is_null() || value.is_i64() || value.is_string())
                        {
                            return None;
                        }
                        Some(Trait {
                            name: name.into(),
                            value: value.clone(),
                        })
                    })
                    .collect();
                Some(((*key).into(), rows))
            })
            .collect();
        let (current_temp_resources, permanent_resources) = sheet::RESOURCE_FIELDS
            .iter()
            .filter_map(|key| {
                view.fields
                    .get(*key)?
                    .as_i64()
                    .map(|value| ((*key).to_owned(), value))
            })
            .partition::<BTreeMap<_, _>, _>(|(key, _)| key.starts_with("Temp"));
        let bundle = Self {
            format: FORMAT.into(),
            character: Character {
                name: document.title,
                kind: document.kind,
                status: document.status,
                player_name: document.player_name,
                identity,
                trait_lists,
                permanent_resources,
            },
            chronicle_rules: ChronicleRules {
                owbn_bylaws_enabled: profile.owbn_bylaws_enabled,
                allow_non_binding_sources: profile.allow_non_binding_sources,
                house_rules_pack: profile.house_rules_pack,
            },
            challenge_pools: document.challenge_pools,
            simple_mechanics: mechanics,
            source_references: document.references,
            published_source_references: document.published_references,
            tonight_play_state: TonightPlayState {
                current_temp_resources,
                ..TonightPlayState::default()
            },
        };
        let safe = crate::importer::redact_projection(serde_json::to_value(bundle)?, markers)?;
        let bundle: Self = serde_json::from_value(safe).map_err(|_| invalid())?;
        if bundle.format == FORMAT {
            Ok(bundle)
        } else {
            Err(invalid())
        }
    }

    pub(crate) fn encode(&self) -> Result<Vec<u8>> {
        let bytes = serde_json::to_vec_pretty(self)?;
        if bytes.len() > MAX_BYTES {
            return Err(invalid());
        }
        Ok(bytes)
    }
}
