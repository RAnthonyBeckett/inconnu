use super::*;
use crate::associations::{self, Link, Resolved, Snapshot};

fn live_character(record: &Record) -> Result<()> {
    if crate::sheet::is_character_kind(&record.kind) && record.build.is_none() {
        Ok(())
    } else {
        Err(error(
            "ASSOCIATION_CHARACTER_REQUIRED",
            "Select a live character. Draft and test copies do not receive assignments.",
        ))
    }
}

fn saved_links(record: &Record) -> Result<Vec<Link>> {
    record
        .private_fields
        .get(associations::FIELD)
        .map(|v| serde_json::from_value(v.clone()).map_err(Into::into))
        .transpose()
        .map(|links| links.unwrap_or_default())
}

fn read_in_transaction(tx: &rusqlite::Transaction<'_>, id: &str) -> Result<Option<Record>> {
    let value: Option<String> = tx
        .query_row("SELECT data FROM records WHERE id=?1", [id], |row| {
            row.get(0)
        })
        .optional()?;
    value
        .map(|v| serde_json::from_str(&v).map_err(Into::into))
        .transpose()
}

impl Library {
    pub fn association_snapshot(&self, character_id: &str, role: Role) -> Result<Snapshot> {
        require_st(role)?;
        let revision = self.revision()?;
        let record = self.stored_record(character_id)?;
        live_character(&record)?;
        let (legacy, mut retained) = associations::legacy(&record);
        let mut remaining_detail_bytes = 8 * 1024 * 1024;
        let mut linked = Vec::new();
        for link in saved_links(&record)? {
            let current = match self.stored_record(&link.target_id) {
                Ok(target) if target.kind == link.kind.record_kind() && target.build.is_none() => {
                    let mut edit = super::world_store::world_projection(&target);
                    edit.fields.retain(|key, value| {
                        let bytes = serde_json::to_vec(value)
                            .map_or(usize::MAX, |v| v.len().saturating_add(key.len()));
                        if bytes <= remaining_detail_bytes {
                            remaining_detail_bytes -= bytes;
                            true
                        } else {
                            retained += 1;
                            false
                        }
                    });
                    Some(edit)
                }
                Ok(_) => None,
                Err(e) if e.code == "NOT_FOUND" => None,
                Err(e) => return Err(e),
            };
            linked.push(Resolved { link, current });
        }
        if self.revision()? != revision {
            return Err(error(
                "STALE_REVISION",
                "The library changed while reading assignments. Reopen this character.",
            ));
        }
        Ok(Snapshot {
            character_id: character_id.into(),
            character_name: record.name,
            revision,
            linked,
            legacy,
            retained,
        })
    }

    pub fn save_associations(
        &mut self,
        character_id: &str,
        links: &[Link],
        expected: i64,
        role: Role,
    ) -> Result<i64> {
        require_st(role)?;
        associations::validate(links)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        advance(&tx, expected)?;
        let mut character = read_in_transaction(&tx, character_id)?
            .ok_or_else(|| error("NOT_FOUND", "The character no longer exists."))?;
        live_character(&character)?;
        let previous = saved_links(&character)?;
        let mut bindings = Vec::with_capacity(links.len());
        for input in links {
            let old = previous
                .iter()
                .find(|l| l.target_id == input.target_id && l.kind == input.kind);
            let mut link = input.clone();
            match read_in_transaction(&tx, &link.target_id)? {
                Some(target)
                    if target.kind == link.kind.record_kind() && target.build.is_none() =>
                {
                    // A rename does not change identity or the historical binding label.
                    link.label_at_binding = old.map_or(target.name, |l| l.label_at_binding.clone());
                }
                _ if old == Some(input) => {}
                _ => {
                    return Err(error(
                        "ASSOCIATION_TARGET_REQUIRED",
                        "Choose an existing target of the matching type. Missing saved targets can be kept unchanged or removed.",
                    ));
                }
            }
            bindings.push(link);
        }
        associations::validate(&bindings)?;
        character
            .private_fields
            .insert(associations::FIELD.into(), serde_json::to_value(bindings)?);
        tx.execute(
            "UPDATE records SET data=?1 WHERE id=?2",
            params![serde_json::to_string(&character)?, character_id],
        )?;
        audit(
            &tx,
            "replace_character_associations",
            Some(character_id),
            expected + 1,
        )?;
        tx.commit()?;
        Ok(expected + 1)
    }
}
