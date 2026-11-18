//! Versioned local challenge rules and portable custom power definitions.
use crate::{
    model::{Result, error},
    owbn_bylaws::PacketAuthority,
    power_catalogue::{Catalogue, Definition},
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, io::Read, path::Path};

pub const MAX_BYTES: usize = 256 * 1024;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ChallengeFormula {
    pub include_ability_in_trait_call: bool,
    pub include_linked_item_bonuses: bool,
    pub include_merit_flaw_modifiers: bool,
    pub max_trait_bid_cap: Option<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Attribute {
    Physical,
    Social,
    Mental,
}
impl Attribute {
    pub const ALL: [Self; 3] = [Self::Physical, Self::Social, Self::Mental];
    pub const fn label(self) -> &'static str {
        match self {
            Self::Physical => "Physical",
            Self::Social => "Social",
            Self::Mental => "Mental",
        }
    }
    pub const fn list(self) -> &'static str {
        match self {
            Self::Physical => "PhysicalList",
            Self::Social => "SocialList",
            Self::Mental => "MentalList",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModifierTarget {
    Physical,
    Social,
    Mental,
    NamedChallenge(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceKind {
    Merit,
    Flaw,
    Power,
    Item,
    Custom,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChallengeModifierRule {
    pub id: String,
    pub label: String,
    pub source_kind: SourceKind,
    pub match_name: String,
    pub target: ModifierTarget,
    #[serde(default)]
    pub ability_override: Option<String>,
    pub delta: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamedChallenge {
    pub name: String,
    pub attribute: Attribute,
    #[serde(default)]
    pub ability: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HouseRulesPack {
    pub format_version: u32,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub challenge_formula: ChallengeFormula,
    #[serde(default)]
    pub custom_modifiers: Vec<ChallengeModifierRule>,
    #[serde(default)]
    pub custom_powers: Vec<Definition>,
    /// Explicit attribute/ability bindings for the named challenges printed on sheets.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub named_challenges: Vec<NamedChallenge>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Summary {
    pub name: String,
    pub version: String,
    pub challenge_formula: ChallengeFormula,
}

fn invalid() -> crate::model::AppError {
    error(
        "HOUSE_RULES_INVALID",
        "Use a version 1 house rules pack with valid names, modifiers, powers and formula bounds.",
    )
}

fn text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}

impl Default for HouseRulesPack {
    fn default() -> Self {
        Self {
            format_version: 1,
            name: "Local house rules".into(),
            version: "1".into(),
            challenge_formula: ChallengeFormula::default(),
            custom_modifiers: Vec::new(),
            custom_powers: Vec::new(),
            named_challenges: Vec::new(),
        }
    }
}

impl HouseRulesPack {
    pub fn summary(&self) -> Summary {
        Summary {
            name: self.name.clone(),
            version: self.version.clone(),
            challenge_formula: self.challenge_formula.clone(),
        }
    }

    pub(crate) fn for_public_math(&self, markers: &[(String, String)]) -> Result<Self> {
        fn visible(value: impl Serialize, markers: &[(String, String)]) -> Result<bool> {
            let value = serde_json::to_value(value)?;
            Ok(crate::importer::redact_projection(value.clone(), markers)? == value)
        }
        let mut public = Self {
            challenge_formula: self.challenge_formula.clone(),
            ..Self::default()
        };
        for rule in &self.custom_modifiers {
            if visible(rule, markers)? {
                public.custom_modifiers.push(rule.clone());
            }
        }
        for challenge in &self.named_challenges {
            if visible(challenge, markers)? {
                public.named_challenges.push(challenge.clone());
            }
        }
        Ok(public)
    }

    pub fn validate(&self) -> Result<()> {
        let mut ids = BTreeSet::new();
        let mut names = BTreeSet::new();
        if self.format_version != 1
            || !text(&self.name)
            || !text(&self.version)
            || self
                .challenge_formula
                .max_trait_bid_cap
                .is_some_and(|cap| !(1..=4096).contains(&cap))
            || self.custom_modifiers.len() > 256
            || self.custom_powers.len() > 128
            || self.named_challenges.len() > 64
            || self.custom_modifiers.iter().any(|rule| {
                !text(&rule.id)
                    || !ids.insert(normalize(&rule.id))
                    || !text(&rule.label)
                    || !text(&rule.match_name)
                    || rule
                        .ability_override
                        .as_deref()
                        .is_some_and(|name| !text(name))
                    || !(-4096..=4096).contains(&rule.delta)
                    || matches!(&rule.target, ModifierTarget::NamedChallenge(name) if !text(name))
            })
            || self.named_challenges.iter().any(|challenge| {
                !text(&challenge.name)
                    || !names.insert(normalize(&challenge.name))
                    || challenge.ability.as_deref().is_some_and(|name| !text(name))
            })
            || self.custom_powers.iter().any(|power| {
                power.packet_authority != PacketAuthority::CustomHouseRule
                    || power.legacy_power.is_some()
                    || power.legacy_preview.is_some()
                    || crate::power_catalogue::builtin()
                        .lookup(&power.stable_id)
                        .is_some()
            })
        {
            return Err(invalid());
        }
        if !self.custom_powers.is_empty() {
            let catalog = serde_json::json!({"version": 1, "entries": self.custom_powers});
            Catalogue::from_json(&serde_json::to_string(&catalog)?).map_err(|_| invalid())?;
        }
        if serde_json::to_vec_pretty(self)?.len() > MAX_BYTES {
            return Err(invalid());
        }
        Ok(())
    }

    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_BYTES {
            return Err(invalid());
        }
        let pack: Self = serde_json::from_slice(bytes).map_err(|_| invalid())?;
        pack.validate()?;
        Ok(pack)
    }

    pub fn load(path: &Path) -> Result<Self> {
        let mut bytes = Vec::new();
        std::fs::File::open(path)?
            .take((MAX_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        Self::from_json(&bytes)
    }

    pub fn save_new(&self, path: &Path) -> Result<()> {
        self.validate()?;
        crate::safe_files::write_new(path, &serde_json::to_vec_pretty(self)?)
    }
}

pub(crate) fn normalize(value: &str) -> String {
    value.trim().to_lowercase()
}
