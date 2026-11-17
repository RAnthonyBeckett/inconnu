//! Source-bound local purchase declarations; these are not organizational authorization.
use crate::{importer, model::*, pricing::*, purchase_preview::*, xp};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TeacherRule {
    Required,
    NotRequiredByLocalRuling,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PurchaseRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_id: Option<String>,
    pub id: String,
    pub power_id: String,
    pub profile: Option<PricingProfile>,
    pub relation: Option<ClanRelation>,
    pub affiliation_name: String,
    pub day: String,
    pub available_from: String,
    pub next_session_reference: String,
    pub actor: String,
    pub reason: String,
    pub policy_version: String,
    pub policy_reference: String,
    pub affiliation_evidence: String,
    pub approval_reference: String,
    pub eligibility_evidence: String,
    pub teacher_rule: Option<TeacherRule>,
    pub teacher_evidence: String,
    pub standard_cost_confirmed: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PurchaseQuote {
    pub request: PurchaseRequest,
    pub library_id: String,
    pub record_id: String,
    pub record_name: String,
    pub revision: i64,
    pub record_hash: String,
    pub price: AnimalismPurchasePreview,
    pub before_unspent: String,
    pub after_unspent: String,
    pub trait_added: Value,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PurchaseReceipt {
    pub quote: PurchaseQuote,
    pub committed_revision: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cancellation: Option<crate::cancellations::CancellationReceipt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batch: Option<crate::purchase_batch::Binding>,
}

pub fn record_hash(record: &Record) -> Result<String> {
    Ok(Sha256::digest(serde_json::to_vec(record)?)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}
pub(crate) fn discipline_list(record: &Record) -> Result<Value> {
    let field = crate::creature_profiles::profile(&record.kind)
        .map(|p| p.primary_power_list.as_str())
        .unwrap_or("DisciplineList");
    power_list(record, field)
}
pub(crate) fn power_list(record: &Record, field: &str) -> Result<Value> {
    if !crate::sheet::LIST_FIELDS.contains(&field) {
        return Err(error(
            "PURCHASE_UNSUPPORTED_SHEET",
            "Unknown target power list.",
        ));
    }
    let explicit = record
        .private_fields
        .get(field)
        .or_else(|| record.public_fields.get(field));
    let list = if let Some(list) = explicit {
        list.clone()
    } else if let Some(list) = crate::sheet::extract(record).lists.get(field) {
        // Use the same unique legacy projection as the sheet editor. Starting an
        // empty canonical list here would hide existing imported disciplines.
        list.clone()
    } else {
        let legacy = record
            .private_fields
            .get("UnmappedTraitLists")
            .or_else(|| record.public_fields.get("UnmappedTraitLists"));
        let has_discipline = legacy.is_some_and(|value| {
            let candidates: Vec<&Value> = value
                .as_array()
                .map(|rows| rows.iter().collect())
                .unwrap_or_else(|| vec![value]);
            candidates.iter().any(|row| {
                row["Name"].as_str().and_then(crate::importer::xml_list_key) == Some(field)
            })
        });
        if has_discipline {
            return Err(error(
                "PURCHASE_UNSUPPORTED_SHEET",
                "Legacy discipline lists are ambiguous or unreadable. Resolve them before purchasing.",
            ));
        }
        crate::sheet::empty_list(field)
    };
    if !crate::sheet::editable_list(&list) {
        return Err(error(
            "PURCHASE_UNSUPPORTED_SHEET",
            "Existing discipline data needs review before a purchase can change it.",
        ));
    }
    Ok(list)
}
pub fn quote(
    library_id: &str,
    revision: i64,
    record: &Record,
    input: &PurchaseRequest,
) -> Result<PurchaseQuote> {
    quote_from_catalogue(
        library_id,
        revision,
        record,
        input,
        crate::power_catalogue::builtin(),
        crate::creature_profiles::builtin(),
    )
}

pub fn quote_from_catalogue(
    library_id: &str,
    revision: i64,
    record: &Record,
    input: &PurchaseRequest,
    powers: &crate::power_catalogue::Catalogue,
    profiles: &crate::creature_profiles::Catalogue,
) -> Result<PurchaseQuote> {
    crate::purchase_reviews::verify_from_catalogues(library_id, record, input, powers, profiles)?;
    if !crate::sheet::is_character_kind(&record.kind) || record.build.is_some() {
        return Err(error(
            "PURCHASE_SCOPE",
            "Only a live character sheet can purchase this supported power.",
        ));
    }
    if uuid::Uuid::parse_str(&input.id)
        .ok()
        .is_none_or(|id| id.to_string() != input.id)
    {
        return Err(error(
            "PURCHASE_ID",
            "A canonical purchase request ID is required.",
        ));
    }
    if !input.standard_cost_confirmed || input.teacher_rule.is_none() {
        return Err(error(
            "PURCHASE_RULES_UNRESOLVED",
            "Confirm the exact source price and an explicit local teacher ruling. Other cost adjustments are unsupported.",
        ));
    }
    for field in [
        &input.actor,
        &input.reason,
        &input.affiliation_name,
        &input.policy_version,
        &input.policy_reference,
        &input.affiliation_evidence,
        &input.approval_reference,
        &input.eligibility_evidence,
        &input.teacher_evidence,
        &input.next_session_reference,
    ] {
        if field.trim().is_empty() || field.len() > 16000 {
            return Err(error(
                "PURCHASE_EVIDENCE_REQUIRED",
                "Record the actor, reason, local policy/version, affiliation, eligibility, approval, teacher ruling and next-session evidence before quoting.",
            ));
        }
    }
    if !xp::day_valid(&input.day)
        || !xp::day_valid(&input.available_from)
        || input.available_from < input.day
    {
        return Err(error(
            "PURCHASE_DATE",
            "Use valid purchase/availability dates and an availability date on or after the purchase, tied to the declared next session.",
        ));
    }
    let definition = powers
        .lookup(&input.power_id)
        .ok_or_else(|| error("UNKNOWN_ANIMALISM_POWER", "Unknown reviewed power."))?;
    let rules = profiles
        .profile(&record.kind)
        .ok_or_else(|| error("UNSUPPORTED_PRICING_SCOPE", "Unknown creature profile."))?;
    let price = preview_from_definition(
        &record.kind,
        definition,
        input.profile,
        input.relation,
        rules,
    )?;
    price.validate_scope(&record.kind)?;
    let list = power_list(record, price.target_list())?;
    if list["Traits"].as_array().unwrap().iter().any(|t| {
        t["AppSourcePowerId"].as_str() == Some(&input.power_id)
            || t["Name"]
                .as_str()
                .is_some_and(|n| n.trim().eq_ignore_ascii_case(&price.name))
    }) {
        return Err(error(
            "PURCHASE_ALREADY_PRESENT",
            "The source power or matching discipline label already exists. Resolve legacy identity before buying it again.",
        ));
    }
    let ledger = record.xp_ledger.as_ref().ok_or_else(|| {
        error(
            "XP_OPENING_REQUIRED",
            "Declare a supported XP opening before purchasing.",
        )
    })?;
    if ledger.entries.iter().any(|e| e.command.id == input.id) {
        return Err(error(
            "PURCHASE_ID_CONFLICT",
            "That purchase ID already identifies an XP event.",
        ));
    }
    let (_, balance) = xp::replay(ledger)?;
    let cost = xp::amount(&price.price.total)?;
    let after = balance.checked_sub(cost).ok_or_else(|| {
        error(
            "INVALID_XP",
            "Purchase arithmetic exceeds supported bounds.",
        )
    })?;
    if after < 0 {
        return Err(error(
            "PURCHASE_INSUFFICIENT_XP",
            "The committed ledger does not have enough unspent XP.",
        ));
    }
    let trait_added = json!({"Name":price.name,"Value":price.tier.sheet_value(),"Note":format!("Available from {}; next-session availability declared locally.",input.available_from),"AppSourcePowerId":input.power_id,"AppPurchaseId":input.id,"AppAvailableFrom":input.available_from});
    let q = PurchaseQuote {
        request: input.clone(),
        library_id: library_id.into(),
        record_id: record.id.clone(),
        record_name: record.name.clone(),
        revision,
        record_hash: record_hash(record)?,
        price,
        before_unspent: xp::decimal(balance),
        after_unspent: xp::decimal(after),
        trait_added,
    };
    // Quote includes replay of the dated debit, so backdating cannot hide a negative transition.
    let _ = apply(record, &q, 0)?;
    Ok(q)
}
fn command(q: &PurchaseQuote) -> xp::XpCommand {
    let r = &q.request;
    let p = &q.price.price;
    xp::XpCommand {
        id: r.id.clone(),
        kind: xp::XpKind::Spend,
        amount: p.total.clone(),
        day: r.day.clone(),
        actor: r.actor.clone(),
        reason: r.reason.clone(),
        evidence: format!("Purchase receipt {}; {}", r.id, r.approval_reference),
        reversal_of: None,
        cost: Some(xp::CostEvidence {
            affiliation_name: r.affiliation_name.clone(),
            power_family: q.price.cost_family_label(),
            approval_reference: r.approval_reference.clone(),
            base: p.base.clone(),
            affiliation_adjustment: p.affiliation_adjustment.clone(),
            affiliation: r
                .relation
                .map(|r| format!("{r:?}"))
                .unwrap_or_else(|| "Unknown".into()),
            source: p.citation.book.clone(),
            edition: p.citation.edition.clone(),
            page: p.citation.printed_page.to_string(),
            policy_version: r.policy_version.clone(),
        }),
    }
}
pub fn apply(record: &Record, q: &PurchaseQuote, recorded_unix_seconds: u64) -> Result<Record> {
    if record.id != q.record_id || record_hash(record)? != q.record_hash {
        return Err(error(
            "STALE_PURCHASE",
            "The committed sheet changed after this quote.",
        ));
    }
    q.price.validate_scope(&record.kind)?;
    let field = q.price.target_list();
    let mut updated = record.clone();
    let ledger = updated
        .xp_ledger
        .as_mut()
        .ok_or_else(|| error("XP_OPENING_REQUIRED", "Declare an XP opening first."))?;
    let c = xp::canonical_command(ledger, &command(q))?;
    ledger.entries.push(xp::Entry {
        command: c,
        recorded_unix_seconds,
        origin_library_id: q.library_id.clone(),
    });
    xp::replay(ledger)?;
    let mut list = power_list(record, field)?;
    list["Traits"]
        .as_array_mut()
        .unwrap()
        .push(q.trait_added.clone());
    // Preserve the same public/private field policy used by ordinary sheet editing.
    if updated.public_fields.contains_key(field) {
        let mut safe = importer::redact(&list);
        if let Some((start, end)) = &updated.st_markers {
            safe = importer::redact_custom(&safe, start, end);
        }
        updated.public_fields.insert(field.into(), safe.clone());
        if safe != list {
            updated.private_fields.insert(field.into(), list);
        } else {
            updated.private_fields.remove(field);
        }
    } else {
        updated.private_fields.insert(field.into(), list);
    }
    Ok(updated)
}
pub fn receipt(quote: &PurchaseQuote, committed_revision: i64) -> PurchaseReceipt {
    PurchaseReceipt {
        quote: quote.clone(),
        committed_revision,
        cancellation: None,
        batch: None,
    }
}
pub fn findings(record: &Record, receipt: &PurchaseReceipt) -> Vec<xp::Finding> {
    if let Some(cancel) = &receipt.cancellation {
        return crate::cancellations::findings(record, receipt, cancel);
    }
    let q = &receipt.quote;
    let mut out = vec![];
    let add = |out: &mut Vec<xp::Finding>, code: &str, message: &str| {
        out.push(xp::Finding {
            code: code.into(),
            severity: "review".into(),
            entry_id: Some(q.request.id.clone()),
            message: message.into(),
        })
    };
    if crate::purchase_reviews::verify_declarations(&q.library_id, record, &q.request).is_err() {
        add(
            &mut out,
            "PURCHASE_REVIEW_DRIFT",
            "The saved local review no longer matches its linked purchase declarations; inspect the review history.",
        );
    }
    let list = record
        .private_fields
        .get(q.price.target_list())
        .or_else(|| record.public_fields.get(q.price.target_list()));
    let traits = list.and_then(|v| v["Traits"].as_array());
    let matches: Vec<_> = traits
        .into_iter()
        .flatten()
        .filter(|t| t["AppPurchaseId"].as_str() == Some(q.request.id.as_str()))
        .collect();
    if matches.len() != 1
        || matches
            .first()
            .is_none_or(|t| mechanical_trait(t) != mechanical_trait(&q.trait_added))
    {
        add(
            &mut out,
            "PURCHASE_TRAIT_DRIFT",
            "A purchased trait no longer matches its receipt. Review the later sheet change; this is not a cheating finding.",
        );
    }
    let ledger = record.xp_ledger.as_ref();
    let entries: Vec<_> = ledger
        .into_iter()
        .flat_map(|l| &l.entries)
        .filter(|e| e.command.id == q.request.id)
        .collect();
    if entries.len() != 1 || entries.first().is_none_or(|e| e.command != command(q)) {
        add(
            &mut out,
            "PURCHASE_SPEND_MISMATCH",
            "The XP debit linked to a purchase receipt is missing or differs from that receipt.",
        );
    }
    if ledger.is_some_and(|l| {
        l.entries
            .iter()
            .any(|e| e.command.reversal_of.as_deref() == Some(q.request.id.as_str()))
    }) {
        add(
            &mut out,
            "PURCHASE_SPEND_REVERSED",
            "A purchase debit was reversed while its receipt remains. Review funding and the trait together.",
        );
    }
    out
}

fn mechanical_trait(value: &Value) -> Value {
    let mut trait_value = value.clone();
    if let Some(fields) = trait_value.as_object_mut() {
        fields.remove(crate::published_reference::TRAIT_FIELD);
    }
    trait_value
}
