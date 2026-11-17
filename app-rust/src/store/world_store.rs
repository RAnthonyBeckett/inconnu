use super::*;

impl Library {
    pub fn world_edit(&self, id: &str, role: Role) -> Result<crate::world::Edit> {
        require_st(role)?;
        let record = self.stored_record(id)?;
        if !crate::world::is_world_kind(&record.kind) {
            return Err(error(
                "WORLD_KIND_REQUIRED",
                "Select an item, location or rote record.",
            ));
        }
        if record.build.is_some() {
            return Err(error(
                "WORLD_BUILD_READ_ONLY",
                "Build copies are not editable as world records.",
            ));
        }
        Ok(world_projection(&record))
    }

    pub fn create_world(
        &mut self,
        kind: &str,
        edit: &crate::world::Edit,
        expected: i64,
        role: Role,
    ) -> Result<String> {
        require_st(role)?;
        crate::world::validate_edit(kind, edit)?;
        let name = edit.name.trim().to_owned();
        let kind = kind.to_owned();
        let id = Uuid::new_v4().to_string();
        let mut record = Record {
            id: id.clone(),
            kind: kind.clone(),
            name: name.clone(),
            player_id: None,
            legacy_id: None,
            public_fields: Map::new(),
            private_fields: Map::new(),
            original: json!({"created_in_app": true}),
            source_sha256: String::new(),
            st_markers: None,
            build: None,
            xp_ledger: None,
        };
        store_field(&mut record, "Name", json!(name));
        for (key, value) in &edit.fields {
            store_typed_field(&mut record, &kind, key, value.clone());
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        advance(&tx, expected)?;
        insert(&tx, &record)?;
        audit(&tx, "create_world", Some(&id), expected + 1)?;
        tx.commit()?;
        Ok(id)
    }

    pub fn save_world(
        &mut self,
        id: &str,
        edit: &crate::world::Edit,
        expected: i64,
        role: Role,
    ) -> Result<i64> {
        require_st(role)?;
        let mut record = self.stored_record(id)?;
        if !crate::world::is_world_kind(&record.kind) {
            return Err(error(
                "WORLD_KIND_REQUIRED",
                "Select an item, location or rote record.",
            ));
        }
        if record.build.is_some() {
            return Err(error(
                "WORLD_BUILD_READ_ONLY",
                "Build copies are not editable as world records.",
            ));
        }
        crate::world::validate_edit(&record.kind, edit)?;
        record.name = edit.name.trim().to_owned();
        let name = record.name.clone();
        store_field(&mut record, "Name", json!(name));
        let kind = record.kind.clone();
        for (key, value) in &edit.fields {
            store_typed_field(&mut record, &kind, key, value.clone());
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        advance(&tx, expected)?;
        tx.execute(
            "UPDATE records SET name=?1,data=?2 WHERE id=?3",
            params![record.name, serde_json::to_string(&record)?, id],
        )?;
        audit(&tx, "edit_world", Some(id), expected + 1)?;
        tx.commit()?;
        Ok(expected + 1)
    }
}

fn store_typed_field(record: &mut Record, kind: &str, key: &str, value: Value) {
    if key == "SphereList" && kind == "Rote" {
        store_public_field(record, key, value);
    } else {
        record.public_fields.remove(key);
        record.private_fields.insert(key.into(), value);
    }
}

fn store_field(record: &mut Record, key: &str, value: Value) {
    store_public_field(record, key, value);
}

fn store_public_field(record: &mut Record, key: &str, value: Value) {
    let mut safe = importer::redact(&value);
    if let Some((start, end)) = &record.st_markers {
        safe = importer::redact_custom(&safe, start, end);
    }
    record.public_fields.insert(key.into(), safe.clone());
    if safe == value {
        record.private_fields.remove(key);
    } else {
        record.private_fields.insert(key.into(), value);
    }
}

pub(super) fn world_projection(record: &Record) -> crate::world::Edit {
    let mut fields = record.public_fields.clone();
    fields.extend(record.private_fields.clone());
    // Older libraries kept some XML aliases only in the raw subtree. Expose a
    // corrected draft without rewriting the record or overriding saved edits.
    if let Some(xml) = record.original.get("xml") {
        let old = importer::xml_fields(xml);
        for (key, value) in importer::normalized_world_xml_fields(&record.kind, xml) {
            if crate::world::value_supported(&record.kind, &key, &value)
                && fields.get(&key).is_none_or(|stored| {
                    !crate::world::value_supported(&record.kind, &key, stored)
                        && old.get(&key) == Some(stored)
                })
            {
                fields.insert(key, value);
            }
        }
    }
    let fields = fields
        .into_iter()
        .filter(|(key, value)| crate::world::value_supported(&record.kind, key, value))
        .collect();
    crate::world::Edit {
        name: record.name.clone(),
        fields,
    }
}
