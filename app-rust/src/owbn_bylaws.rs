//! Local evidence checks for reviewed One World by Night policies.
use crate::{
    chronicle::Profile,
    model::{Record, Result, SheetEdit, error},
    pricing::MetTier,
    purchases::{PurchaseRequest, TeacherRule},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
pub enum PacketAuthority {
    #[default]
    Unknown,
    CustomHouseRule,
    NonBindingOrBook,
    BindingPacket,
    BylawMandated,
}
impl PacketAuthority {
    pub fn is_unknown(&self) -> bool {
        *self == Self::Unknown
    }

    fn priority(self) -> u8 {
        match self {
            Self::Unknown => 0,
            Self::NonBindingOrBook | Self::CustomHouseRule => 1,
            Self::BindingPacket => 2,
            Self::BylawMandated => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TierMappingBasis {
    NativeMetTier,
    ConvertedFrom20thLevel,
    PacketSpecified,
}
impl TierMappingBasis {
    pub fn convert_20th_level(level: u8) -> Option<MetTier> {
        match level {
            1..=2 => Some(MetTier::Basic),
            3..=4 => Some(MetTier::Intermediate),
            5 => Some(MetTier::Advanced),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Regulation {
    Unregulated,
    CoordinatorNotify,
    CoordinatorApproval,
    CouncilVote,
    DisallowedForPc,
}
impl Regulation {
    pub fn label(self, pt: bool) -> &'static str {
        match (self, pt) {
            (Self::Unregulated, false) => "Unregulated",
            (Self::Unregulated, true) => "Sem restrição",
            (Self::CoordinatorNotify, false) => "Coordinator notification",
            (Self::CoordinatorNotify, true) => "Notificação ao coordenador",
            (Self::CoordinatorApproval, false) => "Coordinator approval",
            (Self::CoordinatorApproval, true) => "Aprovação do coordenador",
            (Self::CouncilVote, false) => "Council vote",
            (Self::CouncilVote, true) => "Votação do Conselho",
            (Self::DisallowedForPc, false) => "Disallowed for player characters",
            (Self::DisallowedForPc, true) => "Vedado a personagens de jogador",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegulationEvidence {
    pub rule_id: String,
    pub regulation: Regulation,
    pub owbn_coordinator_reference: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Evidence {
    pub references: Vec<RegulationEvidence>,
    pub teacher_exception_reference: String,
    pub confirm_non_binding_sources: bool,
}
impl Evidence {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    pub fn validate(&self) -> Result<()> {
        let mut ids = BTreeSet::new();
        let mut references = BTreeSet::new();
        if self.references.len() > 64
            || self.teacher_exception_reference.len() > 512
            || self
                .teacher_exception_reference
                .chars()
                .any(char::is_control)
            || self.references.iter().any(|item| {
                let reference = item.owbn_coordinator_reference.trim();
                !bounded(&item.rule_id, 96)
                    || !reference.is_empty() && !references.insert(reference)
                    || item.owbn_coordinator_reference.len() > 512
                    || item
                        .owbn_coordinator_reference
                        .chars()
                        .any(char::is_control)
                    || !ids.insert(&item.rule_id)
            })
        {
            return Err(error(
                "OWBN_EVIDENCE_INVALID",
                "Review the OWbN evidence fields. / Revise as referências da OWbN.",
            ));
        }
        Ok(())
    }
}

fn bounded(value: &str, limit: usize) -> bool {
    !value.trim().is_empty() && value.len() <= limit && !value.chars().any(char::is_control)
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Selector {
    Creature,
    Identity {
        field: String,
        value: String,
    },
    Power {
        value: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        level: Option<u8>,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct SelectedPower<'a> {
    pub names: &'a [&'a str],
    pub level: Option<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub id: String,
    pub kind: String,
    pub selector: Selector,
    pub authority: PacketAuthority,
    pub pc: Regulation,
    pub npc: Regulation,
    pub citation: String,
}
impl Rule {
    pub fn regulation(&self, npc: bool) -> Regulation {
        let state = if npc { self.npc } else { self.pc };
        if npc && state == Regulation::DisallowedForPc {
            Regulation::Unregulated
        } else {
            state
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub version: u32,
    pub mechanics_reference: String,
    pub character_reference: String,
    pub vampire_pc_min_generation: i64,
    pub pc_discipline_max_level: u8,
    pub pc_sorcery_path_max_level: u8,
    pub pc_numina_path_max_level: u8,
    pub rules: Vec<Rule>,
}
impl Policy {
    pub fn from_json(source: &str) -> Result<Self> {
        let invalid = || {
            error(
                "OWBN_POLICY_INVALID",
                "OWbN policy needs review. / A política da OWbN precisa de revisão.",
            )
        };
        if source.len() > 1024 * 1024 {
            return Err(invalid());
        }
        let policy: Self = serde_json::from_str(source).map_err(|_| invalid())?;
        let mut ids = BTreeSet::new();
        if policy.version != 1
            || !bounded(&policy.mechanics_reference, 512)
            || !bounded(&policy.character_reference, 512)
            || policy.vampire_pc_min_generation != 8
            || policy.pc_discipline_max_level != 5
            || !(5..=6).contains(&policy.pc_sorcery_path_max_level)
            || !(5..=6).contains(&policy.pc_numina_path_max_level)
            || policy.rules.len() > 4096
            || policy.rules.iter().any(|rule| {
                !bounded(&rule.id, 96)
                    || !ids.insert(&rule.id)
                    || !crate::sheet::is_character_kind(&rule.kind)
                    || !bounded(&rule.citation, 512)
                    || rule.authority.is_unknown()
                    || match &rule.selector {
                        Selector::Creature => false,
                        Selector::Identity { field, value } => {
                            !crate::sheet::IDENTITY_FIELDS.contains(&field.as_str())
                                || !bounded(value, 160)
                        }
                        Selector::Power { value, level } => {
                            !bounded(value, 160) || level.is_some_and(|n| !(1..=12).contains(&n))
                        }
                    }
            })
        {
            return Err(invalid());
        }
        Ok(policy)
    }

    pub fn matching<'a>(
        &'a self,
        kind: &str,
        sheet: &SheetEdit,
        powers: &[SelectedPower<'_>],
        npc: bool,
    ) -> Vec<&'a Rule> {
        let mut selected = BTreeMap::new();
        for rule in self.rules.iter().filter(|rule| rule.kind == kind) {
            let key = match &rule.selector {
                Selector::Creature => (0, "", String::new()),
                Selector::Identity { field, value }
                    if sheet
                        .identity
                        .get(field)
                        .is_some_and(|actual| same(actual, value)) =>
                {
                    (1, field.as_str(), normalized(value))
                }
                Selector::Power { value, level }
                    if powers.iter().any(|power| {
                        (level.is_none() || power.level.is_none() || power.level == *level)
                            && power.names.iter().any(|name| same(name, value))
                    }) =>
                {
                    (2, "", normalized(value))
                }
                _ => continue,
            };
            let candidate = (rule.authority.priority(), rule.regulation(npc));
            let current: &mut (u8, Regulation, &Rule) =
                selected
                    .entry(key)
                    .or_insert((candidate.0, candidate.1, rule));
            if candidate > (current.0, current.1) {
                *current = (candidate.0, candidate.1, rule);
            }
        }
        selected.into_values().map(|(_, _, rule)| rule).collect()
    }

    pub fn check_regulations(&self, rules: &[&Rule], npc: bool, evidence: &Evidence) -> Result<()> {
        for rule in rules {
            let regulation = rule.regulation(npc);
            match regulation {
                Regulation::Unregulated => {}
                Regulation::DisallowedForPc => {
                    return Err(error(
                        "OWBN_PC_DISALLOWED",
                        format!(
                            "{}: {} / {}. {}",
                            rule.id,
                            regulation.label(false),
                            regulation.label(true),
                            rule.citation
                        ),
                    ));
                }
                _ if !evidence.references.iter().any(|entry| {
                    entry.rule_id == rule.id
                        && entry.regulation == regulation
                        && bounded(&entry.owbn_coordinator_reference, 512)
                }) =>
                {
                    return Err(error(
                        "OWBN_REGULATION_EVIDENCE",
                        format!(
                            "{}: enter a local reference for {}. / Informe uma referência local para {}. {}",
                            rule.id,
                            regulation.label(false),
                            regulation.label(true),
                            rule.citation
                        ),
                    ));
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub fn check_generation(&self, kind: &str, npc: bool, generation: Option<i64>) -> Result<()> {
        if kind == "Vampire" && !npc {
            match generation {
                Some(value) if value < self.vampire_pc_min_generation => {
                    return Err(error(
                        "OWBN_PC_GENERATION",
                        "A Vampire PC below 8th Generation must retire as an NPC. / Um vampiro PJ abaixo da 8ª Geração deve ser retirado de jogo como PNJ.",
                    ));
                }
                None => {
                    return Err(error(
                        "OWBN_GENERATION_REQUIRED",
                        "Record the Vampire PC's generation before creation or advancement. / Registre a geração do vampiro PJ antes da criação ou evolução.",
                    ));
                }
                _ => {}
            }
        }
        Ok(())
    }
}

pub fn builtin() -> &'static Policy {
    static POLICY: std::sync::OnceLock<Policy> = std::sync::OnceLock::new();
    POLICY.get_or_init(|| {
        Policy::from_json(include_str!("../data/owbn_bylaws.json")).expect("reviewed OWbN policy")
    })
}

fn normalized(value: &str) -> String {
    value.trim().to_lowercase()
}
fn same(left: &str, right: &str) -> bool {
    normalized(left) == normalized(right)
}

pub(crate) fn is_npc(record: &Record) -> bool {
    record
        .private_fields
        .get("NPC")
        .or_else(|| record.public_fields.get("NPC"))
        .is_some_and(|value| {
            value
                .as_bool()
                .or_else(|| value["value"].as_bool())
                .unwrap_or_else(|| {
                    value.as_str().is_some_and(|text| {
                        matches!(
                            text.trim().to_ascii_lowercase().as_str(),
                            "yes" | "true" | "1" | "-1"
                        )
                    })
                })
        })
}

pub fn check_authority(
    profile: &Profile,
    authority: PacketAuthority,
    evidence: &Evidence,
) -> Result<()> {
    if !profile.owbn_bylaws_enabled {
        return Ok(());
    }
    match authority {
        PacketAuthority::Unknown => Err(error(
            "OWBN_AUTHORITY_UNKNOWN",
            "Review the source's OWbN authority. / Revise a autoridade da fonte na OWbN.",
        )),
        PacketAuthority::NonBindingOrBook
            if !profile.allow_non_binding_sources && !evidence.confirm_non_binding_sources =>
        {
            Err(error(
                "OWBN_NON_BINDING_SOURCE",
                "Confirm Storyteller use of this non-binding source. / Confirme com o Narrador o uso desta fonte não vinculante.",
            ))
        }
        _ => Ok(()),
    }
}
pub fn non_binding_badge(pt: bool) -> &'static str {
    if pt {
        "Pacote não vinculante / fonte do 20º aniversário (a critério do Narrador local)"
    } else {
        "Non-Binding Packet / 20th Ann. Source (Local ST Discretion)"
    }
}

pub fn check_teacher(request: &PurchaseRequest) -> Result<()> {
    if request.relation == Some(crate::pricing::ClanRelation::OutOfClan)
        && request.teacher_rule == Some(TeacherRule::NotRequiredByLocalRuling)
        && !bounded(&request.owbn_evidence.teacher_exception_reference, 512)
    {
        return Err(error(
            "OWBN_TEACHER_REQUIRED",
            "Out-of-Clan learning requires a teacher or a Coordinator/packet exception reference. / Aprender fora do clã exige um professor ou uma referência de exceção do coordenador ou do pacote.",
        ));
    }
    Ok(())
}

pub fn check_advancement(profile: &Profile, record: &Record) -> Result<()> {
    if profile.owbn_bylaws_enabled {
        builtin().check_generation(
            &record.kind,
            is_npc(record),
            crate::sheet::extract(record)
                .resources
                .get("Generation")
                .copied(),
        )?;
    }
    Ok(())
}

pub fn check_purchase(profile: &Profile, record: &Record, request: &PurchaseRequest) -> Result<()> {
    if !profile.owbn_bylaws_enabled {
        return Ok(());
    }
    request.owbn_evidence.validate()?;
    check_advancement(profile, record)?;
    let npc = is_npc(record);
    let power = crate::power_catalogue::builtin()
        .lookup(&request.power_id)
        .ok_or_else(|| {
            error(
                "OWBN_POWER_UNKNOWN",
                "Review the power's OWbN source. / Revise a fonte do poder na OWbN.",
            )
        })?;
    let sheet = crate::sheet::extract(record);
    let rules = builtin().matching(
        &record.kind,
        &sheet,
        &[SelectedPower {
            names: &[&power.stable_id, &power.name, &power.family],
            level: power.source_level,
        }],
        npc,
    );
    builtin().check_regulations(&rules, npc, &request.owbn_evidence)?;
    check_teacher(request)?;
    check_catalogue_level(npc, power)?;
    check_authority(profile, power.effective_authority(), &request.owbn_evidence)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LevelKind {
    Discipline,
    SorceryPath,
    NuminaPath,
}

fn check_catalogue_level(npc: bool, power: &crate::power_catalogue::Definition) -> Result<()> {
    let kind = power.level_kind.or(match power.target_list.as_str() {
        "NuminaList" => Some(LevelKind::NuminaPath),
        "DisciplineList" => Some(LevelKind::Discipline),
        _ => None,
    });
    if let (Some(kind), Some(level)) = (kind, power.source_level) {
        check_power_level(npc, kind, level)?;
    }
    Ok(())
}

pub fn check_power_level(npc: bool, kind: LevelKind, level: u8) -> Result<()> {
    let policy = builtin();
    let max = match kind {
        LevelKind::Discipline => policy.pc_discipline_max_level,
        LevelKind::SorceryPath => policy.pc_sorcery_path_max_level,
        LevelKind::NuminaPath => policy.pc_numina_path_max_level,
    };
    if !npc && level > max {
        return Err(error(
            "OWBN_PC_POWER_CEILING",
            format!(
                "The PC power exceeds the permitted level ({max}). / O poder do PJ excede o nível permitido ({max})."
            ),
        ));
    }
    Ok(())
}

fn initial_powers<'a>(record: &Record, sheet: &'a SheetEdit) -> Vec<(&'static str, &'a Value)> {
    let mut powers = Vec::new();
    if let Some(profile) = crate::creature_profiles::profile(&record.kind) {
        for list in [&profile.primary_power_list, &profile.secondary_power_list] {
            if let Some(rows) = sheet
                .lists
                .get(list)
                .and_then(|value| value["Traits"].as_array())
            {
                powers.extend(rows.iter().map(|row| (list.as_str(), row)));
            }
        }
    }
    powers
}

pub(crate) fn check_creation(
    profile: &Profile,
    record: &Record,
    quote: &crate::vampire_creation::Quote,
) -> Result<()> {
    if !profile.owbn_bylaws_enabled {
        return Ok(());
    }
    let evidence = &quote.request.owbn_evidence;
    evidence.validate()?;
    let npc = quote.request.create_as_npc || is_npc(record);
    let sheet = &quote.effective_sheet;
    builtin().check_generation(
        &record.kind,
        npc,
        sheet.resources.get("Generation").copied(),
    )?;
    let initial = initial_powers(record, sheet);
    let attached: Vec<_> = initial
        .iter()
        .map(|(_, row)| crate::published_reference::verified_reference(row))
        .collect();
    let mapped: BTreeMap<_, _> = quote
        .met_mappings
        .iter()
        .filter_map(|mapping| {
            crate::power_catalogue::builtin()
                .lookup(&mapping.power_id)
                .map(|power| (mapping.index, power))
        })
        .collect();
    let mut powers: Vec<(Vec<&str>, Option<u8>)> = Vec::new();
    let mut has_manual_source = false;
    for (index, ((list, row), reference)) in initial.iter().zip(&attached).enumerate() {
        let mut names: Vec<_> = row["Name"].as_str().into_iter().collect();
        let met = (*list == "DisciplineList")
            .then(|| mapped.get(&index))
            .flatten();
        if let Some(power) = met {
            names.extend([
                power.stable_id.as_str(),
                power.name.as_str(),
                power.family.as_str(),
            ]);
        }
        if let Some(source) = reference {
            names.extend([
                source.power.id.as_str(),
                source.power.name.as_str(),
                source.power.discipline.as_str(),
            ]);
        }
        let level = reference
            .as_ref()
            .map(|source| source.power.level)
            .or_else(|| met.and_then(|power| power.source_level));
        if reference.is_none() && met.is_none() {
            has_manual_source = true;
            check_manual_power_level(npc, list, row)?;
        }
        powers.push((names, level));
    }
    let selections: Vec<_> = powers
        .iter()
        .map(|(names, level)| SelectedPower {
            names,
            level: *level,
        })
        .collect();
    let rules = builtin().matching(&record.kind, sheet, &selections, npc);
    builtin().check_regulations(&rules, npc, evidence)?;
    for reference in attached.iter().flatten() {
        use crate::published_powers::Kind;
        let kind = match reference.power.kind {
            Kind::Discipline => Some(LevelKind::Discipline),
            Kind::Path => Some(LevelKind::SorceryPath),
            Kind::Numina => Some(LevelKind::NuminaPath),
            _ => None,
        };
        if let Some(kind) = kind {
            check_power_level(npc, kind, reference.power.level)?;
        }
        check_authority(profile, reference.power.effective_authority(), evidence)?;
    }
    for power in mapped.values() {
        check_catalogue_level(npc, power)?;
        check_authority(profile, power.effective_authority(), evidence)?;
    }
    if has_manual_source {
        check_authority(profile, PacketAuthority::NonBindingOrBook, evidence)?;
    }
    Ok(())
}

fn check_manual_power_level(npc: bool, list: &str, row: &Value) -> Result<()> {
    let kind = match list {
        "DisciplineList" => LevelKind::Discipline,
        "NuminaList" => LevelKind::NuminaPath,
        _ => return Ok(()),
    };
    let value = normalized(row["Value"].as_str().unwrap_or(""));
    let level = match value.as_str() {
        "elder" | "ancião" => Some(6),
        value => value
            .strip_prefix("level ")
            .or_else(|| value.strip_prefix("nível "))
            .unwrap_or(value)
            .parse()
            .ok(),
    };
    if let Some(level) = level {
        check_power_level(npc, kind, level)?;
    }
    Ok(())
}

pub(crate) fn set_creation_npc(record: &mut Record) {
    record.public_fields.remove("NPC");
    record
        .private_fields
        .insert("NPC".into(), Value::Bool(true));
}

pub(crate) fn append_mechanics_reminders(document: &mut crate::printing::SheetDocument) {
    use crate::printing::{DocLanguage, Row, Section};
    let pt = document.options.language == DocLanguage::Portuguese;
    let (title, rows) = if pt {
        (
            "Estatutos de Mecânica da OWbN",
            [
                (
                    "Empates de vampiros PJ",
                    "Até 30 Características em lances para desempate.",
                ),
                (
                    "Combate de vampiros PJ",
                    "Até 10 ações por rodada. Criaturas criadas ou invocadas usam ações próprias.",
                ),
                (
                    "Bônus de Moralidade",
                    "Quando um poder concede bônus igual à Moralidade, use metade, arredondada para cima.",
                ),
                (
                    "Fé Verdadeira",
                    "Exige Moralidade 9 ou mais para ser mantida.",
                ),
            ],
        )
    } else {
        (
            "OWbN Mechanics Bylaws",
            [
                ("Vampire PC ties", "Bid up to 30 Traits for ties."),
                (
                    "Vampire PC combat",
                    "Up to 10 actions per round. Created or summoned creatures use their own actions.",
                ),
                (
                    "Morality bonuses",
                    "When a power grants bonus Traits equal to Morality, halve the bonus and round up.",
                ),
                (
                    "True Faith",
                    "Maintaining True Faith requires Morality 9 or higher.",
                ),
            ],
        )
    };
    document.sections.push(Section {
        title: title.into(),
        rows: rows
            .into_iter()
            .map(|(name, value)| Row {
                name: name.into(),
                value: value.into(),
                note: None,
            })
            .collect(),
    });
}
