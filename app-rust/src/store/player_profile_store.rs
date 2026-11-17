use super::*;
use crate::player_profiles::{self, Profile, Snapshot};

fn require_player(record: &Record) -> Result<()> {
    if record.kind != "Player" || record.build.is_some() {
        return Err(error(
            "PLAYER_PROFILE_REQUIRED",
            "Select a live Player profile.",
        ));
    }
    Ok(())
}
fn projection(record: &Record) -> Profile {
    let text = |key| {
        record
            .private_fields
            .get(key)
            .or_else(|| record.public_fields.get(key))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    Profile {
        name: record.name.clone(),
        position: text("Position"),
        status: text("Status"),
        email: text("EMail"),
        phone: text("Phone"),
        address: text("Address"),
        notes: record
            .private_fields
            .get("AppNotes")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| text("Notes")),
    }
}
fn apply(record: &mut Record, p: &Profile) {
    record.name = p.name.trim().into();
    let name = json!(record.name);
    let mut safe = importer::redact(&name);
    if let Some((a, b)) = &record.st_markers {
        safe = importer::redact_custom(&safe, a, b);
    }
    record.public_fields.insert("Name".into(), safe.clone());
    if safe == name {
        record.private_fields.remove("Name");
    } else {
        record.private_fields.insert("Name".into(), name);
    }
    for (key, value) in [("Position", &p.position), ("Status", &p.status)] {
        set_sheet_field(record, key, json!(value));
    }
    for (key, value) in [
        ("EMail", &p.email),
        ("Phone", &p.phone),
        ("Address", &p.address),
        ("AppNotes", &p.notes),
    ] {
        record.public_fields.remove(key);
        record.private_fields.insert(key.into(), json!(value));
    }
}
impl Library {
    pub fn player_profile(&self, id: &str, role: Role) -> Result<Snapshot> {
        require_st(role)?;
        let revision = self.revision()?;
        let record = self.stored_record(id)?;
        require_player(&record)?;
        let profile = projection(&record);
        let mut linked_characters: Vec<_> = self
            .records()?
            .into_iter()
            .filter(|r| {
                r.build.is_none()
                    && crate::sheet::is_character_kind(&r.kind)
                    && r.player_id.as_deref() == Some(id)
            })
            .map(|r| (r.id, r.name))
            .collect();
        linked_characters.sort_by(|a, b| a.1.cmp(&b.1));
        if self.revision()? != revision {
            return Err(error(
                "STALE_REVISION",
                "The library changed. Reopen this profile.",
            ));
        }
        Ok(Snapshot {
            id: id.into(),
            revision,
            legacy_id: record.legacy_id,
            profile,
            linked_characters,
        })
    }
    pub fn save_player_profile(
        &mut self,
        id: &str,
        p: &Profile,
        expected: i64,
        role: Role,
    ) -> Result<i64> {
        require_st(role)?;
        player_profiles::validate(p)?;
        let mut record = self.stored_record(id)?;
        require_player(&record)?;
        apply(&mut record, p);
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        advance(&tx, expected)?;
        tx.execute(
            "UPDATE records SET name=?1,data=?2 WHERE id=?3",
            params![record.name, serde_json::to_string(&record)?, id],
        )?;
        audit(&tx, "edit_player_profile", Some(id), expected + 1)?;
        tx.commit()?;
        Ok(expected + 1)
    }
    pub fn create_player_profile(
        &mut self,
        p: &Profile,
        expected: i64,
        role: Role,
    ) -> Result<String> {
        require_st(role)?;
        player_profiles::validate(p)?;
        let id = Uuid::new_v4().to_string();
        let mut record = Record {
            id: id.clone(),
            kind: "Player".into(),
            name: String::new(),
            player_id: None,
            legacy_id: None,
            public_fields: Map::new(),
            private_fields: Map::new(),
            original: json!({"created_in_app":true}),
            source_sha256: String::new(),
            st_markers: None,
            build: None,
            xp_ledger: None,
        };
        apply(&mut record, p);
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        advance(&tx, expected)?;
        insert(&tx, &record)?;
        audit(&tx, "create_player_profile", Some(&id), expected + 1)?;
        tx.commit()?;
        Ok(id)
    }
}
