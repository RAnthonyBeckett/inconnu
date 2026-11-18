use super::*;
use crate::vampire_creation as creation;

impl Library {
    pub fn vampire_creation_context(&self, id: &str, role: Role) -> Result<creation::Context> {
        require_st(role)?;
        let record = self.stored_record(id)?;
        creation::scope(&record)?;
        let history = creation::history(&record)?;
        let request = if let Some(draft) = creation::draft(&record)? {
            draft.request
        } else if let Some(last) = history.last() {
            let mut request = last.quote.request.clone();
            request.id = Uuid::new_v4().to_string();
            request.sheet = crate::sheet::extract(&record);
            request.review_confirmed = false;
            request
        } else {
            creation::initial_request(&record)
        };
        Ok(creation::Context {
            revision: self.revision()?,
            record_id: record.id.clone(),
            record_name: record.name.clone(),
            request,
            history,
            editable: creation::editable(&record),
        })
    }
    pub fn quote_vampire_creation(
        &self,
        id: &str,
        request: &creation::Request,
        expected: i64,
        role: Role,
    ) -> Result<creation::Quote> {
        require_st(role)?;
        let (library_id, revision): (String, i64) =
            self.conn
                .query_row("SELECT id,revision FROM library", [], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })?;
        if revision != expected {
            return Err(error(
                "STALE_REVISION",
                "The library changed; reload creation before reviewing.",
            ));
        }
        let record = self.stored_record(id)?;
        let (profile, _) = super::chronicle_store::read_profile(&self.conn)?;
        let mut quote = creation::quote(&library_id, revision, &record, request)?;
        if let Err(issue) = crate::owbn_bylaws::check_creation(&profile, &record, &quote) {
            quote.ready = false;
            quote.issues.push(issue.message);
        }
        self.prepared_sheet(id, &quote.effective_sheet)?;
        if self.revision()? != revision {
            return Err(error("STALE_REVISION", "Reload before reviewing creation."));
        }
        Ok(quote)
    }
    pub fn commit_vampire_creation(
        &mut self,
        quote: &creation::Quote,
        role: Role,
    ) -> Result<creation::Receipt> {
        self.commit_creation_with(quote, role, |library| {
            library.quote_vampire_creation(&quote.record_id, &quote.request, quote.revision, role)
        })
    }

    fn commit_creation_with(
        &mut self,
        quote: &creation::Quote,
        role: Role,
        requote: impl FnOnce(&Self) -> Result<creation::Quote>,
    ) -> Result<creation::Receipt> {
        require_st(role)?;
        // Permit exact retries without reapplying initial Traits or changing the ledger.
        let current = self.stored_record(&quote.record_id)?;
        let mut history = creation::history(&current)?;
        if let Some(receipt) = history
            .iter()
            .find(|r| r.quote.request.id == quote.request.id)
        {
            if receipt.quote == *quote {
                return Ok(receipt.clone());
            }
            return Err(error(
                "CREATION_ID_CONFLICT",
                "This review identity already belongs to another creation review.",
            ));
        }
        let fresh = requote(self)?;
        if fresh != *quote || !fresh.ready {
            return Err(error(
                "CREATION_NOT_READY",
                "Creation is incomplete or the review was changed. Preview again before applying.",
            ));
        }
        if history.len() >= 64 {
            return Err(error(
                "CREATION_LIMIT",
                "Initial creation history reached its review limit.",
            ));
        }
        let mut updated = self.prepared_sheet(&quote.record_id, &quote.effective_sheet)?;
        if quote.request.create_as_npc {
            crate::owbn_bylaws::set_creation_npc(&mut updated);
        }
        if !quote.published_sources.is_empty() {
            let mut sources = crate::published_reference::history(&updated)?;
            for source in &quote.published_sources {
                sources.push(crate::published_reference::Receipt {
                    quote: source.clone(),
                    committed_revision: quote
                        .revision
                        .checked_add(1)
                        .ok_or_else(|| error("REVISION_LIMIT", "Revision limit reached."))?,
                });
            }
            updated.private_fields.insert(
                crate::published_reference::FIELD.into(),
                serde_json::to_value(sources)?,
            );
            crate::published_reference::history(&updated)?;
        }
        let receipt = creation::Receipt {
            quote: quote.clone(),
            committed_revision: quote
                .revision
                .checked_add(1)
                .ok_or_else(|| error("REVISION_LIMIT", "Revision limit reached."))?,
            managed_signature: creation::signature_for_quote(
                quote,
                &crate::sheet::extract(&updated),
            )?,
        };
        history.push(receipt.clone());
        let value = serde_json::to_value(&history)?;
        if serde_json::to_vec(&value)?.len() > 8 * 1024 * 1024 {
            return Err(error(
                "CREATION_LIMIT",
                "Initial creation history exceeds its supported size.",
            ));
        }
        updated.private_fields.insert(creation::FIELD.into(), value);
        updated.private_fields.remove(creation::DRAFT_FIELD);
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let library_id: String = tx.query_row("SELECT id FROM library", [], |r| r.get(0))?;
        let data: String = tx.query_row(
            "SELECT data FROM records WHERE id=?1",
            [&quote.record_id],
            |r| r.get(0),
        )?;
        if library_id != quote.library_id
            || crate::purchases::record_hash(&serde_json::from_str(&data)?)? != quote.record_hash
        {
            return Err(error(
                "CREATION_STALE",
                "Creation belongs to another library or a changed character.",
            ));
        }
        advance(&tx, quote.revision)?;
        tx.execute(
            "UPDATE records SET data=?1 WHERE id=?2",
            params![serde_json::to_string(&updated)?, quote.record_id],
        )?;
        audit(
            &tx,
            "vampire_creation_apply",
            Some(&quote.record_id),
            quote.revision + 1,
        )?;
        tx.commit()?;
        Ok(receipt)
    }
    pub fn save_vampire_creation_draft(
        &mut self,
        id: &str,
        request: &creation::Request,
        expected: i64,
        role: Role,
    ) -> Result<i64> {
        require_st(role)?;
        creation::validate_request(request)?;
        let mut record = self.stored_record(id)?;
        creation::scope(&record)?;
        if !creation::editable(&record) {
            return Err(error(
                "CREATION_CLOSED",
                "Creation drafts are closed after XP opening or leaving Draft status.",
            ));
        }
        let library_id: String = self
            .conn
            .query_row("SELECT id FROM library", [], |r| r.get(0))?;
        if let Some(saved) = creation::draft(&record)?
            && (saved.library_id != library_id || saved.base_hash != creation::base_hash(&record)?)
        {
            return Err(error(
                "CREATION_DRAFT_STALE",
                "Preserve your draft notes, then explicitly discard the saved draft before starting from the changed sheet.",
            ));
        }
        let draft = creation::Draft {
            library_id,
            record_id: id.into(),
            base_hash: creation::base_hash(&record)?,
            request: request.clone(),
        };
        record
            .private_fields
            .insert(creation::DRAFT_FIELD.into(), serde_json::to_value(draft)?);
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        advance(&tx, expected)?;
        tx.execute(
            "UPDATE records SET data=?1 WHERE id=?2",
            params![serde_json::to_string(&record)?, id],
        )?;
        audit(&tx, "vampire_creation_draft", Some(id), expected + 1)?;
        tx.commit()?;
        Ok(expected + 1)
    }
    pub fn discard_vampire_creation_draft(
        &mut self,
        id: &str,
        expected: i64,
        role: Role,
    ) -> Result<i64> {
        require_st(role)?;
        let mut record = self.stored_record(id)?;
        creation::scope(&record)?;
        if !creation::editable(&record) {
            return Err(error(
                "CREATION_CLOSED",
                "Creation is closed after XP opening or leaving Draft status.",
            ));
        }
        record.private_fields.remove(creation::DRAFT_FIELD);
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        advance(&tx, expected)?;
        tx.execute(
            "UPDATE records SET data=?1 WHERE id=?2",
            params![serde_json::to_string(&record)?, id],
        )?;
        audit(&tx, "vampire_creation_discard", Some(id), expected + 1)?;
        tx.commit()?;
        Ok(expected + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{creature_profiles, pricing::SourceCitation};

    #[test]
    fn reviewed_configured_creation_commits_starting_resources_and_visibility() {
        let temp = tempfile::tempdir().unwrap();
        let mut library =
            Library::create(&temp.path().join("creation.gvlib"), "Synthetic").unwrap();
        let role = Role::Storyteller;
        let id = library
            .create_character("Wraith", "Synthetic", 0, role)
            .unwrap();
        let mut profile = creature_profiles::profile("Wraith").unwrap().clone();
        profile.rules_reviewed = true;
        profile.cost_source = Some(SourceCitation {
            book: "Synthetic rules".into(),
            edition: "Fixture".into(),
            system: "Synthetic".into(),
            printed_page: 1,
            pdf_page_1based: 2,
            original_pdf_sha256: "a".repeat(64),
        });
        profile.attribute_priorities = [0; 3];
        profile.ability_base = 0;
        profile.background_base = 0;
        profile.basic_power_base = 0;
        profile.free_traits_base = 5;
        profile.free_trait_costs = Some(creature_profiles::FreeTraitCosts {
            attribute: 1,
            ability: 1,
            background: 1,
            primary_power: 1,
            secondary_power: 1,
            specialization: 1,
            permanent_resource: 2,
        });
        profile.starting_resources = [("Willpower".into(), 3), ("Pathos".into(), 4)].into();
        profile.secondary_power_tier_costs = Some(
            [
                ("Basic".into(), 4),
                ("Intermediate".into(), 6),
                ("Advanced".into(), 9),
                ("in_affiliation_adj".into(), 0),
                ("out_of_affiliation_adj".into(), 1),
            ]
            .into(),
        );
        let mut request = library.vampire_creation_context(&id, role).unwrap().request;
        request.standard_pools = true;
        request.review_confirmed = true;
        request.actor = "Synthetic reviewer".into();
        request.day = "2026-11-17".into();
        request.reason = "Synthetic creation".into();
        request.policy_version = "fixture-v1".into();
        request.review_reference = "Synthetic review".into();
        request.manual_power_reference = "Synthetic initial powers".into();
        request.budget.willpower_free = 1;
        let requote = |lib: &Library| {
            creation::quote_configured_profile(
                &lib.library_id()?,
                lib.revision()?,
                &lib.stored_record(&id)?,
                &request,
                &profile,
            )
        };
        let quote = requote(&library).unwrap();
        assert!(quote.ready, "{:?}", quote.issues);
        assert_eq!(
            quote.configured_budget.as_ref().unwrap().free_spent,
            Some(2)
        );
        library.commit_creation_with(&quote, role, requote).unwrap();
        let stored = library.stored_record(&id).unwrap();
        assert_eq!(stored.public_fields["Willpower"], 4);
        assert_eq!(stored.public_fields["Pathos"], 4);
        assert!(stored.xp_ledger.is_none());
        assert_eq!(creation::history(&stored).unwrap().len(), 1);
    }
}
