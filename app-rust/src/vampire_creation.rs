//! Initial MET creation is independent of the earned/spent XP ledger.
use crate::{
    creation_budget as budget, creation_generation as generation, creature_profiles, model::*,
    purchases, sheet,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
pub const FIELD: &str = "_AppVampireCreationReviewsV1";
pub const DRAFT_FIELD: &str = "_AppVampireCreationDraftV1";
pub const MANAGED_LISTS: &[&str] = &[
    "PhysicalList",
    "SocialList",
    "MentalList",
    "AbilityList",
    "BackgroundList",
    "DisciplineList",
    "PhysicalNegList",
    "SocialNegList",
    "MentalNegList",
    "MeritList",
    "FlawList",
];
const SHARED_PROFILE_LISTS: &[&str] = &[
    "PhysicalList",
    "SocialList",
    "MentalList",
    "AbilityList",
    "BackgroundList",
    "PhysicalNegList",
    "SocialNegList",
    "MentalNegList",
    "MeritList",
    "FlawList",
];
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfiguredBudgetQuote {
    pub attribute_bases: [i64; 3],
    pub attribute_remaining: [i64; 3],
    pub ability_base: i64,
    pub ability_remaining: i64,
    pub background_base: i64,
    pub background_remaining: i64,
    pub primary_power_base: i64,
    pub primary_power_remaining: i64,
    pub secondary_power_count: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secondary_power_remaining: Option<i64>,
    pub negative_credit: i64,
    pub flaw_credit: i64,
    pub free_available: i64,
    pub free_spent: Option<i64>,
    pub free_remaining: Option<i64>,
    pub complete: bool,
    pub issues: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SeemingBonus {
    Glamour,
    Willpower,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wilder_bonus: Option<SeemingBonus>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub shadow_review: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risen_start: Option<RisenStart>,
    #[serde(default, skip_serializing_if = "crate::vampire_creation::zero_u8")]
    pub mage_arete_free: u8,
    pub id: String,
    pub sheet: SheetEdit,
    pub budget: budget::Input,
    pub actor: String,
    pub day: String,
    pub reason: String,
    pub policy_version: String,
    pub review_reference: String,
    pub review_confirmed: bool,
    pub standard_pools: bool,
    pub current_resources_reference: String,
    pub manual_power_reference: String,
    #[serde(
        default,
        skip_serializing_if = "crate::owbn_bylaws::Evidence::is_empty"
    )]
    pub owbn_evidence: crate::owbn_bylaws::Evidence,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub create_as_npc: bool,
    pub specializations: Vec<String>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RisenStart {
    pub from_wraith: bool,
    pub potency: u8,
    pub review_confirmed: bool,
}

fn zero_u8(value: &u8) -> bool {
    *value == 0
}

#[derive(Clone, Debug)]
pub struct Context {
    pub revision: i64,
    pub record_id: String,
    pub record_name: String,
    pub request: Request,
    pub history: Vec<Receipt>,
    pub editable: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Quote {
    pub library_id: String,
    pub record_id: String,
    pub revision: i64,
    pub record_hash: String,
    pub request: Request,
    pub effective_sheet: SheetEdit,
    pub budget: budget::Quote,
    pub generation: generation::GenerationLimits,
    pub issues: Vec<String>,
    pub warnings: Vec<String>,
    pub citations: Vec<crate::pricing::SourceCitation>,
    #[serde(default)]
    pub published_sources: Vec<crate::published_reference::Quote>,
    #[serde(default)]
    pub met_mappings: Vec<crate::creation_sources::MetMapping>,
    pub ready: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configured_profile: Option<creature_profiles::Profile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configured_budget: Option<ConfiguredBudgetQuote>,
}
impl Quote {
    pub fn is_configured(&self) -> bool {
        self.configured_profile.is_some()
    }
}

pub fn managed_lists(kind: &str) -> Vec<String> {
    if kind == "Vampire" {
        return MANAGED_LISTS.iter().map(|key| (*key).into()).collect();
    }
    let Some(profile) = creature_profiles::profile(kind) else {
        return Vec::new();
    };
    profile_managed_lists(profile)
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Receipt {
    pub quote: Quote,
    pub committed_revision: i64,
    pub managed_signature: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Draft {
    pub library_id: String,
    pub record_id: String,
    pub base_hash: String,
    pub request: Request,
}
pub fn scope(record: &Record) -> Result<()> {
    if !sheet::is_character_kind(&record.kind)
        || record.build.is_some()
        || record.legacy_id.is_some()
        || !record.source_sha256.is_empty()
        || record.original["created_in_app"] != true
    {
        return Err(error(
            "CREATION_SCOPE",
            "Creation applies only to a new native character, not an imported character or build copy.",
        ));
    }
    Ok(())
}
pub fn editable(record: &Record) -> bool {
    record.xp_ledger.is_none()
        && record
            .private_fields
            .get("Status")
            .or_else(|| record.public_fields.get("Status"))
            .and_then(Value::as_str)
            == Some("Draft")
}
pub fn initial_request(record: &Record) -> Request {
    let mut sheet = sheet::extract(record);
    let managed = managed_lists(&record.kind);
    for key in managed {
        if !sheet.lists.contains_key(&key) {
            sheet.lists.insert(key.clone(), sheet::empty_list(&key));
            sheet.visibility.insert(key, true);
        }
    }
    if record.kind == "Vampire" {
        if !sheet.identity.contains_key("Path") {
            sheet.identity.insert("Path".into(), "Humanity".into());
            sheet.visibility.insert("Path".into(), true);
        }
    } else if let Some(profile) = creature_profiles::profile(&record.kind) {
        for key in &profile.identity_fields {
            if !sheet.identity.contains_key(key) {
                sheet.identity.insert(key.clone(), String::new());
                sheet.visibility.insert(key.clone(), true);
            }
        }
    }
    Request {
        id: uuid::Uuid::new_v4().to_string(),
        sheet,
        budget: budget::Input::default(),
        actor: String::new(),
        day: String::new(),
        reason: String::new(),
        policy_version: String::new(),
        review_reference: String::new(),
        review_confirmed: false,
        standard_pools: false,
        current_resources_reference: String::new(),
        manual_power_reference: String::new(),
        wilder_bonus: None,
        shadow_review: false,
        risen_start: None,
        mage_arete_free: 0,
        owbn_evidence: Default::default(),
        create_as_npc: false,
        specializations: vec![],
    }
}
pub fn validate_request(request: &Request) -> Result<()> {
    request.owbn_evidence.validate()?;
    if uuid::Uuid::parse_str(&request.id)
        .ok()
        .map(|id| id.to_string())
        .as_deref()
        != Some(&request.id)
        || serde_json::to_vec(request)?.len() > 4 * 1024 * 1024
        || [
            &request.actor,
            &request.day,
            &request.reason,
            &request.policy_version,
            &request.review_reference,
            &request.current_resources_reference,
            &request.manual_power_reference,
        ]
        .iter()
        .any(|v| v.len() > 16000)
        || request.specializations.len() > 4096
        || request.specializations.iter().any(|v| v.len() > 1000)
    {
        return Err(error(
            "CREATION_INPUT",
            "Creation input exceeds its limits or has an invalid review identity.",
        ));
    }
    sheet::validate(&request.sheet)
}
pub fn history(record: &Record) -> Result<Vec<Receipt>> {
    if record.public_fields.contains_key(FIELD) || record.public_fields.contains_key(DRAFT_FIELD) {
        return Err(error(
            "CREATION_PRIVACY",
            "Creation review and unfinished drafts must remain private.",
        ));
    }
    let Some(value) = record.private_fields.get(FIELD) else {
        return Ok(vec![]);
    };
    if serde_json::to_vec(value)?.len() > 8 * 1024 * 1024 {
        return Err(error(
            "CREATION_LIMIT",
            "Creation history exceeds its supported limit.",
        ));
    }
    let receipts: Vec<Receipt> = serde_json::from_value(value.clone())?;
    for receipt in &receipts {
        match (
            &receipt.quote.configured_profile,
            &receipt.quote.configured_budget,
        ) {
            (Some(profile), Some(_))
                if profile.kind == record.kind && profile.kind != "Vampire" =>
            {
                profile.validate_snapshot()?;
            }
            (None, None) => {}
            _ => {
                return Err(error(
                    "CREATION_HISTORY",
                    "Configured creation snapshot does not match its creature context.",
                ));
            }
        }
    }

    let mut ids = std::collections::HashSet::new();
    if receipts.len() > 64
        || receipts.iter().any(|r| {
            r.quote.record_id != record.id
                || !ids.insert(&r.quote.request.id)
                || r.quote.revision.checked_add(1) != Some(r.committed_revision)
        })
    {
        return Err(error(
            "CREATION_HISTORY",
            "Creation history identities or revisions are inconsistent.",
        ));
    }
    Ok(receipts)
}
pub fn base_hash(record: &Record) -> Result<String> {
    let mut clean = record.clone();
    clean.private_fields.remove(FIELD);
    clean.private_fields.remove(DRAFT_FIELD);
    purchases::record_hash(&clean)
}
pub fn draft(record: &Record) -> Result<Option<Draft>> {
    history(record)?;
    record
        .private_fields
        .get(DRAFT_FIELD)
        .map(|value| {
            if serde_json::to_vec(value)?.len() > 4 * 1024 * 1024 + 4096 {
                return Err(error(
                    "CREATION_LIMIT",
                    "Saved creation draft exceeds its supported limit.",
                ));
            }
            let draft: Draft = serde_json::from_value(value.clone())?;
            validate_request(&draft.request)?;
            if draft.record_id != record.id {
                return Err(error(
                    "CREATION_DRAFT_ID",
                    "Saved creation draft belongs to another character.",
                ));
            }
            Ok(draft)
        })
        .transpose()
}
fn rows<'a>(edit: &'a SheetEdit, key: &str) -> Result<&'a [Value]> {
    edit.lists
        .get(key)
        .and_then(|v| v.get("Traits"))
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| {
            error(
                "CREATION_LIST",
                format!("Restore the {key} list before reviewing creation."),
            )
        })
}
fn numeric_total_with_generation_zero(
    edit: &SheetEdit,
    key: &str,
    allow_generation_zero: bool,
) -> Result<i64> {
    let mut total = 0;
    let mut names = std::collections::HashSet::new();
    for row in rows(edit, key)? {
        let name = row["Name"].as_str().unwrap_or("").trim();
        let value = row["Value"]
            .as_str()
            .unwrap_or("")
            .parse::<i64>()
            .map_err(|_| {
                error(
                    "CREATION_TRAIT_VALUE",
                    format!("{key}: use a numeric MET Trait count or listed point value."),
                )
            })?;
        let minimum = if allow_generation_zero
            && key == "BackgroundList"
            && name.eq_ignore_ascii_case("Generation")
        {
            0
        } else {
            1
        };
        if name.is_empty()
            || !(minimum..=4096).contains(&value)
            || row.get("AppPurchaseId").is_some()
            || row.get("AppTraitAdvancementId").is_some()
            || row.get("_AppPurchaseId").is_some()
        {
            return Err(error(
                "CREATION_TRAIT_VALUE",
                format!(
                    "{key}: use named positive initial Traits without an XP purchase identity."
                ),
            ));
        }
        if !key.starts_with("Physical")
            && !key.starts_with("Social")
            && !key.starts_with("Mental")
            && !names.insert(name.to_lowercase())
        {
            return Err(error(
                "CREATION_DUPLICATE",
                format!("{key}: combine duplicate named entries before review."),
            ));
        }
        total += value;
    }
    Ok(total)
}
fn numeric_total(edit: &SheetEdit, key: &str) -> Result<i64> {
    numeric_total_with_generation_zero(edit, key, true)
}
fn configured_numeric_total(edit: &SheetEdit, key: &str) -> Result<i64> {
    numeric_total_with_generation_zero(edit, key, false)
}
pub fn signature(edit: &SheetEdit) -> Result<String> {
    let lists = MANAGED_LISTS
        .iter()
        .map(|key| {
            Ok((
                key,
                rows(edit, key)?
                    .iter()
                    .map(|r| json!([r.get("Name"), r.get("Value")]))
                    .collect::<Vec<_>>(),
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let resources = [
        "Generation",
        "Blood",
        "Willpower",
        "Conscience",
        "Conviction",
        "SelfControl",
        "Instinct",
        "Courage",
        "PathTraits",
        "TempBlood",
        "TempWillpower",
    ]
    .map(|key| (key, edit.resources.get(key)));
    use sha2::{Digest, Sha256};
    Ok(
        Sha256::digest(serde_json::to_vec(&(lists, resources, &edit.identity))?)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
    )
}
fn profile_managed_lists(profile: &creature_profiles::Profile) -> Vec<String> {
    let mut fields = SHARED_PROFILE_LISTS
        .iter()
        .map(|key| (*key).to_owned())
        .collect::<Vec<_>>();
    for key in [&profile.primary_power_list, &profile.secondary_power_list] {
        if !fields.contains(key) {
            fields.push(key.clone());
        }
    }
    fields
}
fn signature_for_profile(profile: &creature_profiles::Profile, edit: &SheetEdit) -> Result<String> {
    // Configured creation can apply the whole sheet; bind every applied field.
    // The legacy Vampire signature remains a separate, unchanged compatibility path.
    use sha2::{Digest, Sha256};
    Ok(Sha256::digest(serde_json::to_vec(&(&profile.kind, edit))?)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

pub fn signature_for(kind: &str, edit: &SheetEdit) -> Result<String> {
    if kind == "Vampire" {
        return signature(edit);
    }
    let profile = creature_profiles::profile(kind).ok_or_else(|| {
        error(
            "CREATION_PROFILE",
            "No configured creation profile is available.",
        )
    })?;
    signature_for_profile(profile, edit)
}
pub fn signature_for_quote(quote: &Quote, edit: &SheetEdit) -> Result<String> {
    if let Some(profile) = &quote.configured_profile {
        signature_for_profile(profile, edit)
    } else {
        signature(edit)
    }
}
pub fn ensure_opening_allowed(record: &Record) -> Result<()> {
    if record.private_fields.contains_key(DRAFT_FIELD) {
        return Err(error(
            "CREATION_UNFINISHED",
            "Finish or explicitly discard the saved creation draft before opening XP.",
        ));
    }
    if let Some(receipt) = history(record)?.last()
        && receipt.managed_signature
            != signature_for_quote(&receipt.quote, &sheet::extract(record))?
    {
        return Err(error(
            "CREATION_REVIEW_NEEDED",
            "Initial traits changed after creation review; review creation again before opening XP.",
        ));
    }
    Ok(())
}

fn inapplicable_budget() -> budget::Quote {
    budget::Quote {
        base_remaining: [0; 7],
        free_available: 0,
        free_spent: 0,
        free_remaining: 0,
        virtues: [0; 3],
        morality: 0,
        complete: false,
        issues: vec![
            "Vampire-specific creation budget fields do not apply to this profile.".into(),
        ],
        warnings: Vec::new(),
    }
}

fn inapplicable_generation() -> generation::GenerationLimits {
    generation::GenerationLimits {
        generation: 0,
        attribute_max: 0,
        ability_max: 0,
        blood_capacity: 0,
        blood_per_turn: 0,
        starting_willpower: 0,
        willpower_max: 0,
    }
}

pub fn uses_separate_shadow(profile: &creature_profiles::Profile) -> bool {
    profile.kind == "Wraith"
        && profile
            .cost_source
            .as_ref()
            .is_some_and(|source| source.book == "Oblivion")
}

fn garou_starting_resources(edit: &SheetEdit) -> Option<[i64; 3]> {
    let identity = |key| {
        edit.identity
            .get(key)
            .map(|value| value.trim().to_ascii_lowercase())
    };
    let gnosis = match identity("Breed")?.as_str() {
        "homid" => 1,
        "metis" => 3,
        "lupus" => 5,
        _ => return None,
    };
    let rage = match identity("Auspice")?.as_str() {
        "ragabash" => 1,
        "theurge" => 2,
        "philodox" => 3,
        "galliard" => 4,
        "ahroun" => 5,
        _ => return None,
    };
    let willpower = match identity("Tribe")?.as_str() {
        "bone gnawers" | "children of gaia" | "wendigo" => 4,
        "black furies" | "fianna" | "get of fenris" | "glass walkers" | "red talons"
        | "shadow lords" | "silent striders" | "silver fangs" | "uktena" => 3,
        _ => return None,
    };
    Some([gnosis, rage, willpower])
}

pub fn uses_c20_changeling_creation(profile: &creature_profiles::Profile) -> bool {
    profile.kind == "Changeling" && profile.secondary_power_base.is_some()
}

fn check_c20_starting_powers(
    edit: &SheetEdit,
    profile: &creature_profiles::Profile,
    issues: &mut Vec<String>,
) -> Result<()> {
    for (list, kind) in [
        (
            &profile.primary_power_list,
            crate::published_powers::Kind::Art,
        ),
        (
            &profile.secondary_power_list,
            crate::published_powers::Kind::Realm,
        ),
    ] {
        let mut ranks = std::collections::BTreeMap::<&str, Vec<u8>>::new();
        for row in rows(edit, list)? {
            let name = row["Name"].as_str().unwrap_or_default().trim();
            let matches: Vec<_> = crate::published_powers::catalogue()?
                .entries
                .iter()
                .filter(|power| {
                    power.book_id == "phase7a-c20-packet-changeling"
                        && power.kind == kind
                        && power.name.eq_ignore_ascii_case(name)
                })
                .collect();
            let [power] = matches.as_slice() else {
                issues.push(format!(
                    "{list}: choose an unambiguous published rank for {name}."
                ));
                continue;
            };
            if kind == crate::published_powers::Kind::Art {
                let tier = crate::owbn_bylaws::TierMappingBasis::convert_20th_level(power.level);
                if power.level > 3 {
                    issues
                        .push("Art ranks above 3 are unavailable in standard C20 creation.".into());
                }
                if row["Value"].as_str().map(str::trim).is_none_or(|value| {
                    tier.is_none_or(|tier| !tier.sheet_value().eq_ignore_ascii_case(value))
                }) {
                    issues.push(format!(
                        "{list}: {name} must use its source-confirmed tier."
                    ));
                }
            } else if row["Value"]
                .as_str()
                .map(str::trim)
                .and_then(|value| value.parse::<u8>().ok())
                != Some(power.level)
            {
                issues.push(format!("{list}: {name} must use its numeric Realm rank."));
            }
            ranks
                .entry(&power.discipline)
                .or_default()
                .push(power.level);
        }
        for (family, mut levels) in ranks {
            levels.sort_unstable();
            if levels.len() > 5 || !levels.iter().copied().eq(1..=levels.len() as u8) {
                issues.push(format!("{list}: record preceding ranks in {family}."));
            }
        }
    }
    Ok(())
}

fn configured_power_count(edit: &SheetEdit, key: &str, issues: &mut Vec<String>) -> Result<i64> {
    let mut names = std::collections::HashSet::new();
    let values = rows(edit, key)?;
    for row in values {
        let name = row["Name"].as_str().unwrap_or("").trim();
        if name.is_empty()
            || row["Value"]
                .as_str()
                .is_none_or(|value| value.trim().is_empty())
            || !names.insert(name.to_lowercase())
            || row.get("AppPurchaseId").is_some()
            || row.get("AppTraitAdvancementId").is_some()
            || row.get("_AppPurchaseId").is_some()
        {
            issues.push(format!(
                "{key}: use distinct named starting powers without XP purchase identities."
            ));
            break;
        }
    }
    Ok(values.len() as i64)
}

fn configured_budget_quote(
    profile: &creature_profiles::Profile,
    input: &budget::Input,
    secondary_power_count: i64,
    additional_free_spent: i64,
) -> ConfiguredBudgetQuote {
    let mut issues = Vec::new();
    let mut priorities = input.priorities;
    priorities.sort_unstable();
    let priorities_valid = priorities == [0, 1, 2];
    if !priorities_valid {
        issues.push("Choose each configured Attribute priority exactly once.".into());
    }
    let attribute_bases = std::array::from_fn(|index| {
        let priority = if priorities_valid {
            input.priorities[index]
        } else {
            index
        };
        profile.attribute_priorities[priority]
    });
    let attribute_remaining =
        std::array::from_fn(|index| attribute_bases[index] - input.attribute_totals[index]);
    let ability_remaining = profile.ability_base - input.ability_total;
    let background_remaining = profile.background_base - input.background_total;
    let primary_power_remaining = profile.basic_power_base - input.basic_power_count;
    let secondary_power_remaining = profile
        .secondary_power_base
        .map(|base| base - secondary_power_count);
    if let Some(remaining) = secondary_power_remaining.filter(|remaining| *remaining > 0) {
        issues.push(format!(
            "Secondary powers are short by {remaining} configured Trait(s)."
        ));
    }
    let negative_credit = input.negative_traits.iter().sum::<i64>();
    let bounded_negative_credit = negative_credit.min(profile.max_negative_credit);
    let bounded_flaw_credit = input.flaw_credit.min(profile.max_flaw_credit);
    let free_available = profile.free_traits_base + bounded_negative_credit + bounded_flaw_credit;

    for (name, total) in [
        ("Physical Attributes", attribute_remaining[0]),
        ("Social Attributes", attribute_remaining[1]),
        ("Mental Attributes", attribute_remaining[2]),
        ("Abilities", ability_remaining),
        ("Backgrounds", background_remaining),
        ("Primary powers", primary_power_remaining),
    ] {
        if total > 0 {
            issues.push(format!("{name} are short by {total} configured Trait(s)."));
        }
    }
    if negative_credit > profile.max_negative_credit {
        issues.push(format!(
            "Negative Trait credit exceeds this profile's configured maximum of {}.",
            profile.max_negative_credit
        ));
    }
    if input.flaw_credit > profile.max_flaw_credit {
        issues.push(format!(
            "Flaw credit exceeds this profile's configured maximum of {}.",
            profile.max_flaw_credit
        ));
    }
    let allocations = [
        attribute_remaining.iter().map(|n| (-n).max(0)).sum(),
        (-ability_remaining).max(0),
        (-background_remaining).max(0),
        (-primary_power_remaining).max(0),
        if uses_separate_shadow(profile) {
            0
        } else {
            (-secondary_power_remaining.unwrap_or(-secondary_power_count)).max(0)
        },
        input.specialization_count,
        input.willpower_free,
    ];
    let free_spent = if let Some(costs) = &profile.free_trait_costs {
        let prices = [
            costs.attribute,
            costs.ability,
            costs.background,
            costs.primary_power,
            costs.secondary_power,
            costs.specialization,
            costs.permanent_resource,
        ];
        let spent = allocations.into_iter().zip(prices).try_fold(
            input.merit_cost + additional_free_spent,
            |sum, (count, price)| sum.checked_add(count.checked_mul(price)?),
        );
        if spent.is_none() {
            issues.push("Configured Free Trait arithmetic exceeds supported bounds.".into());
        }
        spent
    } else if allocations.iter().any(|count| *count != 0) {
        issues.push("Allocations above configured base pools need a reviewed creation-cost rule before they can be approved.".into());
        None
    } else {
        Some(input.merit_cost)
    };
    if profile.kind == "Various" && profile.free_trait_costs.is_none() {
        issues.push("Various requires an explicit local Free Trait configuration.".into());
    }
    let free_remaining = free_spent.and_then(|spent| free_available.checked_sub(spent));
    if free_spent.is_some() && free_remaining.is_none() {
        issues.push("Configured Free Trait arithmetic exceeds supported bounds.".into());
    }
    if free_remaining.is_some_and(|remaining| remaining < 0) {
        issues.push("Allocations exceed the configured available Free Trait pool.".into());
    }
    ConfiguredBudgetQuote {
        attribute_bases,
        attribute_remaining,
        ability_base: profile.ability_base,
        ability_remaining,
        background_base: profile.background_base,
        background_remaining,
        primary_power_base: profile.basic_power_base,
        primary_power_remaining,
        secondary_power_count,
        secondary_power_remaining,
        negative_credit,
        flaw_credit: input.flaw_credit,
        free_available,
        free_spent,
        free_remaining,
        complete: issues.is_empty(),
        issues,
        warnings: Vec::new(),
    }
}

fn quote_configured(
    library_id: &str,
    revision: i64,
    record: &Record,
    request: &Request,
) -> Result<Quote> {
    let profile = creature_profiles::profile(&record.kind).ok_or_else(|| {
        error(
            "CREATION_PROFILE",
            "No configured creation profile is available.",
        )
    })?;
    quote_configured_profile(library_id, revision, record, request, profile)
}
/// Pure reviewed-profile seam. Store commits still re-quote against the bundled profile.
pub fn quote_configured_profile(
    library_id: &str,
    revision: i64,
    record: &Record,
    request: &Request,
    profile: &creature_profiles::Profile,
) -> Result<Quote> {
    if !(0..=4096).contains(&request.budget.willpower_free) {
        return Err(error(
            "INVALID_CREATION_INPUT",
            "Additional starting Willpower must be between 0 and 4096.",
        ));
    }
    profile.validate_snapshot()?;
    if profile.kind != record.kind || profile.kind == "Vampire" {
        return Err(error(
            "CREATION_PROFILE",
            "Configured creation requires a matching non-Vampire profile.",
        ));
    }
    scope(record)?;
    validate_request(request)?;
    history(record)?;
    if !editable(record) {
        return Err(error(
            "CREATION_CLOSED",
            "Initial creation is closed after XP opening or after leaving Draft status.",
        ));
    }
    if let Some(saved) = draft(record)?
        && (saved.library_id != library_id || saved.base_hash != base_hash(record)?)
    {
        return Err(error(
            "CREATION_DRAFT_STALE",
            "The character changed after this draft was saved. Preserve your notes, then explicitly discard and review from the current sheet.",
        ));
    }
    let mut input = request.budget.clone();
    for (index, key) in ["PhysicalList", "SocialList", "MentalList"]
        .iter()
        .enumerate()
    {
        input.attribute_totals[index] = configured_numeric_total(&request.sheet, key)?;
    }
    for (index, key) in ["PhysicalNegList", "SocialNegList", "MentalNegList"]
        .iter()
        .enumerate()
    {
        input.negative_traits[index] = configured_numeric_total(&request.sheet, key)?;
    }
    input.ability_total = configured_numeric_total(&request.sheet, "AbilityList")?;
    input.background_total = configured_numeric_total(&request.sheet, "BackgroundList")?;
    input.merit_cost = configured_numeric_total(&request.sheet, "MeritList")?;
    input.flaw_credit = configured_numeric_total(&request.sheet, "FlawList")?;
    let mut issues = Vec::new();
    let mut warnings = Vec::new();
    if profile.starting_resources.is_empty() {
        warnings.push(
            "No starting resources are configured for this profile; existing values are retained."
                .into(),
        );
    }
    if !request.standard_pools {
        issues.push("Explicitly adopt this reviewed creation profile before applying.".into());
    }
    if !profile.rules_reviewed {
        issues.push(format!(
            "{} creation rules are not reviewed: {}",
            profile.kind, profile.rules_note
        ));
        warnings.push(profile.rules_note.clone());
    }
    if profile.cost_source.is_none() {
        issues.push(
            "A reviewed source citation is required before this profile's pools can be approved."
                .into(),
        );
    }
    if !request.review_confirmed
        || [
            &request.actor,
            &request.reason,
            &request.policy_version,
            &request.review_reference,
        ]
        .iter()
        .any(|value| value.trim().is_empty())
        || !crate::xp::day_valid(&request.day)
    {
        issues.push(
            "Record reviewer, valid date, reason, local policy version and eligibility review, then confirm that review.".into(),
        );
    }
    if request.manual_power_reference.trim().is_empty() {
        issues.push(
            "Record a local source and staff review reference for initial power assignments."
                .into(),
        );
    }
    if request.budget.sabbat
        || request.budget.optional_derangement
        || request.budget.lower_morality
        || request.budget.virtue_allocated != [0; 3]
        || request.budget.virtue_free != [0; 3]
        || request.budget.conviction
        || request.budget.instinct
        || request.budget.morality_free != 0
    {
        issues.push(
            "Sabbat, Virtue, Derangement, Morality and Vampire-only free-pool options cannot be used for this profile.".into(),
        );
    }
    let original = sheet::extract(record);
    if sheet::RESOURCE_FIELDS
        .iter()
        .any(|field| request.sheet.visibility.get(*field) != original.visibility.get(*field))
    {
        issues.push("Review resource sharing in the main editor; creation cannot change resource visibility.".into());
    }
    let mut effective_sheet = request.sheet.clone();
    let shadow = uses_separate_shadow(profile);
    let risen = profile.kind == "Risen"
        && profile
            .cost_source
            .as_ref()
            .is_some_and(|source| source.edition == "OWbN_Risen_2019");
    let risen_start = risen.then_some(request.risen_start.as_ref()).flatten();
    let source_edition = profile
        .cost_source
        .as_ref()
        .map(|source| source.edition.as_str());
    let mage = profile.kind == "Mage" && source_edition == Some("MET_Laws_of_Ascension");
    let garou =
        profile.kind == "Werewolf" && source_edition == Some("MET_Laws_of_the_Wild_Revised");
    let mortal = profile.kind == "Mortal" && source_edition == Some("MET_Laws_of_the_Hunt_Revised");
    if risen {
        if risen_start.is_none_or(|start| !start.review_confirmed || start.potency > 5)
            || request.current_resources_reference.trim().is_empty()
        {
            issues.push("Confirm the Risen origin, Potency and reviewed starting resources with a Storyteller reference.".into());
        }
        if input.willpower_free != 0 || !request.specializations.is_empty() {
            issues.push("The Risen template requires separate review for optional Willpower and specializations.".into());
        }
    }
    let shadow_evidence =
        request.shadow_review && !request.current_resources_reference.trim().is_empty();
    if shadow && (input.willpower_free != 0 || !request.specializations.is_empty()) {
        issues.push(
            "Additional starting Willpower and specializations need a separate reviewed cost."
                .into(),
        );
    }
    if shadow
        && (!rows(&request.sheet, "ThornList")?.is_empty()
            || request.sheet.resources.contains_key("Angst"))
        && !shadow_evidence
    {
        issues
            .push("Confirm the separate Shadow allocation and record its review reference.".into());
    }
    if shadow
        && request
            .sheet
            .resources
            .get("Angst")
            .is_some_and(|angst| !(0..=10).contains(angst))
    {
        issues.push("Starting Shadow Angst must be between 0 and 10.".into());
    }
    for (key, value) in &request.sheet.resources {
        if shadow && key == "Angst" || risen && matches!(key.as_str(), "Pathos" | "Corpus") {
            continue;
        }
        if original.resources.get(key) != Some(value)
            && profile.starting_resources.get(key) != Some(value)
        {
            issues.push(format!(
                "{key}: this resource edit is not covered by the configured starting-resource rule."
            ));
        }
    }
    for (key, value) in &profile.starting_resources {
        effective_sheet.resources.insert(key.clone(), *value);
        effective_sheet
            .visibility
            .entry(key.clone())
            .or_insert(true);
    }
    if garou {
        if let Some([gnosis, rage, willpower]) = garou_starting_resources(&effective_sheet) {
            for (key, value) in [("Gnosis", gnosis), ("Rage", rage), ("Willpower", willpower)] {
                effective_sheet.resources.insert(key.into(), value);
            }
        } else {
            issues.push("Choose a Breed, Auspice and Tribe supported by the Revised Garou starting-resource tables.".into());
        }
    }
    if let Some(start) = risen_start {
        let defaults = &profile.starting_resources;
        let Some((&base_pathos, &base_corpus)) = defaults.get("Pathos").zip(defaults.get("Corpus"))
        else {
            return Err(error(
                "CREATION_PROFILE",
                "Risen needs reviewed Pathos and Corpus baselines.",
            ));
        };
        let pathos = request
            .sheet
            .resources
            .get("Pathos")
            .copied()
            .unwrap_or(base_pathos);
        let corpus = request
            .sheet
            .resources
            .get("Corpus")
            .copied()
            .unwrap_or(base_corpus);
        if !(0..=10 + i64::from(start.potency)).contains(&pathos) {
            issues.push("Starting Pathos exceeds the reviewed limit for this Potency.".into());
        }
        if start.from_wraith
            && (!request.sheet.resources.contains_key("Corpus") || !(1..=4096).contains(&corpus))
            || !start.from_wraith && corpus != base_corpus
        {
            issues.push(
                "Enter the inherited Wraith Corpus, or use the standalone Risen Corpus baseline."
                    .into(),
            );
        }
        effective_sheet.resources.insert("Pathos".into(), pathos);
        effective_sheet.resources.insert("Corpus".into(), corpus);
        if start.from_wraith {
            warnings.push("Inherited traits are transcribed from the reviewed Wraith sheet. This creation does not change that source character or transfer its XP ledger.".into());
        }
    }
    if uses_c20_changeling_creation(profile) {
        if input.willpower_free != 0 || !request.specializations.is_empty() {
            issues.push(
                "This profile does not price optional starting Willpower or specializations."
                    .into(),
            );
        }
        let choice = match effective_sheet
            .identity
            .get("Seeming")
            .map(|value| value.trim().to_ascii_lowercase())
            .as_deref()
        {
            Some("childling") => Some(SeemingBonus::Glamour),
            Some("grump") => Some(SeemingBonus::Willpower),
            Some("wilder") => request.wilder_bonus,
            _ => None,
        };
        if let Some(choice) = choice {
            let key = match choice {
                SeemingBonus::Glamour => "Glamour",
                SeemingBonus::Willpower => "Willpower",
            };
            if let Some(resource) = effective_sheet.resources.get_mut(key) {
                *resource += 1;
            }
        } else {
            issues.push("Select a Childling, Grump, or Wilder Seeming and choose the Wilder bonus when applicable.".into());
        }
        check_c20_starting_powers(&effective_sheet, profile, &mut issues)?;
    } else if input.willpower_free > 0 {
        match effective_sheet.resources.get("Willpower").and_then(|base|base.checked_add(input.willpower_free)).filter(|n|*n<=32767) {
            Some(total)=>{effective_sheet.resources.insert("Willpower".into(),total);},
            None=>issues.push("Additional Willpower requires a configured starting Willpower pool and bounded purchase.".into()),
        }
    }
    let mut spec_names = std::collections::HashSet::new();
    for name in &request.specializations {
        if name.trim().is_empty()
            || !spec_names.insert(name.trim().to_lowercase())
            || !rows(&request.sheet, "AbilityList")?.iter().any(|row| {
                row["Name"]
                    .as_str()
                    .unwrap_or("")
                    .trim()
                    .eq_ignore_ascii_case(name.trim())
            })
        {
            issues.push("Each specialization must identify a different existing Ability.".into());
        }
    }
    input.specialization_count = request.specializations.len() as i64;
    input.basic_power_count =
        configured_power_count(&request.sheet, &profile.primary_power_list, &mut issues)?;
    let secondary_power_count =
        configured_power_count(&request.sheet, &profile.secondary_power_list, &mut issues)?;
    let mut extra_free_spent = 0;
    if mage {
        let arete = 1 + i64::from(request.mage_arete_free);
        if arete > 3 {
            issues.push("Starting Arete cannot exceed 3.".into());
        }
        extra_free_spent = i64::from(request.mage_arete_free) * 2;
        effective_sheet.resources.insert("Arete".into(), arete);
        let avatar = rows(&request.sheet, "BackgroundList")?
            .iter()
            .filter(|row| {
                row["Name"]
                    .as_str()
                    .is_some_and(|name| name.eq_ignore_ascii_case("Avatar"))
            })
            .filter_map(|row| {
                row["Value"]
                    .as_str()
                    .and_then(|value| value.parse::<i64>().ok())
            })
            .sum::<i64>();
        if !(0..=5).contains(&avatar) {
            issues.push("Starting Avatar must be between 0 and 5.".into());
        }
        effective_sheet
            .resources
            .insert("Quintessence".into(), avatar);
        let catalogue = crate::published_powers::catalogue()?;
        let mut families = std::collections::HashSet::new();
        input.basic_power_count = 0;
        for row in rows(&request.sheet, "SphereList")? {
            let power = catalogue.entries.iter().find(|power| {
                power.book_id == "phase7b-ascension"
                    && power.kind == crate::published_powers::Kind::Sphere
                    && row["Name"]
                        .as_str()
                        .is_some_and(|name| power.name.eq_ignore_ascii_case(name.trim()))
            });
            let Some(power) = power else {
                issues.push("Choose a reviewed Sphere name and its native numeric rank.".into());
                continue;
            };
            if i64::from(power.level) > arete
                || row["Value"]
                    .as_str()
                    .and_then(|value| value.parse::<u8>().ok())
                    != Some(power.level)
                || !families.insert(&power.discipline)
            {
                issues.push(format!(
                    "{}: enter one rank per Sphere at or below starting Arete.",
                    power.discipline
                ));
            }
            input.basic_power_count += i64::from(power.level);
        }
        if secondary_power_count != 0 {
            issues.push("Starting Rotes need a separate reviewed allocation rule.".into());
        }
    }
    if mortal && secondary_power_count != 0 {
        issues.push("Starting sorcery rituals need a separate reviewed allocation rule.".into());
    }
    if garou && (input.basic_power_count > profile.basic_power_base || secondary_power_count != 0) {
        issues.push(
            "Additional starting Gifts and Rites need a separately reviewed creation rule.".into(),
        );
    }
    if shadow || mortal || garou {
        for row in rows(&request.sheet, &profile.primary_power_list)? {
            if row["Value"]
                .as_str()
                .is_none_or(|value| !value.trim().eq_ignore_ascii_case("basic"))
            {
                issues.push("Use Basic primary powers for this starting profile.".into());
                break;
            }
        }
    }
    if (mage || mortal || garou)
        && effective_sheet
            .resources
            .get("Willpower")
            .is_some_and(|value| *value > 10)
    {
        issues.push("Starting Willpower cannot exceed 10.".into());
    }
    let configured_budget =
        configured_budget_quote(profile, &input, secondary_power_count, extra_free_spent);
    issues.extend(configured_budget.issues.clone());
    let citations = profile.cost_source.clone().into_iter().collect();
    let ready = issues.is_empty()
        && configured_budget.complete
        && profile.rules_reviewed
        && profile.cost_source.is_some();
    Ok(Quote {
        library_id: library_id.into(),
        record_id: record.id.clone(),
        revision,
        record_hash: purchases::record_hash(record)?,
        request: request.clone(),
        effective_sheet,
        budget: inapplicable_budget(),
        generation: inapplicable_generation(),
        issues,
        warnings,
        citations,
        published_sources: Vec::new(),
        met_mappings: Vec::new(),
        ready,
        configured_profile: Some(profile.clone()),
        configured_budget: Some(configured_budget),
    })
}

pub fn quote(library_id: &str, revision: i64, record: &Record, request: &Request) -> Result<Quote> {
    if record.kind == "Vampire" {
        quote_vampire(library_id, revision, record, request)
    } else {
        quote_configured(library_id, revision, record, request)
    }
}

fn quote_vampire(
    library_id: &str,
    revision: i64,
    record: &Record,
    request: &Request,
) -> Result<Quote> {
    scope(record)?;
    validate_request(request)?;
    history(record)?;
    if !editable(record) {
        return Err(error(
            "CREATION_CLOSED",
            "Initial creation is closed after XP opening or after leaving Draft status.",
        ));
    }
    if let Some(saved) = draft(record)?
        && (saved.library_id != library_id || saved.base_hash != base_hash(record)?)
    {
        return Err(error(
            "CREATION_DRAFT_STALE",
            "The character changed after this draft was saved. Preserve your notes, then explicitly discard and review from the current sheet.",
        ));
    }
    let mut input = request.budget.clone();
    for (i, key) in ["PhysicalList", "SocialList", "MentalList"]
        .iter()
        .enumerate()
    {
        input.attribute_totals[i] = numeric_total(&request.sheet, key)?;
    }
    for (i, key) in ["PhysicalNegList", "SocialNegList", "MentalNegList"]
        .iter()
        .enumerate()
    {
        input.negative_traits[i] = numeric_total(&request.sheet, key)?;
    }
    input.ability_total = numeric_total(&request.sheet, "AbilityList")?;
    input.background_total = numeric_total(&request.sheet, "BackgroundList")?;
    input.merit_cost = numeric_total(&request.sheet, "MeritList")?;
    input.flaw_credit = numeric_total(&request.sheet, "FlawList")?;
    let mut issues = vec![];
    let mut warnings=vec!["Local staff review is a declaration, not organizational approval. Published V20/DA20 levels are not automatic MET tiers.".into()];
    if !request.standard_pools {
        issues.push("Explicitly select the MET Revised ordinary, uncompressed creation profile; optional compressed or exceptional profiles are unsupported.".into());
    }
    if !request.review_confirmed
        || [
            &request.actor,
            &request.reason,
            &request.policy_version,
            &request.review_reference,
        ]
        .iter()
        .any(|v| v.trim().is_empty())
        || !crate::xp::day_valid(&request.day)
    {
        issues.push("Record reviewer, valid date, reason, local policy version and eligibility/progression review, then confirm that review.".into());
    }
    let backgrounds = rows(&request.sheet, "BackgroundList")?;
    let generation_points = backgrounds
        .iter()
        .find(|r| {
            r["Name"]
                .as_str()
                .unwrap_or("")
                .trim()
                .eq_ignore_ascii_case("Generation")
        })
        .map(|r| r["Value"].as_str().unwrap_or("").parse::<i64>())
        .transpose()
        .map_err(|_| {
            error(
                "CREATION_GENERATION",
                "Generation Background must be numeric.",
            )
        })?
        .unwrap_or(0);
    let generation = generation::from_background(generation_points)?;
    for row in backgrounds {
        let value = row["Value"]
            .as_str()
            .unwrap_or("")
            .parse::<i64>()
            .unwrap_or(0);
        if value > 5 {
            issues.push("An ordinary Background cannot exceed five Traits.".into());
        }
        if value > 3 {
            warnings.push(
                "Backgrounds above three require explicit concept approval in the staff review."
                    .into(),
            );
        }
    }
    for count in input.attribute_totals {
        if count > generation.attribute_max {
            issues.push(
                "An Attribute pool exceeds this Generation's ordinary permanent maximum.".into(),
            );
        }
    }
    for row in rows(&request.sheet, "AbilityList")? {
        if row["Value"]
            .as_str()
            .unwrap_or("")
            .parse::<i64>()
            .unwrap_or(0)
            > generation.ability_max
        {
            issues.push("An Ability exceeds this Generation's maximum.".into());
        }
    }
    let mut specs = std::collections::HashSet::new();
    for name in &request.specializations {
        if name.trim().is_empty()
            || !specs.insert(name.trim().to_lowercase())
            || !rows(&request.sheet, "AbilityList")?.iter().any(|r| {
                r["Name"]
                    .as_str()
                    .unwrap_or("")
                    .trim()
                    .eq_ignore_ascii_case(name.trim())
            })
        {
            issues.push("Each specialization must identify a different existing Ability; record its specialty in that Ability's note.".into());
        }
    }
    input.specialization_count = request.specializations.len() as i64;
    let prepared = crate::creation_sources::prepare(library_id, revision, record, request)?;
    let mut effective_sheet = prepared.sheet;
    let published_sources = prepared.quotes;
    let met_mappings = prepared.mappings;
    issues.extend(prepared.issues);
    let powers = rows(&effective_sheet, "DisciplineList")?;
    let mut power_names = std::collections::HashSet::new();
    let mut unbound = false;
    for row in powers {
        if row["Name"].as_str().unwrap_or("").trim().is_empty()
            || !power_names.insert(row["Name"].as_str().unwrap_or("").trim().to_lowercase())
            || row["Value"].as_str() != Some("basic")
            || row.get("AppPurchaseId").is_some()
            || row.get("AppTraitAdvancementId").is_some()
        {
            issues.push("List each distinct initial MET Basic power separately with Value 'basic', without an XP purchase identity.".into());
        }
        unbound |= crate::published_reference::verified_reference(row).is_none();
    }
    if unbound && request.manual_power_reference.trim().is_empty() {
        issues.push("Unbound initial powers need an explicit source/edition/page and reviewed MET Basic/progression mapping reference.".into());
    }
    if unbound {
        warnings.push("Initial powers use a manually declared source/mapping review; automated catalogue eligibility is not claimed.".into());
    }
    input.basic_power_count = powers.len() as i64;
    if input.optional_derangement
        && request
            .sheet
            .lists
            .get("DerangementList")
            .and_then(|v| v["Traits"].as_array())
            .is_none_or(|r| {
                !r.iter().any(|row| {
                    row["Name"]
                        .as_str()
                        .is_some_and(|name| !name.trim().is_empty())
                })
            })
    {
        issues.push("The optional derangement credit requires a recorded qualifying derangement and staff eligibility review.".into());
    }
    let budget = budget::quote(&input)?;
    issues.extend(budget.issues.clone());
    warnings.extend(budget.warnings.clone());
    let first = if input.conviction {
        "Conviction"
    } else {
        "Conscience"
    };
    let second = if input.instinct {
        "Instinct"
    } else {
        "SelfControl"
    };
    for key in [
        if input.conviction {
            "Conscience"
        } else {
            "Conviction"
        },
        if input.instinct {
            "SelfControl"
        } else {
            "Instinct"
        },
    ] {
        if effective_sheet.resources.get(key).copied().unwrap_or(0) != 0 {
            issues.push(format!(
                "Reconcile the unused {key} value explicitly before changing Virtue kinds."
            ));
        }
    }
    if effective_sheet
        .identity
        .get("Path")
        .is_none_or(|v| v.trim().is_empty())
    {
        issues.push("Record Humanity or the explicitly reviewed Path name.".into());
    }
    let willpower = generation.starting_willpower + input.willpower_free;
    if willpower > generation.willpower_max {
        issues.push("Willpower exceeds the Generation maximum.".into());
    }
    for (key, value) in [
        ("Generation", generation.generation),
        ("Blood", generation.blood_capacity),
        ("Willpower", willpower),
        (first, budget.virtues[0]),
        (second, budget.virtues[1]),
        ("Courage", budget.virtues[2]),
        ("PathTraits", budget.morality),
    ] {
        effective_sheet.resources.insert(key.into(), value);
        if !record.private_fields.contains_key(key) && !record.public_fields.contains_key(key) {
            effective_sheet.visibility.entry(key.into()).or_insert(true);
        }
    }
    for (key, max) in [
        ("TempBlood", generation.blood_capacity),
        ("TempWillpower", willpower),
    ] {
        if let Some(value) = effective_sheet.resources.get(key)
            && (!(0..=max).contains(value) || request.current_resources_reference.trim().is_empty())
        {
            issues.push(format!("{key} requires a game-start/current-resource reference and a value within capacity."));
        }
    }
    warnings.push("Blood is capacity. Current Blood is not auto-filled; game-start tests and modifiers need their own record.".into());
    let mut citations = generation::source_citations().to_vec();
    for page in [67, 68, 69, 70, 71, 72, 106] {
        let mut citation = citations[0].clone();
        citation.printed_page = page;
        citation.pdf_page_1based = page + 1;
        citations.push(citation);
    }
    let ready = issues.is_empty() && budget.complete;
    Ok(Quote {
        library_id: library_id.into(),
        record_id: record.id.clone(),
        revision,
        record_hash: purchases::record_hash(record)?,
        request: request.clone(),
        effective_sheet,
        budget,
        generation,
        issues,
        warnings,
        citations,
        published_sources,
        met_mappings,
        ready,
        configured_profile: None,
        configured_budget: None,
    })
}
