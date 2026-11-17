use super::*;
use crate::boon_book;

fn live_vampire(record: &Record) -> Result<()> {
    if record.kind == "Vampire" && record.build.is_none() {
        Ok(())
    } else {
        Err(error(
            "BOON_HOLDER_REQUIRED",
            "Select a live Vampire holder.",
        ))
    }
}

fn read_record(tx: &rusqlite::Transaction<'_>, id: &str) -> Result<Record> {
    let data: Option<String> = tx
        .query_row("SELECT data FROM records WHERE id=?1", [id], |row| {
            row.get(0)
        })
        .optional()?;
    serde_json::from_str(&data.ok_or_else(|| error("NOT_FOUND", "Record no longer exists."))?)
        .map_err(Into::into)
}

fn store_state(
    tx: &rusqlite::Transaction<'_>,
    record: &mut Record,
    state: &boon_book::State,
    operation: &str,
    expected: i64,
) -> Result<i64> {
    record
        .private_fields
        .insert(boon_book::FIELD.into(), serde_json::to_value(state)?);
    advance(tx, expected)?;
    if tx.execute(
        "UPDATE records SET data=?1 WHERE id=?2",
        params![serde_json::to_string(record)?, record.id],
    )? != 1
    {
        return Err(error("NOT_FOUND", "The Vampire no longer exists."));
    }
    audit(tx, operation, Some(&record.id), expected + 1)?;
    Ok(expected + 1)
}

impl Library {
    pub fn boon_snapshot(&self, holder: &str, role: Role) -> Result<boon_book::Snapshot> {
        require_st(role)?;
        let revision = self.revision()?;
        let record = self.stored_record(holder)?;
        live_vampire(&record)?;
        let state = boon_book::state(&record)?;
        if self.revision()? != revision {
            return Err(error(
                "STALE_REVISION",
                "The library changed while reading this Boon list. Reopen it.",
            ));
        }
        Ok(boon_book::snapshot(&record, revision, state))
    }

    pub fn add_boon(
        &mut self,
        holder: &str,
        edit: &boon_book::Edit,
        expected: i64,
        role: Role,
    ) -> Result<String> {
        require_st(role)?;
        boon_book::validate_new(edit)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut record = read_record(&tx, holder)?;
        live_vampire(&record)?;
        let mut state = boon_book::state(&record)?;
        if state.entries.len().saturating_add(state.removed.len()) >= 2000 {
            return Err(error(
                "BOON_LIMIT",
                "This Vampire's Boon list has reached the 2,000-entry limit.",
            ));
        }
        let id = loop {
            let candidate = Uuid::new_v4().to_string();
            if state
                .entries
                .iter()
                .chain(state.removed.iter())
                .all(|entry| entry.id != candidate)
            {
                break candidate;
            }
        };
        state
            .entries
            .push(boon_book::Entry::new(id.clone(), edit.clone(), None));
        store_state(&tx, &mut record, &state, "create_boon", expected)?;
        tx.commit()?;
        Ok(id)
    }

    pub fn edit_boon(
        &mut self,
        holder: &str,
        entry_id: &str,
        edit: &boon_book::Edit,
        expected: i64,
        role: Role,
    ) -> Result<i64> {
        require_st(role)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut record = read_record(&tx, holder)?;
        live_vampire(&record)?;
        let mut state = boon_book::state(&record)?;
        let entry = state
            .entries
            .iter_mut()
            .find(|entry| entry.id == entry_id)
            .ok_or_else(|| error("BOON_NOT_FOUND", "The Boon is no longer in this list."))?;
        boon_book::validate_change(&entry.edit, edit)?;
        entry.edit.clone_from(edit);
        store_state(&tx, &mut record, &state, "edit_boon", expected)?;
        tx.commit()?;
        Ok(expected + 1)
    }

    pub fn remove_boon(
        &mut self,
        holder: &str,
        entry_id: &str,
        expected: i64,
        role: Role,
    ) -> Result<i64> {
        require_st(role)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut record = read_record(&tx, holder)?;
        live_vampire(&record)?;
        let mut state = boon_book::state(&record)?;
        let index = state
            .entries
            .iter()
            .position(|entry| entry.id == entry_id)
            .ok_or_else(|| error("BOON_NOT_FOUND", "The Boon is no longer in this list."))?;
        let removed = state.entries.remove(index);
        state.removed.push(removed);
        store_state(&tx, &mut record, &state, "remove_boon", expected)?;
        tx.commit()?;
        Ok(expected + 1)
    }
}
