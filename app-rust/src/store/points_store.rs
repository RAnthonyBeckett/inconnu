use super::*;
use crate::player_points::{self, Baseline, Command, Ledger};
pub(super) const FIELD: &str = "_AppPlayerPointsV1";
fn player(record: &Record) -> Result<()> {
    if record.kind != "Player" || record.build.is_some() {
        return Err(error(
            "PLAYER_POINTS_SCOPE",
            "Select a Player record; character XP is a separate pool.",
        ));
    }
    Ok(())
}
fn ledger(record: &Record) -> Result<Option<Ledger>> {
    player(record)?;
    let ledger: Option<Ledger> = record
        .private_fields
        .get(FIELD)
        .map(|v| serde_json::from_value(v.clone()))
        .transpose()?;
    if let Some(ref value) = ledger {
        player_points::replay(value)?;
    }
    Ok(ledger)
}
impl Library {
    pub fn create_player(&mut self, name: &str, expected: i64, role: Role) -> Result<String> {
        require_st(role)?;
        if name.trim().is_empty() || name.len() > 4096 {
            return Err(error("EMPTY_NAME", "Enter a bounded Player name."));
        }
        let id = Uuid::new_v4().to_string();
        let record = Record {
            id: id.clone(),
            kind: "Player".into(),
            name: name.trim().into(),
            player_id: None,
            legacy_id: None,
            public_fields: serde_json::from_value(
                json!({"Name":importer::redact(&json!(name.trim()))}),
            )?,
            private_fields: Map::new(),
            original: json!({"created_in_app":true}),
            source_sha256: String::new(),
            st_markers: None,
            build: None,
            xp_ledger: None,
        };
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        advance(&tx, expected)?;
        insert(&tx, &record)?;
        audit(&tx, "create_player", Some(&id), expected + 1)?;
        tx.commit()?;
        Ok(id)
    }
    pub fn player_points(&self, id: &str, role: Role) -> Result<Option<Ledger>> {
        require_st(role)?;
        ledger(&self.stored_record(id)?)
    }
    pub fn initialize_player_points(
        &mut self,
        id: &str,
        input: &Baseline,
        expected: i64,
        role: Role,
    ) -> Result<i64> {
        require_st(role)?;
        let opened = player_points::open(input.clone())?;
        self.write_player_points(id, expected, role, |old| match old {
            Some(old) if old.baseline == opened.baseline => Ok(old),
            Some(_) => Err(error(
                "PLAYER_POINTS_ALREADY_OPEN",
                "Retain the opening; use dated corrections.",
            )),
            None => Ok(opened),
        })
    }
    pub fn post_player_points(
        &mut self,
        id: &str,
        input: &Command,
        expected: i64,
        role: Role,
    ) -> Result<i64> {
        self.write_player_points(id, expected, role, |old| {
            player_points::apply(
                &old.ok_or_else(|| {
                    error(
                        "PLAYER_POINTS_OPENING_REQUIRED",
                        "Declare a Player points opening first.",
                    )
                })?,
                input,
            )
        })
    }
    fn write_player_points(
        &mut self,
        id: &str,
        expected: i64,
        role: Role,
        update: impl FnOnce(Option<Ledger>) -> Result<Ledger>,
    ) -> Result<i64> {
        require_st(role)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let data: String = tx
            .query_row("SELECT data FROM records WHERE id=?1", [id], |r| r.get(0))
            .optional()?
            .ok_or_else(|| error("NOT_FOUND", "The Player record no longer exists."))?;
        let mut record: Record = serde_json::from_str(&data)?;
        let old = ledger(&record)?;
        let updated = update(old.clone())?;
        if old.as_ref() == Some(&updated) {
            return tx
                .query_row("SELECT revision FROM library", [], |r| r.get(0))
                .map_err(Into::into);
        }
        player_points::replay(&updated)?;
        record
            .private_fields
            .insert(FIELD.into(), serde_json::to_value(updated)?);
        advance(&tx, expected)?;
        tx.execute(
            "UPDATE records SET data=?1 WHERE id=?2",
            params![serde_json::to_string(&record)?, id],
        )?;
        audit(&tx, "player_points_post", Some(id), expected + 1)?;
        tx.commit()?;
        Ok(expected + 1)
    }
}
