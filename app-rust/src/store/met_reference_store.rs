use super::*;
use crate::met_references as met;
impl Library {
    pub fn met_reference_context(&self, id: &str, role: Role) -> Result<met::Context> {
        self.met_reference_context_for_list(id, None, role)
    }
    pub fn met_reference_context_for_list(
        &self,
        id: &str,
        target_list: Option<&str>,
        role: Role,
    ) -> Result<met::Context> {
        require_st(role)?;
        let record = self.stored_record(id)?;
        if !crate::sheet::is_character_kind(&record.kind) || record.build.is_some() {
            return Err(error(
                "MET_REFERENCE_SCOPE",
                "Choose an original Vampire; copies inherit applicable source mappings.",
            ));
        }
        let field = crate::met_references::target_field(&record, target_list)?;
        Ok(met::Context {
            target_list: field.clone(),
            revision: self.revision()?,
            record_id: record.id.clone(),
            record_name: record.name.clone(),
            traits: crate::purchases::power_list(&record, &field)?["Traits"]
                .as_array()
                .cloned()
                .unwrap_or_default(),
            history: met::history(&record)?,
        })
    }
    pub fn quote_met_reference(
        &self,
        id: &str,
        request: &met::Request,
        expected: i64,
        role: Role,
    ) -> Result<met::Quote> {
        self.quote_met_reference_for_list(id, request, expected, None, role)
    }
    pub fn quote_met_reference_for_list(
        &self,
        id: &str,
        request: &met::Request,
        expected: i64,
        target_list: Option<&str>,
        role: Role,
    ) -> Result<met::Quote> {
        require_st(role)?;
        let (library_id,revision,data):(String,i64,String)=self.conn.query_row("SELECT library.id,library.revision,records.data FROM library JOIN records ON records.id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
        if expected != revision {
            return Err(error(
                "STALE_REVISION",
                "Reload before reviewing the MET reminder mapping.",
            ));
        }
        met::quote_for_list(
            &library_id,
            revision,
            &serde_json::from_str(&data)?,
            request,
            target_list,
        )
    }
    pub fn commit_met_reference(&mut self, quote: &met::Quote, role: Role) -> Result<met::Receipt> {
        require_st(role)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (library_id, revision): (String, i64) =
            tx.query_row("SELECT id,revision FROM library", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?;
        if library_id != quote.library_id {
            return Err(error(
                "MET_REFERENCE_LIBRARY",
                "This review belongs to another library.",
            ));
        }
        let data: String = tx.query_row(
            "SELECT data FROM records WHERE id=?1",
            [&quote.record_id],
            |r| r.get(0),
        )?;
        let mut record: Record = serde_json::from_str(&data)?;
        let mut history = met::history(&record)?;
        if let Some(saved) = history
            .iter()
            .find(|r| r.quote.request.id == quote.request.id)
        {
            if saved.quote == *quote {
                return Ok(saved.clone());
            }
            return Err(error(
                "MET_REFERENCE_ID_CONFLICT",
                "This review ID already identifies another mapping.",
            ));
        }
        if revision != quote.revision
            || met::quote_for_list(
                &library_id,
                revision,
                &record,
                &quote.request,
                quote.target_list.as_deref(),
            )? != *quote
        {
            return Err(error(
                "STALE_REVISION",
                "The character or review changed; preview the MET mapping again.",
            ));
        }
        let field = crate::met_references::target_field(&record, quote.target_list.as_deref())?;
        let mut list = crate::purchases::power_list(&record, &field)?;
        list["Traits"][quote.request.trait_index] = quote.updated_trait.clone();
        crate::sheet::write_field(&mut record, &field, list);
        let receipt = met::Receipt {
            quote: quote.clone(),
            committed_revision: revision
                .checked_add(1)
                .ok_or_else(|| error("REVISION_LIMIT", "Revision limit reached."))?,
        };
        history.push(receipt.clone());
        record
            .private_fields
            .insert(met::FIELD.into(), serde_json::to_value(history)?);
        met::history(&record)?;
        advance(&tx, revision)?;
        tx.execute(
            "UPDATE records SET data=?1 WHERE id=?2",
            params![serde_json::to_string(&record)?, record.id],
        )?;
        audit(&tx, "met_reminder_mapping", Some(&record.id), revision + 1)?;
        tx.commit()?;
        Ok(receipt)
    }
}

#[cfg(test)]
mod secondary_list_tests {
    use super::*;
    use crate::met_references::{Request, TRAIT_FIELD};
    use serde_json::json;

    #[test]
    fn secondary_mapping_removal_is_list_bound_atomic_and_keeps_primary_receipt_wire() {
        let dir = tempfile::tempdir().unwrap();
        let mut lib =
            Library::create(&dir.path().join("secondary-reference.gvlib"), "Synthetic").unwrap();
        let id = lib
            .create_character("Vampire", "Synthetic", 0, Role::Storyteller)
            .unwrap();
        let mut edit = lib.sheet_edit(&id, Role::Storyteller).unwrap();
        let row = json!({"Name":"Synthetic reminder","Value":"basic",TRAIT_FIELD:{"review_id":"old"},"AppPublishedReference":{"id":"old"}});
        edit.lists.insert(
            "RitualList".into(),
            json!({"Name":"Rituals","Traits":[row.clone()]}),
        );
        edit.lists.insert(
            "DisciplineList".into(),
            json!({"Name":"Disciplines","Traits":[row]}),
        );
        lib.save_sheet(&id, &edit, lib.revision().unwrap(), Role::Storyteller)
            .unwrap();
        let request = Request {
            day: "2026-01-01".into(),
            actor: "Synthetic".into(),
            reason: "Synthetic".into(),
            review_reference: "Synthetic".into(),
            confirmed: true,
            clear: true,
            ..Request::default()
        };
        let revision = lib.revision().unwrap();
        let old = lib
            .quote_met_reference(&id, &request, revision, Role::Storyteller)
            .unwrap();
        let wire = serde_json::to_value(&old).unwrap();
        assert!(wire.get("target_list").is_none());
        assert_eq!(
            serde_json::to_value(serde_json::from_value::<met::Quote>(wire.clone()).unwrap())
                .unwrap(),
            wire
        );
        let mut secondary = lib
            .quote_met_reference_for_list(
                &id,
                &request,
                revision,
                Some("RitualList"),
                Role::Storyteller,
            )
            .unwrap();
        assert_eq!(secondary.target_list.as_deref(), Some("RitualList"));
        secondary.target_list = Some("GiftList".into());
        assert!(
            lib.commit_met_reference(&secondary, Role::Storyteller)
                .is_err()
        );
        assert_eq!(lib.revision().unwrap(), revision);
        secondary.target_list = Some("RitualList".into());
        let receipt = lib
            .commit_met_reference(&secondary, Role::Storyteller)
            .unwrap();
        assert_eq!(
            lib.commit_met_reference(&secondary, Role::Storyteller)
                .unwrap(),
            receipt
        );
        let primary = lib.met_reference_context(&id, Role::Storyteller).unwrap();
        let secondary = lib
            .met_reference_context_for_list(&id, Some("RitualList"), Role::Storyteller)
            .unwrap();
        assert!(primary.traits[0].get(TRAIT_FIELD).is_some());
        assert!(secondary.traits[0].get(TRAIT_FIELD).is_none());
        let published_request = crate::published_reference::Request {
            id: uuid::Uuid::new_v4().to_string(),
            trait_index: 0,
            source_id: String::new(),
            day: "2026-01-02".into(),
            actor: "Synthetic".into(),
            reason: "Synthetic".into(),
            clear: true,
            rename_to_source: false,
            confirm_alias: false,
            confirm_index_evidence: false,
        };
        let published_primary = lib
            .quote_published_reference(
                &id,
                &published_request,
                lib.revision().unwrap(),
                Role::Storyteller,
            )
            .unwrap();
        assert!(
            serde_json::to_value(&published_primary)
                .unwrap()
                .get("target_list")
                .is_none()
        );
        let published_secondary = lib
            .quote_published_reference_for_list(
                &id,
                &published_request,
                lib.revision().unwrap(),
                Some("RitualList"),
                Role::Storyteller,
            )
            .unwrap();
        assert_eq!(
            published_secondary.target_list.as_deref(),
            Some("RitualList")
        );
        let published_receipt = lib
            .commit_published_reference(&published_secondary, Role::Storyteller)
            .unwrap();
        assert_eq!(
            lib.commit_published_reference(&published_secondary, Role::Storyteller)
                .unwrap(),
            published_receipt
        );
        assert!(
            lib.published_reference_context(&id, Role::Storyteller)
                .unwrap()
                .traits[0]
                .get("AppPublishedReference")
                .is_some()
        );
        assert!(
            lib.published_reference_context_for_list(&id, Some("RitualList"), Role::Storyteller)
                .unwrap()
                .traits[0]
                .get("AppPublishedReference")
                .is_none()
        );
        assert!(
            lib.met_reference_context_for_list(&id, Some("GiftList"), Role::Storyteller)
                .is_err()
        );
    }
}
