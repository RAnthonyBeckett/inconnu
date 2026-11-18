use crate::{diagnostics, importer, model::*};
use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior, params};
use serde_json::{Map, Value, json};
use std::{
    collections::HashSet,
    fs::OpenOptions as FileOptions,
    path::{Path, PathBuf},
    time::Duration,
};
use uuid::Uuid;

const APPLICATION_ID: i64 = 0x47565253;
const SCHEMA_VERSION: i64 = 9;

pub struct Library {
    pub path: PathBuf,
    pub title: String,
    conn: Connection,
}

fn require_st(role: Role) -> Result<()> {
    if role == Role::Storyteller {
        Ok(())
    } else {
        Err(error(
            "ACCESS_DENIED",
            "This operation requires the local library owner / storyteller view.",
        ))
    }
}
fn key(record: &Record) -> String {
    let source_name = record
        .original
        .pointer("/fields/Name")
        .and_then(Value::as_str)
        .or_else(|| {
            record
                .original
                .pointer("/xml/attributes")
                .and_then(Value::as_array)
                .and_then(|a| a.iter().find(|a| a["name"] == "name"))
                .and_then(|a| a["value"].as_str())
        })
        .unwrap_or(&record.name);
    format!(
        "{}/{}",
        record.kind,
        record
            .legacy_id
            .clone()
            .unwrap_or_else(|| source_name.to_lowercase())
    )
}
fn checked_open(path: &Path) -> Result<Connection> {
    let c = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let app: i64 = c.pragma_query_value(None, "application_id", |r| r.get(0))?;
    let version: i64 = c.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if app != APPLICATION_ID {
        return Err(error(
            "NOT_A_LIBRARY",
            "This file is not an Inconnu library.",
        ));
    }
    if ![1, 2, 3, 4, 5, 6, 7, 8, SCHEMA_VERSION].contains(&version) {
        return Err(error(
            "UNSUPPORTED_SCHEMA",
            "This library uses an unsupported schema. It was not migrated or modified.",
        ));
    }
    if version == SCHEMA_VERSION {
        let has_guard:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM pragma_table_info('purchase_write_guard') WHERE name='schema_revision')",[],|r|r.get(0))?;
        if !has_guard {
            return Err(error(
                "INCOMPLETE_SCHEMA",
                "This prerelease library lacks the current writer guard. Use a supported backup or a new import.",
            ));
        }
        let has_profile: bool = c.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='chronicle_profile')",
            [],
            |r| r.get(0),
        )?;
        let has_mixed:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='mixed_advancement_receipts')",[],|r|r.get(0))?;
        if !has_mixed {
            return Err(error(
                "INCOMPLETE_SCHEMA",
                "This library lacks its mixed-plan receipt table.",
            ));
        }
        if !has_profile {
            return Err(error(
                "INCOMPLETE_SCHEMA",
                "This library lacks its current Chronicle profile table. Restore a supported backup.",
            ));
        }
    }
    let integrity: String = c.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
    if integrity != "ok" {
        return Err(error(
            "CORRUPT_LIBRARY",
            "Library integrity check failed. Restore a known-good backup.",
        ));
    }
    c.busy_timeout(Duration::from_secs(2))?;
    c.pragma_update(None, "foreign_keys", true)?;
    Ok(c)
}

fn backup_before_upgrade(conn: &Connection, path: &Path) -> Result<()> {
    let backup_path = path.with_file_name(format!(
        "pre-schema{}-{}.gvbackup",
        SCHEMA_VERSION,
        Uuid::new_v4()
    ));
    FileOptions::new()
        .write(true)
        .create_new(true)
        .open(&backup_path)?;
    let mut destination = Connection::open(&backup_path)?;
    rusqlite::backup::Backup::new(conn, &mut destination)?.run_to_completion(
        64,
        Duration::from_millis(1),
        None,
    )?;
    Ok(())
}

fn install_revision_guard(tx: &rusqlite::Transaction<'_>) -> Result<()> {
    tx.execute_batch(
        "CREATE TRIGGER library_revision_guard_before_update
         BEFORE UPDATE OF revision ON library
         WHEN NEW.revision != COALESCE((SELECT next_revision FROM purchase_write_guard WHERE singleton=1), -1)
           OR (~NEW.revision)+9 != COALESCE((SELECT schema_revision FROM purchase_write_guard WHERE singleton=1), 1)
         BEGIN
             SELECT RAISE(ABORT, 'library revision must be advanced by the current writer');
         END;",
    )?;
    Ok(())
}

fn install_purchase_schema(tx: &rusqlite::Transaction<'_>) -> Result<()> {
    tx.execute_batch(
        "CREATE TABLE purchase_receipts(id TEXT PRIMARY KEY,record_id TEXT NOT NULL,data TEXT NOT NULL);
         CREATE TABLE purchase_write_guard(singleton INTEGER PRIMARY KEY CHECK(singleton=1),next_revision INTEGER NOT NULL,schema_revision INTEGER NOT NULL DEFAULT -1);",
    )?;
    let revision: i64 = tx.query_row("SELECT revision FROM library", [], |r| r.get(0))?;
    let allowed = revision.checked_add(1).ok_or_else(|| {
        error(
            "REVISION_LIMIT",
            "The library revision cannot be advanced further.",
        )
    })?;
    tx.execute(
        "INSERT INTO purchase_write_guard(singleton,next_revision,schema_revision) VALUES(1,?1,?2)",
        params![allowed, (!allowed) + 9],
    )?;
    install_revision_guard(tx)?;
    if tx.execute(
        "UPDATE library SET revision=revision+1 WHERE revision=?1",
        [revision],
    )? != 1
    {
        return Err(error(
            "STALE_REVISION",
            "Library changed while schema4 was being installed.",
        ));
    }
    Ok(())
}

impl Library {
    pub fn create(path: &Path, title: &str) -> Result<Self> {
        if title.trim().is_empty() {
            return Err(error("EMPTY_TITLE", "Enter a library title."));
        }
        if path.exists() {
            return Err(error(
                "FILE_EXISTS",
                "Choose a new filename. Existing files are never overwritten.",
            ));
        }
        // Exclusive filesystem creation avoids clobbering existing/linked files.
        FileOptions::new().write(true).create_new(true).open(path)?;
        let mut conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        conn.busy_timeout(Duration::from_secs(2))?;
        let tx = conn.transaction()?;
        tx.execute_batch("CREATE TABLE library(id TEXT PRIMARY KEY,title TEXT NOT NULL,revision INTEGER NOT NULL); CREATE TABLE records(id TEXT PRIMARY KEY,kind TEXT NOT NULL,name TEXT NOT NULL,data TEXT NOT NULL); CREATE TABLE imports(hash TEXT PRIMARY KEY,source_name TEXT NOT NULL,source_bytes BLOB NOT NULL,report TEXT NOT NULL); CREATE TABLE audit(seq INTEGER PRIMARY KEY,operation TEXT NOT NULL,record_id TEXT,revision INTEGER NOT NULL,time INTEGER NOT NULL DEFAULT(unixepoch())); CREATE TABLE purchase_receipts(id TEXT PRIMARY KEY,record_id TEXT NOT NULL,data TEXT NOT NULL); CREATE TABLE purchase_write_guard(singleton INTEGER PRIMARY KEY CHECK(singleton=1),next_revision INTEGER NOT NULL,schema_revision INTEGER NOT NULL DEFAULT -1);")?;
        tx.execute(
            "INSERT INTO purchase_write_guard(singleton,next_revision,schema_revision) VALUES(1,0,8)",
            [],
        )?;
        tx.execute(
            "INSERT INTO library VALUES(?1,?2,0)",
            params![Uuid::new_v4().to_string(), title.trim()],
        )?;
        install_revision_guard(&tx)?;
        tx.execute_batch("CREATE TABLE game_sessions(id TEXT PRIMARY KEY,data TEXT NOT NULL); CREATE TABLE scheduled_sessions(id TEXT PRIMARY KEY,data TEXT NOT NULL); CREATE TABLE saved_searches(id TEXT PRIMARY KEY,data TEXT NOT NULL); CREATE TABLE chronicle_profile(singleton INTEGER PRIMARY KEY CHECK(singleton=1),data TEXT NOT NULL);")?;
        tx.execute_batch("CREATE TABLE IF NOT EXISTS mixed_advancement_receipts(id TEXT PRIMARY KEY,record_id TEXT NOT NULL,data TEXT NOT NULL);")?;
        tx.pragma_update(None, "application_id", APPLICATION_ID)?;
        tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        tx.commit()?;
        diagnostics::record("create_library", "OK");
        Self::open(path)
    }
    pub fn open(path: &Path) -> Result<Self> {
        let mut conn = checked_open(path)?;
        let opened_version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if opened_version < SCHEMA_VERSION {
            backup_before_upgrade(&conn, path)?;
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let mut schema3_migrated = false;
            let mut schema4_migrated = false;
            let mut version: i64 = tx.pragma_query_value(None, "user_version", |r| r.get(0))?;
            if version >= 4 {
                let mut columns = tx.prepare("PRAGMA table_info(purchase_write_guard)")?;
                let names = columns
                    .query_map([], |row| row.get::<_, String>(1))?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                drop(columns);
                if !names.iter().any(|name| name == "schema_revision") {
                    tx.execute_batch("ALTER TABLE purchase_write_guard ADD COLUMN schema_revision INTEGER NOT NULL DEFAULT -1;")?;
                }
                tx.execute_batch("DROP TRIGGER library_revision_guard_before_update;")?;
                install_revision_guard(&tx)?;
            }
            if version < 3 {
                // Preserve the schema1/2 XP migration while keeping both upgrade
                // revisions and audits atomic with the schema4 installation.
                let current: i64 =
                    tx.query_row("SELECT revision FROM library", [], |r| r.get(0))?;
                let next = current.checked_add(1).ok_or_else(|| {
                    error(
                        "REVISION_LIMIT",
                        "The library revision cannot be advanced further.",
                    )
                })?;
                if tx.execute(
                    "UPDATE library SET revision=?1 WHERE revision=?2",
                    params![next, current],
                )? != 1
                {
                    return Err(error(
                        "STALE_REVISION",
                        "Library changed while its schema was being upgraded.",
                    ));
                }
                tx.pragma_update(None, "user_version", 3)?;
                audit(&tx, "schema3_migration", None, next)?;
                schema3_migrated = true;
                version = 3;
            }
            if version < 4 {
                if version != 3 {
                    return Err(error(
                        "UNSUPPORTED_SCHEMA",
                        "This library could not be upgraded safely.",
                    ));
                }
                install_purchase_schema(&tx)?;
                tx.pragma_update(None, "user_version", 4)?;
                version = 4;
                let revision = tx.query_row("SELECT revision FROM library", [], |r| r.get(0))?;
                audit(&tx, "schema4_migration", None, revision)?;
                schema4_migrated = true;
            }
            if version < 5 {
                tx.execute_batch("CREATE TABLE IF NOT EXISTS game_sessions(id TEXT PRIMARY KEY,data TEXT NOT NULL);")?;
                let current: i64 =
                    tx.query_row("SELECT revision FROM library", [], |r| r.get(0))?;
                advance(&tx, current)?;
                tx.pragma_update(None, "user_version", 5)?;
                audit(&tx, "schema5_migration", None, current + 1)?;
            }
            if version < 6 {
                tx.execute_batch("CREATE TABLE IF NOT EXISTS scheduled_sessions(id TEXT PRIMARY KEY,data TEXT NOT NULL);")?;
                let current: i64 =
                    tx.query_row("SELECT revision FROM library", [], |r| r.get(0))?;
                advance(&tx, current)?;
                tx.pragma_update(None, "user_version", 6)?;
                audit(&tx, "schema6_migration", None, current + 1)?;
            }
            if version < 7 {
                tx.execute_batch("CREATE TABLE IF NOT EXISTS saved_searches(id TEXT PRIMARY KEY,data TEXT NOT NULL);")?;
                let current: i64 =
                    tx.query_row("SELECT revision FROM library", [], |r| r.get(0))?;
                advance(&tx, current)?;
                tx.pragma_update(None, "user_version", 7)?;
                audit(&tx, "schema7_migration", None, current + 1)?;
            }
            if version < 8 {
                tx.execute_batch("CREATE TABLE IF NOT EXISTS chronicle_profile(singleton INTEGER PRIMARY KEY CHECK(singleton=1),data TEXT NOT NULL);")?;
                let current: i64 =
                    tx.query_row("SELECT revision FROM library", [], |r| r.get(0))?;
                advance(&tx, current)?;
                tx.pragma_update(None, "user_version", 8)?;
                audit(&tx, "schema8_migration", None, current + 1)?;
            }
            if version < 9 {
                tx.execute_batch("CREATE TABLE IF NOT EXISTS mixed_advancement_receipts(id TEXT PRIMARY KEY,record_id TEXT NOT NULL,data TEXT NOT NULL);")?;
                let current: i64 =
                    tx.query_row("SELECT revision FROM library", [], |r| r.get(0))?;
                advance(&tx, current)?;
                tx.pragma_update(None, "user_version", 9)?;
                audit(&tx, "schema9_migration", None, current + 1)?;
            }
            tx.commit()?;
            if schema3_migrated {
                diagnostics::record("schema3_migration", "OK");
            }
            if schema4_migrated {
                diagnostics::record("schema4_migration", "OK");
            }
        }
        let title = conn.query_row("SELECT title FROM library", [], |r| r.get(0))?;
        Ok(Self {
            path: path.to_path_buf(),
            title,
            conn,
        })
    }
    /// Menu sources live in the existing retained-import store; no character rows change.
    pub fn menu_catalogues(&self, role: Role) -> Result<Vec<crate::menus::MenuSummary>> {
        require_st(role)?;
        let mut stmt=self.conn.prepare("SELECT hash FROM imports WHERE json_extract(report,'$.kind')='menu-catalogue-v1' ORDER BY source_name,hash")?;
        let hashes = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        hashes
            .into_iter()
            .map(|hash| {
                let menu = self.menu_catalogue(&hash, role)?;
                Ok(crate::menus::MenuSummary {
                    sha256: hash,
                    source_name: menu.source_name,
                    source_format: menu.source_format,
                    rows: menu.rows.len(),
                })
            })
            .collect()
    }
    pub fn menu_catalogue(&self, hash: &str, role: Role) -> Result<crate::menus::MenuPreview> {
        require_st(role)?;
        let stored: Option<(String, Vec<u8>, String)> = self
            .conn
            .query_row(
                "SELECT source_name,source_bytes,report FROM imports WHERE hash=?1",
                [hash],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        let (name, bytes, report) = stored.ok_or_else(|| {
            error(
                "MENU_NOT_FOUND",
                "This library has no saved menu with that source hash.",
            )
        })?;
        let report: Value = serde_json::from_str(&report)?;
        if report["kind"] != "menu-catalogue-v1" {
            return Err(error(
                "MENU_NOT_FOUND",
                "This retained source is game data, not a saved menu.",
            ));
        }
        let encoding = report["encoding"]
            .as_str()
            .ok_or_else(|| error("MENU_STORAGE_INVALID", "Saved menu encoding is missing."))?;
        let menu =
            crate::menus::inspect_with_encoding(&bytes, &name, encoding)?.ok_or_else(|| {
                error(
                    "MENU_STORAGE_INVALID",
                    "Saved bytes are not a supported menu.",
                )
            })?;
        if menu.sha256 != hash
            || report["source_format"] != menu.source_format
            || report["rows"].as_u64() != Some(menu.rows.len() as u64)
        {
            return Err(error(
                "MENU_STORAGE_INVALID",
                "Saved menu source does not match its retained metadata/hash.",
            ));
        }
        Ok(menu)
    }
    pub fn selected_menu_hash(&self, role: Role) -> Result<Option<String>> {
        require_st(role)?;
        Ok(self.conn.query_row("SELECT record_id FROM audit WHERE operation IN ('select_menu','clear_menu') ORDER BY seq DESC LIMIT 1",[],|r|r.get::<_,Option<String>>(0)).optional()?.flatten())
    }
    pub fn active_menu(&self, role: Role) -> Result<Option<crate::menus::MenuPreview>> {
        self.selected_menu_hash(role)?
            .map(|hash| self.menu_catalogue(&hash, role))
            .transpose()
    }
    pub fn save_select_menu(
        &mut self,
        input: &crate::menus::MenuPreview,
        expected: i64,
        role: Role,
    ) -> Result<i64> {
        require_st(role)?;
        let menu = crate::menus::inspect_with_encoding(
            &input.original_bytes,
            &input.source_name,
            &input.encoding,
        )?
        .ok_or_else(|| error("NOT_MENU", "Preview is not backed by supported menu bytes."))?;
        if &menu != input {
            return Err(error(
                "MENU_PREVIEW_CHANGED",
                "Menu preview differs from its source. Reload it before saving.",
            ));
        }
        let report = json!({"kind":"menu-catalogue-v1","source_format":menu.source_format,"encoding":menu.encoding,"rows":menu.rows.len(),"trust":"Raw legacy catalogue; no rules approval or character/XP changes."});
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current: i64 = tx.query_row("SELECT revision FROM library", [], |r| r.get(0))?;
        if current != expected {
            return Err(error(
                "STALE_REVISION",
                "Library changed; reload before choosing a menu.",
            ));
        }
        let old: Option<(String, Vec<u8>)> = tx
            .query_row(
                "SELECT report,source_bytes FROM imports WHERE hash=?1",
                [&menu.sha256],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((old, bytes)) = &old {
            let prior: Value = serde_json::from_str(old)?;
            if prior["kind"] != "menu-catalogue-v1" {
                return Err(error(
                    "MENU_HASH_CONFLICT",
                    "Source hash belongs to another retained import.",
                ));
            }
            if bytes != &menu.original_bytes
                || prior["encoding"] != menu.encoding
                || prior["source_format"] != menu.source_format
                || prior["rows"].as_u64() != Some(menu.rows.len() as u64)
            {
                return Err(error(
                    "MENU_SOURCE_CONFLICT",
                    "Retained source bytes or interpretation differ. Existing menu was not replaced.",
                ));
            }
        }
        let active:Option<String>=tx.query_row("SELECT record_id FROM audit WHERE operation IN ('select_menu','clear_menu') ORDER BY seq DESC LIMIT 1",[],|r|r.get::<_,Option<String>>(0)).optional()?.flatten();
        if old.is_some() && active.as_deref() == Some(&menu.sha256) {
            return Ok(current);
        }
        advance(&tx, expected)?;
        if old.is_none() {
            tx.execute(
                "INSERT INTO imports VALUES(?1,?2,?3,?4)",
                params![
                    menu.sha256,
                    menu.source_name,
                    menu.original_bytes,
                    report.to_string()
                ],
            )?;
            audit(&tx, "save_menu", Some(&menu.sha256), expected + 1)?;
        }
        audit(&tx, "select_menu", Some(&menu.sha256), expected + 1)?;
        tx.commit()?;
        Ok(expected + 1)
    }
    pub fn select_saved_menu(&mut self, hash: &str, expected: i64, role: Role) -> Result<i64> {
        let menu = self.menu_catalogue(hash, role)?;
        self.save_select_menu(&menu, expected, role)
    }
    pub fn clear_selected_menu(&mut self, expected: i64, role: Role) -> Result<i64> {
        require_st(role)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        advance(&tx, expected)?;
        audit(&tx, "clear_menu", None, expected + 1)?;
        tx.commit()?;
        Ok(expected + 1)
    }

    pub fn revision(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT revision FROM library", [], |r| r.get(0))?)
    }
    pub fn library_id(&self) -> Result<String> {
        Ok(self
            .conn
            .query_row("SELECT id FROM library", [], |r| r.get(0))?)
    }
    fn records(&self) -> Result<Vec<Record>> {
        let mut stmt = self
            .conn
            .prepare("SELECT data FROM records ORDER BY kind,name,id")?;
        let strings = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        strings
            .into_iter()
            .map(|s| serde_json::from_str(&s).map_err(Into::into))
            .collect()
    }
    fn stored_record(&self, id: &str) -> Result<Record> {
        let data: Option<String> = self
            .conn
            .query_row("SELECT data FROM records WHERE id=?1", [id], |r| r.get(0))
            .optional()?;
        serde_json::from_str(&data.ok_or_else(|| error("NOT_FOUND", "Record no longer exists."))?)
            .map_err(Into::into)
    }
    pub fn summaries(&self, role: Role) -> Result<Vec<Summary>> {
        let records = self.records()?;
        let players = records
            .iter()
            .filter(|r| r.kind == "Player")
            .map(|r| (r.id.clone(), record_name(r, role)))
            .collect::<std::collections::HashMap<_, _>>();
        Ok(records
            .into_iter()
            .filter(|r| {
                role == Role::Storyteller || (r.build.is_none() && !private_story_kind(&r.kind))
            })
            .map(|r| {
                let player = r
                    .player_id
                    .as_ref()
                    .and_then(|id| players.get(id))
                    .map(String::as_str)
                    .unwrap_or_else(|| {
                        r.public_fields
                            .get("Player")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                    });
                Summary {
                    build: r.build.as_ref().map(|b| b.kind),
                    id: r.id.clone(),
                    kind: r.kind.clone(),
                    name: record_name(&r, role),
                    player_name: display_text(player, role),
                }
            })
            .collect())
    }
    pub fn record(&self, id: &str, role: Role) -> Result<RecordView> {
        let record = self.stored_record(id)?;
        if role == Role::Player && (record.build.is_some() || private_story_kind(&record.kind)) {
            return Err(error(
                "ACCESS_DENIED",
                "Build copies and private Storyteller records are restricted.",
            ));
        }
        let mut fields = record.public_fields.clone();
        if role == Role::Storyteller {
            fields.extend(record.private_fields.clone());
        }
        fields.remove(crate::purchase_reviews::FIELD);
        fields.remove(crate::published_reference::FIELD);
        fields.remove(crate::met_references::FIELD);
        fields.remove(crate::character_lifecycle::FIELD);
        fields.remove(crate::mixed_advancement::FIELD);
        fields.remove(crate::mixed_advancement_draft::FIELD);
        if role == Role::Storyteller
            && record
                .build
                .as_ref()
                .is_some_and(|b| b.kind == BuildKind::Draft)
            && record
                .private_fields
                .contains_key(crate::mixed_advancement_draft::FIELD)
        {
            fields.insert(crate::mixed_advancement_draft::FIELD.into(), json!(true));
        }
        fields.remove(crate::vampire_creation::FIELD);
        fields.remove(crate::vampire_creation::DRAFT_FIELD);
        fields.remove(points_store::FIELD);
        fields.remove(crate::plots::FIELD);
        fields.remove(crate::associations::FIELD);
        fields.remove(crate::story_work::Kind::Action.field());
        fields.remove(crate::story_work::Kind::Rumor.field());
        let notes = if role == Role::Storyteller {
            record
                .private_fields
                .get("AppNotes")
                .or_else(|| record.private_fields.get("Notes"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .into()
        } else {
            String::new()
        };
        let player_name = if let Some(id) = &record.player_id {
            record_name(&self.stored_record(id)?, role)
        } else {
            fields
                .get("Player")
                .and_then(Value::as_str)
                .unwrap_or("")
                .into()
        };
        if !player_name.is_empty() {
            fields.insert("Player".into(), json!(player_name));
        }
        let name = record_name(&record, role);
        Ok(RecordView {
            build: record.build.as_ref().map(|b| b.kind),
            id: record.id,
            kind: record.kind,
            name,
            status: fields
                .get("Status")
                .and_then(Value::as_str)
                .unwrap_or("")
                .into(),
            player_name,
            notes,
            fields,
        })
    }
    pub fn create_character(
        &mut self,
        kind: &str,
        name: &str,
        expected: i64,
        role: Role,
    ) -> Result<String> {
        require_st(role)?;
        if ![
            "Vampire",
            "Werewolf",
            "Mortal",
            "Changeling",
            "Wraith",
            "Mage",
            "Fera",
            "Various",
            "Mummy",
            "KueiJin",
            "Hunter",
            "Demon",
            "Risen",
        ]
        .contains(&kind)
        {
            return Err(error("UNKNOWN_TYPE", "Choose a supported character type."));
        }
        if name.trim().is_empty() {
            return Err(error("EMPTY_NAME", "Enter a name."));
        }
        let id = Uuid::new_v4().to_string();
        let mut public_fields: Map<String, Value> = serde_json::from_value(
            json!({"Name":importer::redact(&json!(name.trim())),"Status":"Draft","Experience":{"earned_decimal":"0","unspent_decimal":"0"}}),
        )?;
        if let Some(profile) = crate::creature_profiles::profile(kind) {
            // Preserve the exact legacy newborn Vampire representation (including no Path key).
            let identity_fields: Vec<&str> = if kind == "Vampire" {
                vec!["Clan", "Sect", "Nature", "Demeanor"]
            } else {
                profile.identity_fields.iter().map(String::as_str).collect()
            };
            for field in identity_fields {
                public_fields.insert(field.into(), Value::String(String::new()));
            }
            public_fields.insert(
                profile.primary_power_list.clone(),
                crate::sheet::empty_list(&profile.primary_power_list),
            );
        }
        let record = Record {
            id: id.clone(),
            kind: kind.into(),
            name: name.trim().into(),
            player_id: None,
            legacy_id: None,
            public_fields,
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
        audit(&tx, "create_character", Some(&id), expected + 1)?;
        tx.commit()?;
        Ok(id)
    }
    pub fn save_edit(&mut self, id: &str, edit: &Edit, expected: i64, role: Role) -> Result<i64> {
        require_st(role)?;
        if edit.name.trim().is_empty() {
            return Err(error(
                "EMPTY_NAME",
                "A character or record name cannot be empty.",
            ));
        }
        let mut record = self.stored_record(id)?;
        apply_basic(&mut record, edit)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        advance(&tx, expected)?;
        tx.execute(
            "UPDATE records SET name=?1,data=?2 WHERE id=?3",
            params![record.name, serde_json::to_string(&record)?, id],
        )?;
        audit(&tx, "edit", Some(id), expected + 1)?;
        tx.commit()?;
        Ok(expected + 1)
    }
    pub fn sheet_edit(&self, id: &str, role: Role) -> Result<SheetEdit> {
        require_st(role)?;
        let record = self.stored_record(id)?;
        if !is_character(&record.kind) {
            return Err(error("NOT_CHARACTER", "Select a character sheet."));
        }
        Ok(crate::sheet::extract(&record))
    }
    pub fn save_sheet(
        &mut self,
        id: &str,
        edit: &SheetEdit,
        expected: i64,
        role: Role,
    ) -> Result<i64> {
        require_st(role)?;
        let record = self.prepared_sheet(id, edit)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        advance(&tx, expected)?;
        tx.execute(
            "UPDATE records SET data=?1 WHERE id=?2",
            params![serde_json::to_string(&record)?, id],
        )?;
        audit(&tx, "edit_sheet", Some(id), expected + 1)?;
        tx.commit()?;
        Ok(expected + 1)
    }
    fn prepared_sheet(&self, id: &str, edit: &SheetEdit) -> Result<Record> {
        crate::sheet::validate(edit)?;
        let mut record = self.stored_record(id)?;
        if !is_character(&record.kind) {
            return Err(error("NOT_CHARACTER", "Select a character sheet."));
        }
        for (key, value) in &edit.identity {
            let existing = record
                .private_fields
                .get(key)
                .or_else(|| record.public_fields.get(key));
            if existing.is_some_and(|v| !v.is_string()) {
                return Err(error(
                    "UNSUPPORTED_EXISTING_FIELD",
                    "Existing nonstandard identity data must be retained for review.",
                ));
            }
            set_sheet_field(&mut record, key, json!(value));
        }
        for (key, list) in &edit.lists {
            let existing = record
                .private_fields
                .get(key)
                .or_else(|| record.public_fields.get(key));
            if existing.is_some_and(|v| !crate::sheet::editable_list(v)) {
                return Err(error(
                    "UNSUPPORTED_EXISTING_FIELD",
                    "Existing nonstandard trait data must be retained for review.",
                ));
            }
            set_sheet_field(&mut record, key, list.clone());
        }
        for (key, value) in &edit.resources {
            let existing = record
                .private_fields
                .get(key)
                .or_else(|| record.public_fields.get(key));
            if existing.is_some_and(|v| v.as_i64().is_none()) {
                return Err(error(
                    "UNSUPPORTED_EXISTING_FIELD",
                    "Existing descriptive resource data must be retained for review.",
                ));
            }
            set_sheet_field(&mut record, key, json!(value));
        }
        // Sharing is an explicit ST choice. Existing private fields are never
        // promoted merely by opening or editing a character.
        for (key, visible) in &edit.visibility {
            let value = record
                .private_fields
                .get(key)
                .or_else(|| record.public_fields.get(key))
                .cloned()
                .ok_or_else(|| {
                    error(
                        "MISSING_SHEET_FIELD",
                        "Reload the sheet before changing visibility.",
                    )
                })?;
            if *visible {
                record
                    .public_fields
                    .entry(key.clone())
                    .or_insert(Value::Null);
                set_sheet_field(&mut record, key, value);
            } else {
                record.public_fields.remove(key);
                record.private_fields.insert(key.clone(), value);
            }
        }
        match &edit.player_link {
            PlayerLink::Keep => {}
            PlayerLink::Clear => {
                record.player_id = None;
                record.public_fields.remove("Player");
                record.private_fields.remove("Player");
            }
            PlayerLink::Assign(player_id) => {
                let player = self.stored_record(player_id)?;
                if player.kind != "Player" || player.build.is_some() {
                    return Err(error(
                        "INVALID_PLAYER_LINK",
                        "Choose an existing player record.",
                    ));
                }
                record.player_id = Some(player.id.clone());
                let public_name = record_name(&player, Role::Player);
                record
                    .public_fields
                    .insert("Player".into(), json!(public_name));
                record
                    .private_fields
                    .insert("Player".into(), json!(player.name));
            }
        }
        Ok(record)
    }
    pub fn save_character(
        &mut self,
        id: &str,
        basic: &Edit,
        sheet: &SheetEdit,
        expected: i64,
        role: Role,
    ) -> Result<i64> {
        require_st(role)?;
        let mut record = self.prepared_sheet(id, sheet)?;
        apply_basic(&mut record, basic)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        advance(&tx, expected)?;
        tx.execute(
            "UPDATE records SET name=?1,data=?2 WHERE id=?3",
            params![record.name, serde_json::to_string(&record)?, id],
        )?;
        audit(&tx, "edit_character", Some(id), expected + 1)?;
        tx.commit()?;
        Ok(expected + 1)
    }
    pub fn create_build(
        &mut self,
        source_id: &str,
        kind: BuildKind,
        expected: i64,
        role: Role,
    ) -> Result<String> {
        require_st(role)?;
        let mut record = self.stored_record(source_id)?;
        if !is_character(&record.kind) || record.build.is_some() {
            return Err(error(
                "INVALID_BUILD_SOURCE",
                "Select an original character to make a build copy.",
            ));
        }
        record.id = Uuid::new_v4().to_string();
        record.build = Some(BuildInfo {
            source_id: source_id.into(),
            source_revision: expected,
            kind,
        });
        record.name = format!(
            "{} — {}",
            record.name,
            if kind == BuildKind::Draft {
                "draft"
            } else {
                "test"
            }
        );
        let name = record.name.clone();
        set_sheet_field(&mut record, "Name", json!(name));
        record.private_fields.remove("Status");
        record.private_fields.remove(crate::associations::FIELD);
        record.private_fields.remove(crate::boon_book::FIELD);
        record
            .private_fields
            .remove(crate::published_reference::FIELD);
        record.private_fields.remove(crate::met_references::FIELD);
        record
            .private_fields
            .remove(crate::mixed_advancement::FIELD);
        record
            .private_fields
            .remove(crate::character_lifecycle::FIELD);
        record.private_fields.remove(crate::vampire_creation::FIELD);
        record
            .private_fields
            .remove(crate::vampire_creation::DRAFT_FIELD);
        record.public_fields.insert(
            "Status".into(),
            json!(if kind == BuildKind::Draft {
                "Draft — not approved"
            } else {
                "Test — not approved"
            }),
        );
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        advance(&tx, expected)?;
        insert(&tx, &record)?;
        audit(&tx, "create_build", Some(&record.id), expected + 1)?;
        tx.commit()?;
        Ok(record.id)
    }

    pub fn purchase_reviews(
        &self,
        id: &str,
        role: Role,
    ) -> Result<Vec<(crate::purchase_reviews::ReviewBinding, bool)>> {
        require_st(role)?;
        let (library_id, data): (String, String) = self.conn.query_row(
            "SELECT library.id,records.data FROM library JOIN records ON records.id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let record: Record = serde_json::from_str(&data)?;
        crate::purchase_reviews::history(&record)?
            .into_iter()
            .map(|b| {
                let current = crate::purchase_reviews::is_current(&library_id, &record, &b)?;
                Ok((b, current))
            })
            .collect()
    }

    pub fn save_purchase_review(
        &mut self,
        id: &str,
        request: &crate::purchases::PurchaseRequest,
        name: &str,
        expected: i64,
        role: Role,
    ) -> Result<crate::purchase_reviews::ReviewBinding> {
        require_st(role)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (library_id, revision): (String, i64) =
            tx.query_row("SELECT id,revision FROM library", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?;
        if revision != expected {
            return Err(error(
                "STALE_REVISION",
                "Library changed; reload before recording a review.",
            ));
        }
        let data: String =
            tx.query_row("SELECT data FROM records WHERE id=?1", [id], |r| r.get(0))?;
        let record: Record = serde_json::from_str(&data)?;
        let binding = crate::purchase_reviews::make(&library_id, &record, request, name)?;
        let updated = crate::purchase_reviews::append(&library_id, &record, &binding)?;
        advance(&tx, expected)?;
        tx.execute(
            "UPDATE records SET data=?1 WHERE id=?2",
            params![serde_json::to_string(&updated)?, id],
        )?;
        audit(&tx, "purchase_review", Some(id), expected + 1)?;
        tx.commit()?;
        Ok(binding)
    }

    pub fn reuse_purchase_review(
        &self,
        id: &str,
        review_id: &str,
        input: &crate::purchases::PurchaseRequest,
        expected: i64,
        role: Role,
    ) -> Result<crate::purchases::PurchaseRequest> {
        require_st(role)?;
        let (library_id, revision, data): (String, i64, String) = self.conn.query_row(
            "SELECT library.id,library.revision,records.data FROM library JOIN records ON records.id=?1",
            [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        if revision != expected {
            return Err(error(
                "STALE_REVISION",
                "Library changed; reload before reusing review.",
            ));
        }
        let record: Record = serde_json::from_str(&data)?;
        let mut updated = input.clone();
        crate::purchase_reviews::reuse(&library_id, &record, review_id, &mut updated)?;
        Ok(updated)
    }

    pub fn quote_purchase(
        &self,
        id: &str,
        request: &crate::purchases::PurchaseRequest,
        expected: i64,
        role: Role,
    ) -> Result<crate::purchases::PurchaseQuote> {
        require_st(role)?;
        // Read the library snapshot and sheet in one SELECT so a concurrent writer
        // cannot pair an old revision with new character data (or the reverse).
        let snapshot: Option<(String, i64, String)> = self
            .conn
            .query_row(
                "SELECT library.id,library.revision,records.data FROM library JOIN records ON records.id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        let (library_id, revision, data) =
            snapshot.ok_or_else(|| error("NOT_FOUND", "Record no longer exists."))?;
        if revision != expected {
            return Err(error(
                "STALE_REVISION",
                "Library changed; reload before quoting a purchase.",
            ));
        }
        let record: Record = serde_json::from_str(&data)?;
        crate::purchases::quote(&library_id, revision, &record, request)
    }

    pub fn commit_purchase(
        &mut self,
        quote: &crate::purchases::PurchaseQuote,
        role: Role,
    ) -> Result<crate::purchases::PurchaseReceipt> {
        require_st(role)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;

        let cancellation_id_reserved:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM purchase_receipts WHERE json_extract(data,'$.cancellation.quote.request.id')=?1)",[&quote.request.id],|r|r.get(0))?;
        if cancellation_id_reserved {
            return Err(error(
                "PURCHASE_ID_RESERVED",
                "That ID belongs to a saved cancellation.",
            ));
        }

        // Request IDs are immutable receipt keys. A byte-for-byte equivalent quote
        // is a safe retry even after later library revisions; any other reuse fails.
        let prior: Option<(String, String)> = tx
            .query_row(
                "SELECT record_id,data FROM purchase_receipts WHERE id=?1",
                [&quote.request.id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((record_id, data)) = prior {
            let saved: crate::purchases::PurchaseReceipt = serde_json::from_str(&data)?;
            let library_id: String = tx.query_row("SELECT id FROM library", [], |r| r.get(0))?;
            validate_receipt_binding(&saved, &quote.request.id, &record_id, &library_id)?;
            if saved.quote == *quote {
                return Ok(saved);
            }
            return Err(error(
                "PURCHASE_ID_CONFLICT",
                "This purchase ID already identifies a different committed receipt.",
            ));
        }

        let (library_id, revision): (String, i64) =
            tx.query_row("SELECT id,revision FROM library", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?;
        if library_id != quote.library_id || revision != quote.revision {
            return Err(error(
                "STALE_PURCHASE",
                "Library changed after this quote. Request a fresh purchase quote.",
            ));
        }
        let data: Option<String> = tx
            .query_row(
                "SELECT data FROM records WHERE id=?1",
                [&quote.record_id],
                |r| r.get(0),
            )
            .optional()?;
        let record: Record = serde_json::from_str(&data.ok_or_else(|| {
            error(
                "STALE_PURCHASE",
                "The quoted character no longer exists. Request a fresh purchase quote.",
            )
        })?)?;
        let recomputed = crate::purchases::quote(&library_id, revision, &record, &quote.request)?;
        if recomputed != *quote {
            return Err(error(
                "PURCHASE_QUOTE_CHANGED",
                "The quote differs from the current character or verified purchase inputs. Request a fresh quote.",
            ));
        }

        let updated = crate::purchases::apply(
            &record,
            &recomputed,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        )?;
        advance(&tx, revision)?;
        if tx.execute(
            "UPDATE records SET name=?1,data=?2 WHERE id=?3",
            params![updated.name, serde_json::to_string(&updated)?, updated.id],
        )? != 1
        {
            return Err(error(
                "STALE_PURCHASE",
                "The quoted character no longer exists. Request a fresh purchase quote.",
            ));
        }
        let saved = crate::purchases::receipt(&recomputed, revision + 1);
        let saved_data = serde_json::to_string(&saved)?;
        tx.execute(
            "INSERT INTO purchase_receipts(id,record_id,data) VALUES(?1,?2,?3)",
            params![saved.quote.request.id, saved.quote.record_id, saved_data],
        )?;
        audit(
            &tx,
            "purchase_commit",
            Some(&saved.quote.record_id),
            revision + 1,
        )?;
        tx.commit()?;
        Ok(saved)
    }

    pub fn quote_cancellation(
        &self,
        purchase_id: &str,
        input: &crate::cancellations::CancellationRequest,
        expected: i64,
        role: Role,
    ) -> Result<crate::cancellations::CancellationQuote> {
        require_st(role)?;
        let row:Option<(String,i64,String,String,String)>=self.conn.query_row(
            "SELECT library.id,library.revision,records.data,purchase_receipts.record_id,purchase_receipts.data FROM library JOIN purchase_receipts ON purchase_receipts.id=?1 JOIN records ON records.id=purchase_receipts.record_id",
            [purchase_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).optional()?;
        let (library_id, revision, data, record_id, receipt_data) =
            row.ok_or_else(|| error("NOT_FOUND", "Select an existing purchase receipt."))?;
        if revision != expected {
            return Err(error(
                "STALE_CANCELLATION",
                "Library changed; review the cancellation again.",
            ));
        }
        let purchase: crate::purchases::PurchaseReceipt = serde_json::from_str(&receipt_data)?;
        validate_receipt_binding(&purchase, purchase_id, &record_id, &library_id)?;
        let record: Record = serde_json::from_str(&data)?;
        crate::cancellations::quote(&library_id, revision, &record, &purchase, input)
    }
    pub fn commit_cancellation(
        &mut self,
        quote: &crate::cancellations::CancellationQuote,
        role: Role,
    ) -> Result<crate::cancellations::CancellationReceipt> {
        require_st(role)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (library_id, revision): (String, i64) =
            tx.query_row("SELECT id,revision FROM library", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?;
        let row: Option<(String, String)> = tx
            .query_row(
                "SELECT record_id,data FROM purchase_receipts WHERE id=?1",
                [&quote.purchase_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let (record_id, data) =
            row.ok_or_else(|| error("NOT_FOUND", "Purchase receipt no longer exists."))?;
        let mut purchase: crate::purchases::PurchaseReceipt = serde_json::from_str(&data)?;
        validate_receipt_binding(&purchase, &quote.purchase_id, &record_id, &library_id)?;
        if let Some(saved) = &purchase.cancellation {
            if saved.quote == *quote {
                return Ok(saved.clone());
            }
            return Err(error(
                "CANCELLATION_ALREADY_RECORDED",
                "This purchase already has a different cancellation.",
            ));
        }
        if library_id != quote.library_id
            || revision != quote.revision
            || record_id != quote.record_id
        {
            return Err(error(
                "STALE_CANCELLATION",
                "The quoted library or sheet changed. Review cancellation again.",
            ));
        }
        let reserved:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM purchase_receipts WHERE id=?1 OR json_extract(data,'$.cancellation.quote.request.id')=?1)",[&quote.request.id],|r|r.get(0))?;
        if reserved {
            return Err(error(
                "CANCELLATION_ID_CONFLICT",
                "That request ID already belongs to a purchase or cancellation.",
            ));
        }
        let record_data: String =
            tx.query_row("SELECT data FROM records WHERE id=?1", [&record_id], |r| {
                r.get(0)
            })?;
        let record: Record = serde_json::from_str(&record_data)?;
        let recomputed =
            crate::cancellations::quote(&library_id, revision, &record, &purchase, &quote.request)?;
        if recomputed != *quote {
            return Err(error(
                "CANCELLATION_QUOTE_CHANGED",
                "The cancellation differs from the committed record or receipt. Review it again.",
            ));
        }
        let updated = crate::cancellations::apply(
            &record,
            &recomputed,
            &purchase,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        )?;
        advance(&tx, revision)?;
        tx.execute(
            "UPDATE records SET data=?1 WHERE id=?2",
            params![serde_json::to_string(&updated)?, record_id],
        )?;
        let saved = crate::cancellations::CancellationReceipt {
            quote: recomputed,
            committed_revision: revision + 1,
        };
        purchase.cancellation = Some(saved.clone());
        if tx.execute("UPDATE purchase_receipts SET data=?1 WHERE id=?2 AND json_extract(data,'$.cancellation') IS NULL",params![serde_json::to_string(&purchase)?,quote.purchase_id])?!=1{return Err(error("CANCELLATION_ALREADY_RECORDED","Purchase cancellation changed during commit."));}
        audit(&tx, "purchase_cancel", Some(&record_id), revision + 1)?;
        tx.commit()?;
        Ok(saved)
    }

    pub fn purchase_receipts(
        &self,
        id: &str,
        role: Role,
    ) -> Result<Vec<crate::purchases::PurchaseReceipt>> {
        require_st(role)?;
        let mut stmt = self.conn.prepare(
            "SELECT id,record_id,data FROM purchase_receipts WHERE record_id=?1 ORDER BY id",
        )?;
        let library_id = self.library_id()?;
        let rows = stmt
            .query_map([id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut receipts = Vec::with_capacity(rows.len());
        for (stored_id, record_id, data) in rows {
            let receipt: crate::purchases::PurchaseReceipt = serde_json::from_str(&data)?;
            validate_receipt_binding(&receipt, &stored_id, &record_id, &library_id)?;
            receipts.push(receipt);
        }
        receipts.sort_by(|a, b| {
            a.committed_revision
                .cmp(&b.committed_revision)
                .then_with(|| a.quote.request.id.cmp(&b.quote.request.id))
        });
        let mut groups: std::collections::BTreeMap<&str, Vec<&crate::purchases::PurchaseReceipt>> =
            std::collections::BTreeMap::new();
        for receipt in &receipts {
            if let Some(binding) = &receipt.batch {
                groups.entry(&binding.quote_hash).or_default().push(receipt);
            }
        }
        for group in groups.values() {
            let first = group[0];
            let count = first.batch.as_ref().unwrap().count;
            let indices: HashSet<_> = group
                .iter()
                .map(|receipt| receipt.batch.as_ref().unwrap().index)
                .collect();
            if group.len() != count
                || indices.len() != count
                || group.iter().any(|receipt| {
                    receipt.batch.as_ref().unwrap().count != count
                        || receipt.committed_revision != first.committed_revision
                })
            {
                return Err(error(
                    "PURCHASE_BATCH_INVALID",
                    "A saved advancement group is incomplete or inconsistent. Review a known-good backup.",
                ));
            }
        }
        Ok(receipts)
    }

    pub fn xp_view(&self, id: &str, role: Role) -> Result<crate::xp::XpView> {
        require_st(role)?;
        let record = self.stored_record(id)?;
        if !is_character(&record.kind) {
            return Err(error("NOT_CHARACTER", "Select a character sheet."));
        }
        let mut view = crate::xp::view(&record);
        view.findings
            .extend(crate::trait_advancement::findings(&record));
        for receipt in self.purchase_receipts(id, role)? {
            view.findings
                .extend(crate::purchases::findings(&record, &receipt));
        }
        Ok(view)
    }
    pub fn xp_roster_audit(&self, role: Role) -> Result<Vec<crate::xp::XpView>> {
        require_st(role)?;
        let records = self.records()?;
        let mut views = Vec::new();
        for record in records
            .iter()
            .filter(|r| is_character(&r.kind) && r.build.is_none())
        {
            let mut view = crate::xp::view(record);
            view.findings
                .extend(crate::trait_advancement::findings(record));
            for receipt in self.purchase_receipts(&record.id, role)? {
                view.findings
                    .extend(crate::purchases::findings(record, &receipt));
            }
            views.push(view);
        }
        Ok(views)
    }
    pub fn print_document(
        &self,
        id: &str,
        options: &crate::printing::Options,
        expected: i64,
        role: Role,
    ) -> Result<crate::printing::SheetDocument> {
        use crate::printing::PrintScope;
        if options.scope == PrintScope::PrivateStoryteller {
            require_st(role)?;
        }
        let (revision, data, player_data): (i64, String, Option<String>) = self.conn.query_row(
            "SELECT library.revision,records.data,player.data FROM library JOIN records ON records.id=?1 LEFT JOIN records AS player ON player.id=json_extract(records.data,'$.player_id')",
            [id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?)),
        )?;
        if revision != expected {
            return Err(error(
                "STALE_REVISION",
                "Library changed; preview the sheet again.",
            ));
        }
        let record: Record = serde_json::from_str(&data)?;
        if !is_character(&record.kind) {
            return Err(error(
                "PRINT_SCOPE",
                "Select a character to preview its sheet.",
            ));
        }
        if record.build.is_some() {
            require_st(role)?;
        }
        let private = options.scope == PrintScope::PrivateStoryteller;
        let mut fields = record.public_fields.clone();
        if private {
            fields.extend(record.private_fields.clone());
        } else {
            let mut safe = importer::redact(&Value::Object(fields));
            if let Some((start, end)) = &record.st_markers {
                safe = importer::redact_custom(&safe, start, end);
            }
            fields = safe.as_object().cloned().unwrap_or_default();
        }
        let player_name = if let Some(data) = player_data {
            let player: Record = serde_json::from_str(&data)?;
            let name = record_name(
                &player,
                if private {
                    Role::Storyteller
                } else {
                    Role::Player
                },
            );
            if private {
                name
            } else {
                let mut safe = importer::redact(&Value::String(name));
                if let Some((start, end)) = &player.st_markers {
                    safe = importer::redact_custom(&safe, start, end);
                }
                safe.as_str().unwrap_or_default().to_owned()
            }
        } else {
            fields
                .get("Player")
                .and_then(Value::as_str)
                .unwrap_or("")
                .into()
        };
        let notes = if private && options.include_notes {
            record
                .private_fields
                .get("AppNotes")
                .or_else(|| record.private_fields.get("Notes"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .into()
        } else {
            String::new()
        };
        fields.remove(crate::published_reference::FIELD);
        fields.remove(crate::met_references::FIELD);
        fields.remove(crate::character_lifecycle::FIELD);
        fields.remove(crate::mixed_advancement::FIELD);
        fields.remove(crate::mixed_advancement_draft::FIELD);
        fields.remove(crate::vampire_creation::FIELD);
        fields.remove(crate::vampire_creation::DRAFT_FIELD);
        let view = RecordView {
            build: record.build.as_ref().map(|b| b.kind),
            id: record.id.clone(),
            kind: record.kind.clone(),
            name: if private {
                record.name.clone()
            } else {
                fields
                    .get("Name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .into()
            },
            status: fields
                .get("Status")
                .and_then(Value::as_str)
                .unwrap_or("")
                .into(),
            player_name,
            notes,
            fields,
        };
        // Only explicit stored source IDs may add references. A matching legacy name
        // alone cannot establish an edition or a power's availability/approval.
        // Copies retain source mappings, but do not duplicate private creation histories.
        // Resolve their original proof internally, bounded by the recorded copy revision.
        let mut mapping_record = record.clone();
        if let Some(build) = &record.build
            && let Ok(source) = self.stored_record(&build.source_id)
            && source.kind == record.kind
        {
            mapping_record.id = source.id.clone();
            mapping_record
                .private_fields
                .remove(crate::met_references::FIELD);
            if let Some(history) = source.private_fields.get(crate::met_references::FIELD) {
                mapping_record
                    .private_fields
                    .insert(crate::met_references::FIELD.into(), history.clone());
            }

            mapping_record
                .private_fields
                .remove(crate::vampire_creation::FIELD);
            if let Some(history) = source.private_fields.get(crate::vampire_creation::FIELD) {
                mapping_record
                    .private_fields
                    .insert(crate::vampire_creation::FIELD.into(), history.clone());
            }
        }
        let power_fields = crate::creature_profiles::profile(&record.kind)
            .map(|p| {
                vec![
                    p.primary_power_list.as_str(),
                    p.secondary_power_list.as_str(),
                ]
            })
            .unwrap_or_default();
        let mut ids = Vec::new();
        for field in &power_fields {
            let visible = view.fields.get(*field).and_then(|v| v["Traits"].as_array());
            let rows = record
                .private_fields
                .get(*field)
                .or_else(|| record.public_fields.get(*field))
                .and_then(|v| v["Traits"].as_array());
            for row in rows.into_iter().flatten() {
                if let Some(id) = crate::creation_sources::mapped_power(&mapping_record, row)
                    && crate::power_catalogue::builtin()
                        .lookup(&id)
                        .is_some_and(|entry| entry.target_list == *field)
                    && visible.is_some_and(|rows| {
                        rows.iter()
                            .any(|r| r["Name"] == row["Name"] && r["Value"] == row["Value"])
                    })
                    && !ids.contains(&id)
                {
                    ids.push(id);
                }
            }
        }
        let mut document = crate::printing::document(&view, &ids, options)?;
        if document.options.include_references {
            for id in &ids {
                if let Some(reference) = crate::printing::mapped_reference(id)
                    && !document
                        .references
                        .iter()
                        .any(|r| r.power == reference.power && r.family == reference.family)
                {
                    document.references.push(reference);
                }
            }
            let mut seen = HashSet::new();
            for field in &power_fields {
                let visible = view.fields.get(*field).and_then(|v| v["Traits"].as_array());
                let rows = record
                    .private_fields
                    .get(*field)
                    .or_else(|| record.public_fields.get(*field))
                    .and_then(|v| v["Traits"].as_array());
                for row in rows.into_iter().flatten() {
                    if visible.is_some_and(|rows| {
                        rows.iter()
                            .any(|r| r["Name"] == row["Name"] && r["Value"] == row["Value"])
                    }) && let Some(reference) =
                        crate::published_reference::verified_reference(row)
                        && crate::published_powers::catalogue()?
                            .book(&reference.book_id)
                            .is_some_and(|book| book.applies_to(&record.kind))
                        && seen.insert((
                            reference.power.id.clone(),
                            row["Name"].as_str().unwrap_or("").to_owned(),
                            row["Value"].as_str().unwrap_or("").to_owned(),
                        ))
                    {
                        document
                            .published_references
                            .push(crate::printing::PublishedReference {
                                recorded_name: row["Name"].as_str().unwrap_or("").into(),
                                recorded_value: row["Value"].as_str().unwrap_or("").into(),
                                source: reference,
                            });
                    }
                }
            }
        }
        // Mechanics remain scoped to the existing reviewed Discipline reminder catalogue.
        let public_traits = view
            .fields
            .get("DisciplineList")
            .and_then(|list| list["Traits"].as_array());
        if options.include_simple_mechanics {
            crate::simple_mechanics::append(
                &mapping_record,
                public_traits.map(Vec::as_slice).unwrap_or_default(),
                &mut document,
            )?;
        }
        Ok(document)
    }

    pub fn playtime_awards(
        &self,
        id: &str,
        start: &str,
        end: &str,
        expected: i64,
        role: Role,
    ) -> Result<Vec<crate::playtime::AwardSnapshot>> {
        require_st(role)?;
        let (revision, data): (i64, String) = self.conn.query_row(
            "SELECT library.revision,records.data FROM library JOIN records ON records.id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        if revision != expected {
            return Err(error(
                "STALE_REVISION",
                "Library changed; reload the award review.",
            ));
        }
        crate::playtime::awards(&serde_json::from_str(&data)?, start, end)
    }
    pub fn playtime_report(
        &self,
        id: &str,
        request: &crate::playtime::Request,
        expected: i64,
        role: Role,
    ) -> Result<Value> {
        require_st(role)?;
        let (library_id, revision, data): (String, i64, String) = self.conn.query_row(
            "SELECT library.id,library.revision,records.data FROM library JOIN records ON records.id=?1",
            [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        if revision != expected {
            return Err(error(
                "STALE_REVISION",
                "Library changed; recalculate the playtime review.",
            ));
        }
        let record: Record = serde_json::from_str(&data)?;
        if !is_character(&record.kind) || record.build.is_some() {
            return Err(error(
                "PLAYTIME_SCOPE",
                "Review an original character, not a player or build copy.",
            ));
        }
        let report = crate::playtime::assess(&record, request)?;
        Ok(json!({
            "format":"grapevine-playtime-transfer-review-v2",
            "library_id":library_id,"library_revision":revision,
            "record_id":id,"record_name":record_name(&record,Role::Player),
            "calculated_unix_seconds":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
            "record_hash":crate::purchases::record_hash(&record)?,
            "declared_assumptions":{
                "classification":request.class,
                "policy_scope_confirmed":request.policy_scope_confirmed,
                "complete_regular_history_confirmed":request.complete_regular_history_confirmed,
                "full_month_eligible_confirmed":request.full_month_eligible_confirmed,
                "home_chronicle_supplied":!request.home_chronicle.trim().is_empty(),
                "review_evidence_supplied":!request.review_evidence.trim().is_empty()
            },
            "privacy":"Review summary omits raw history, ST notes, chronicle name, private evidence and contact fields. Local declarations do not prove authority or approve transfer.",
            "report":report
        }))
    }

    pub fn xp_audit_export(&self, id: Option<&str>, role: Role) -> Result<Value> {
        require_st(role)?;
        let views = if let Some(id) = id {
            vec![self.xp_view(id, role)?]
        } else {
            self.xp_roster_audit(role)?
        };
        let mut records = vec![];
        for view in views {
            let record = self.stored_record(&view.record_id)?;
            records.push(json!({"record_id":view.record_id,"name":record_name(&record,Role::Player),"is_build":view.is_build,"earned":view.earned,"unspent":view.unspent,"legacy_history_count":view.legacy_history_count,"ledger_entry_count":view.ledger.as_ref().map_or(0,|l|l.entries.len()),"source_sha256":record.source_sha256,"coverage":view.coverage,"findings":view.findings}));
        }
        Ok(
            json!({"format":"grapevine-xp-audit-summary-v1","library_id":self.library_id()?,"library_revision":self.revision()?,"privacy":"Local review summary; raw history,recorder names,reasons,evidence text,ST notes and contact fields omitted. Organizational access/approval is not established.","records":records}),
        )
    }
    pub fn initialize_xp(
        &mut self,
        id: &str,
        baseline: &crate::xp::Baseline,
        expected: i64,
        role: Role,
    ) -> Result<i64> {
        require_st(role)?;
        crate::xp::validate_baseline(baseline)?;
        let mut record = self.stored_record(id)?;
        if !is_character(&record.kind) || record.build.is_some() {
            return Err(error(
                "XP_BUILD_READ_ONLY",
                "Open or post a ledger only on an original character; build copies cannot spend XP.",
            ));
        }
        if let Some(existing) = &record.xp_ledger {
            if &existing.baseline == baseline {
                return self.revision();
            }
            return Err(error(
                "XP_LEDGER_EXISTS",
                "Opening balance is retained; use append-only corrections instead of replacing it.",
            ));
        }
        crate::vampire_creation::ensure_opening_allowed(&record)?;
        record.xp_ledger = Some(crate::xp::Ledger {
            baseline: baseline.clone(),
            entries: vec![],
        });
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        advance(&tx, expected)?;
        tx.execute(
            "UPDATE records SET data=?1 WHERE id=?2",
            params![serde_json::to_string(&record)?, id],
        )?;
        audit(&tx, "xp_opening", Some(id), expected + 1)?;
        tx.commit()?;
        Ok(expected + 1)
    }
    pub fn post_xp(
        &mut self,
        id: &str,
        input: &crate::xp::XpCommand,
        expected: i64,
        role: Role,
    ) -> Result<i64> {
        require_st(role)?;
        if crate::trait_advancement::recorded_quote(input).is_some()
            || crate::trait_corrections::recorded_quote(input).is_some()
        {
            return Err(error(
                "TRAIT_COMMAND_RESERVED",
                "A linked trait debit must be saved through its reviewed trait-and-XP transaction.",
            ));
        }
        // Hold the write lock while reserving receipt IDs. A purchase commit and a
        // manual XP command can never race past each other's receipt check.
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let request_id_reserved: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM purchase_receipts WHERE id=?1 OR json_extract(data,'$.cancellation.quote.request.id')=?1)",
            [&input.id],
            |r| r.get(0),
        )?;
        if request_id_reserved {
            return Err(error(
                "PURCHASE_ID_RESERVED",
                "This ID belongs to an immutable purchase receipt and cannot identify a manual XP entry.",
            ));
        }
        let reverses_purchase = if input.kind == crate::xp::XpKind::Reversal {
            if let Some(target) = &input.reversal_of {
                tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM purchase_receipts WHERE id=?1 OR json_extract(data,'$.cancellation.quote.request.id')=?1)",
                    [target],
                    |r| r.get::<_, bool>(0),
                )?
            } else {
                false
            }
        } else {
            false
        };
        if reverses_purchase {
            return Err(error(
                "PURCHASE_SPEND_IMMUTABLE",
                "A purchase-linked XP debit cannot be reversed as a manual entry while its receipt remains. Review the receipt and trait together.",
            ));
        }
        let current: i64 = tx.query_row("SELECT revision FROM library", [], |r| r.get(0))?;
        let data: Option<String> = tx
            .query_row("SELECT data FROM records WHERE id=?1", [id], |r| r.get(0))
            .optional()?;
        let mut record: Record = serde_json::from_str(
            &data.ok_or_else(|| error("NOT_FOUND", "Record no longer exists."))?,
        )?;
        if !is_character(&record.kind) || record.build.is_some() {
            return Err(error(
                "XP_BUILD_READ_ONLY",
                "Build copies cannot post or spend XP.",
            ));
        }
        let ledger = record.xp_ledger.as_mut().ok_or_else(|| {
            error(
                "XP_OPENING_REQUIRED",
                "Declare a supported opening balance first.",
            )
        })?;
        if input.kind == crate::xp::XpKind::Reversal
            && ledger.entries.iter().any(|entry| {
                Some(&entry.command.id) == input.reversal_of.as_ref()
                    && (crate::trait_advancement::recorded_quote(&entry.command).is_some()
                        || crate::trait_corrections::recorded_quote(&entry.command).is_some())
            })
        {
            return Err(error(
                "TRAIT_REVERSAL_REVIEW",
                "This debit also changed a trait. A ledger-only reversal would leave the grant in place; review a paired correction instead.",
            ));
        }
        let command = crate::xp::canonical_command(ledger, input)?;
        if let Some(old) = ledger.entries.iter().find(|e| e.command.id == command.id) {
            if old.command == command {
                return Ok(current);
            }
            return Err(error(
                "XP_DUPLICATE_ID",
                "This transaction ID already identifies different data.",
            ));
        }
        ledger.entries.push(crate::xp::Entry {
            command,
            origin_library_id: tx.query_row("SELECT id FROM library", [], |r| r.get(0))?,
            recorded_unix_seconds: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        });
        crate::xp::replay(ledger)?;
        advance(&tx, expected)?;
        if tx.execute(
            "UPDATE records SET data=?1 WHERE id=?2",
            params![serde_json::to_string(&record)?, id],
        )? != 1
        {
            return Err(error("NOT_FOUND", "Record no longer exists."));
        }
        audit(&tx, "xp_post", Some(id), expected + 1)?;
        tx.commit()?;
        Ok(expected + 1)
    }
    pub fn preview_import(&self, path: &Path, encoding: &str, role: Role) -> Result<Preview> {
        require_st(role)?;
        let mut preview = importer::read_preview(path, encoding)?;
        let imported: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM imports WHERE hash=?1)",
            [&preview.source_hash],
            |r| r.get(0),
        )?;
        if imported {
            return Err(error(
                "ALREADY_IMPORTED",
                "This exact source file has already been imported. No duplicate records were added.",
            ));
        }
        let mut keys = self
            .records()?
            .iter()
            .filter(|r| r.build.is_none())
            .map(key)
            .collect::<HashSet<_>>();
        for record in &preview.records {
            if !keys.insert(key(record)) {
                preview.conflicts.push(record.name.clone());
            }
        }
        Ok(preview)
    }
    pub fn commit_import(
        &mut self,
        preview: &Preview,
        expected: i64,
        skip_conflicts: bool,
        role: Role,
    ) -> Result<ImportResult> {
        require_st(role)?;
        // Re-evaluate conflicts in the transaction, never trust a stale preview.
        let mut keys = self
            .records()?
            .iter()
            .filter(|r| r.build.is_none())
            .map(key)
            .collect::<HashSet<_>>();
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        advance(&tx, expected)?;
        if tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM imports WHERE hash=?1)",
            [&preview.source_hash],
            |r| r.get::<_, bool>(0),
        )? {
            return Err(error(
                "ALREADY_IMPORTED",
                "Source already imported; no changes committed.",
            ));
        }
        let mut inserted = 0;
        let mut skipped = 0;
        for record in &preview.records {
            if !keys.insert(key(record)) {
                if !skip_conflicts {
                    return Err(error(
                        "IMPORT_CONFLICT",
                        "Review conflicts or explicitly choose to skip conflicting records.",
                    ));
                }
                skipped += 1;
                continue;
            }
            insert(&tx, record)?;
            inserted += 1;
        }
        tx.execute(
            "INSERT INTO imports VALUES(?1,?2,?3,?4)",
            params![
                preview.source_hash,
                preview.source_name,
                preview.bytes,
                serde_json::to_string(&preview.report)?
            ],
        )?;
        audit(&tx, "import", None, expected + 1)?;
        tx.commit()?;
        Ok(ImportResult {
            inserted,
            skipped,
            revision: expected + 1,
        })
    }
    pub fn backup(&self, path: &Path, role: Role) -> Result<()> {
        require_st(role)?;
        if path.exists() {
            return Err(error(
                "FILE_EXISTS",
                "Choose a new backup filename. Existing files are never overwritten.",
            ));
        }
        FileOptions::new().write(true).create_new(true).open(path)?;
        let mut destination = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        let backup = rusqlite::backup::Backup::new(&self.conn, &mut destination)?;
        backup.run_to_completion(32, Duration::from_millis(5), None)?;
        drop(backup);
        drop(destination);
        checked_open(path)?;
        Ok(())
    }
    pub fn restore(&mut self, path: &Path, expected: i64, role: Role) -> Result<i64> {
        require_st(role)?;
        if path.canonicalize()? == self.path.canonicalize()? {
            return Err(error(
                "SAME_LIBRARY_FILE",
                "Choose a separate backup file, not the open library.",
            ));
        }
        if self.revision()? != expected {
            return Err(error(
                "STALE_REVISION",
                "Library changed. Refresh before restoring.",
            ));
        }
        let mut source = checked_open(path)?;
        let source_tx = source.transaction()?;
        let backup_id: String = source_tx.query_row("SELECT id FROM library", [], |r| r.get(0))?;
        if backup_id != self.library_id()? {
            return Err(error(
                "WRONG_LIBRARY",
                "This backup belongs to a different library.",
            ));
        }
        let prior = self
            .path
            .with_file_name(format!("before-restore-{}.gvlib", Uuid::new_v4()));
        self.backup(&prior, role)?;
        let restored_title: String =
            source_tx.query_row("SELECT title FROM library", [], |r| r.get(0))?;
        let backup_revision: i64 =
            source_tx.query_row("SELECT revision FROM library", [], |r| r.get(0))?;
        let new_revision = expected
            .max(backup_revision)
            .checked_add(1)
            .ok_or_else(|| {
                error(
                    "REVISION_LIMIT",
                    "The library revision cannot be advanced further.",
                )
            })?;
        // Restore contents AND advance the revision in one transaction. A failed
        // copy cannot expose a restored old revision to concurrent writers.
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        advance(&tx, expected)?;
        tx.execute_batch(
            "DELETE FROM records; DELETE FROM imports; DELETE FROM audit; DELETE FROM purchase_receipts; DELETE FROM game_sessions; DELETE FROM scheduled_sessions; DELETE FROM saved_searches; DELETE FROM chronicle_profile; DELETE FROM mixed_advancement_receipts;",
        )?;
        {
            let mut stmt = source_tx.prepare("SELECT id,kind,name,data FROM records")?;
            for row in stmt.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })? {
                let (id, kind, name, data) = row?;
                let _: Record = serde_json::from_str(&data)?;
                tx.execute(
                    "INSERT INTO records VALUES(?1,?2,?3,?4)",
                    params![id, kind, name, data],
                )?;
            }
            let mut stmt =
                source_tx.prepare("SELECT hash,source_name,source_bytes,report FROM imports")?;
            for row in stmt.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Vec<u8>>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })? {
                let (hash, name, bytes, report) = row?;
                tx.execute(
                    "INSERT INTO imports VALUES(?1,?2,?3,?4)",
                    params![hash, name, bytes, report],
                )?;
            }
            let has_receipts: bool = source_tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='purchase_receipts')",
                [],
                |r| r.get(0),
            )?;
            if has_receipts {
                let mut stmt = source_tx
                    .prepare("SELECT id,record_id,data FROM purchase_receipts ORDER BY id")?;
                for row in stmt.query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                    ))
                })? {
                    let (id, record_id, data) = row?;
                    let receipt: crate::purchases::PurchaseReceipt = serde_json::from_str(&data)?;
                    validate_receipt_binding(&receipt, &id, &record_id, &backup_id)?;
                    let record_exists: bool = tx.query_row(
                        "SELECT EXISTS(SELECT 1 FROM records WHERE id=?1)",
                        [&record_id],
                        |r| r.get(0),
                    )?;
                    if !record_exists {
                        return Err(error(
                            "PURCHASE_RECEIPT_INVALID",
                            "A saved purchase receipt refers to a missing restored character.",
                        ));
                    }
                    tx.execute(
                        "INSERT INTO purchase_receipts(id,record_id,data) VALUES(?1,?2,?3)",
                        params![id, record_id, data],
                    )?;
                }
            }
            let has_sessions: bool = source_tx.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='game_sessions')",[],|r|r.get(0))?;
            if has_sessions {
                let mut sessions =
                    source_tx.prepare("SELECT id,data FROM game_sessions ORDER BY id")?;
                for row in sessions
                    .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
                {
                    let (id, data) = row?;
                    let receipt: SessionReceipt = serde_json::from_str(&data)?;
                    if receipt.quote.request.id != id
                        || receipt.quote.library_id != backup_id
                        || receipt.committed_revision
                            != receipt.quote.revision.checked_add(1).ok_or_else(|| {
                                error("INVALID_RECEIPT", "Receipt revision exceeds bounds.")
                            })?
                    {
                        return Err(error(
                            "INVALID_RECEIPT",
                            "A saved session receipt is inconsistent.",
                        ));
                    }
                    tx.execute(
                        "INSERT INTO game_sessions(id,data) VALUES(?1,?2)",
                        params![id, data],
                    )?;
                }
            }
            let has_mixed:bool=source_tx.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='mixed_advancement_receipts')",[],|r|r.get(0))?;
            if has_mixed {
                let mut statement = source_tx
                    .prepare("SELECT id,record_id,data FROM mixed_advancement_receipts")?;
                for row in statement.query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                    ))
                })? {
                    let (id, record_id, data) = row?;
                    if data.len() > 8 * 1024 * 1024 {
                        return Err(error(
                            "MIXED_PLAN_LIMIT",
                            "Saved mixed receipt exceeds its supported size.",
                        ));
                    }
                    let receipt: crate::mixed_advancement::Receipt = serde_json::from_str(&data)?;
                    crate::mixed_advancement::validate_receipt(&receipt)?;
                    let exists: bool = tx.query_row(
                        "SELECT EXISTS(SELECT 1 FROM records WHERE id=?1)",
                        [&record_id],
                        |r| r.get(0),
                    )?;
                    if !exists
                        || receipt.quote.record_id != record_id
                        || receipt.quote.library_id != backup_id
                        || crate::mixed_advancement::hash(&receipt.quote)? != id
                    {
                        return Err(error(
                            "MIXED_PLAN_RECEIPT",
                            "Saved mixed plan binding is inconsistent.",
                        ));
                    }
                    tx.execute(
                        "INSERT INTO mixed_advancement_receipts VALUES(?1,?2,?3)",
                        params![id, record_id, data],
                    )?;
                }
            }
            let has_schedules:bool=source_tx.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='scheduled_sessions')",[],|r|r.get(0))?;
            if has_schedules {
                let mut statement =
                    source_tx.prepare("SELECT id,data FROM scheduled_sessions ORDER BY id")?;
                for row in statement
                    .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
                {
                    let (id, data) = row?;
                    let scheduled: ScheduledSession = serde_json::from_str(&data)?;
                    if scheduled.request.id != id {
                        return Err(error(
                            "INVALID_SCHEDULE",
                            "Saved event identity is inconsistent.",
                        ));
                    }
                    tx.execute(
                        "INSERT INTO scheduled_sessions(id,data) VALUES(?1,?2)",
                        params![id, data],
                    )?;
                }
            }
            let has_searches: bool = source_tx.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='saved_searches')", [], |r| r.get(0))?;
            if has_searches {
                for entry in search_store::entries(&source_tx)? {
                    tx.execute(
                        "INSERT INTO saved_searches(id,data) VALUES(?1,?2)",
                        params![entry.id, serde_json::to_string(&entry)?],
                    )?;
                }
            }
            let has_profile: bool = source_tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='chronicle_profile')",
                [], |r| r.get(0),
            )?;
            if has_profile {
                let (_, raw) = chronicle_store::read_profile(&source_tx)?;
                if let Some(raw) = raw {
                    tx.execute(
                        "INSERT INTO chronicle_profile(singleton,data) VALUES(1,?1)",
                        [serde_json::to_string(&raw)?],
                    )?;
                }
            }
            let mut stmt = source_tx
                .prepare("SELECT operation,record_id,revision,time FROM audit ORDER BY seq")?;
            for row in stmt.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                ))
            })? {
                let (op, id, revision, time) = row?;
                tx.execute(
                    "INSERT INTO audit(operation,record_id,revision,time) VALUES(?1,?2,?3,?4)",
                    params![op, id, revision, time],
                )?;
            }
        }
        if new_revision != expected + 1 {
            set_allowed_revision(&tx, new_revision)?;
            if tx.execute(
                "UPDATE library SET title=?1,revision=?2 WHERE revision=?3",
                params![restored_title, new_revision, expected + 1],
            )? != 1
            {
                return Err(error(
                    "STALE_REVISION",
                    "The library changed while backup contents were being restored.",
                ));
            }
        } else {
            tx.execute(
                "UPDATE library SET title=?1 WHERE revision=?2",
                params![restored_title, new_revision],
            )?;
        }
        audit(&tx, "restore", None, new_revision)?;
        tx.commit()?;
        self.title = restored_title;
        diagnostics::record("restore", "OK");
        Ok(new_revision)
    }
}

fn display_text(text: &str, role: Role) -> String {
    if role == Role::Storyteller {
        text.into()
    } else {
        importer::redact(&json!(text)).as_str().unwrap_or("").into()
    }
}
fn record_name(record: &Record, role: Role) -> String {
    if role == Role::Storyteller {
        record.name.clone()
    } else {
        record
            .public_fields
            .get("Name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .into()
    }
}
// Schema9 adds a distinct token so already-open schema8 writers cannot ignore mixed receipts.
fn set_allowed_revision(tx: &rusqlite::Transaction<'_>, revision: i64) -> Result<()> {
    if tx.execute(
        "UPDATE purchase_write_guard SET next_revision=?1,schema_revision=?2 WHERE singleton=1",
        params![revision, (!revision) + 9],
    )? != 1
    {
        return Err(error(
            "REVISION_GUARD_MISSING",
            "This library is missing its schema4 revision guard.",
        ));
    }
    Ok(())
}

fn validate_receipt_binding(
    receipt: &crate::purchases::PurchaseReceipt,
    stored_id: &str,
    stored_record_id: &str,
    library_id: &str,
) -> Result<()> {
    let expected_revision = receipt.quote.revision.checked_add(1);
    if receipt.batch.as_ref().is_some_and(|binding| {
        binding.count == 0
            || binding.count > 32
            || binding.index >= binding.count
            || binding.quote_hash.len() != 64
            || !binding
                .quote_hash
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
    }) {
        return Err(error(
            "PURCHASE_RECEIPT_INVALID",
            "A saved purchase batch binding is invalid.",
        ));
    }
    if receipt.quote.request.id != stored_id
        || receipt.quote.record_id != stored_record_id
        || receipt.quote.library_id != library_id
        || expected_revision != Some(receipt.committed_revision)
    {
        return Err(error(
            "PURCHASE_RECEIPT_INVALID",
            "A saved purchase receipt does not match its library, storage key, or committed revision.",
        ));
    }
    if let Some(cancel) = &receipt.cancellation {
        let q = &cancel.quote;
        if q.purchase_id != stored_id
            || q.record_id != stored_record_id
            || q.library_id != library_id
            || q.revision < receipt.committed_revision
            || q.revision.checked_add(1) != Some(cancel.committed_revision)
            || q.refund != receipt.quote.price.price.total
            || q.before_unspent != receipt.quote.after_unspent
            || q.after_unspent != receipt.quote.before_unspent
            || q.request.id == stored_id
        {
            return Err(error(
                "CANCELLATION_RECEIPT_INVALID",
                "A saved cancellation does not match its purchase, library or revision.",
            ));
        }
    }
    Ok(())
}

fn advance(tx: &rusqlite::Transaction<'_>, expected: i64) -> Result<()> {
    let next = expected.checked_add(1).ok_or_else(|| {
        error(
            "REVISION_LIMIT",
            "The library revision cannot be advanced further.",
        )
    })?;
    set_allowed_revision(tx, next)?;
    if tx.execute(
        "UPDATE library SET revision=?1 WHERE revision=?2",
        params![next, expected],
    )? != 1
    {
        return Err(error(
            "STALE_REVISION",
            "The library changed in another window. Refresh before saving.",
        ));
    }
    Ok(())
}
fn insert(tx: &rusqlite::Transaction<'_>, r: &Record) -> Result<()> {
    tx.execute(
        "INSERT INTO records VALUES(?1,?2,?3,?4)",
        params![r.id, r.kind, r.name, serde_json::to_string(r)?],
    )?;
    Ok(())
}
fn audit(tx: &rusqlite::Transaction<'_>, op: &str, id: Option<&str>, rev: i64) -> Result<()> {
    tx.execute(
        "INSERT INTO audit(operation,record_id,revision) VALUES(?1,?2,?3)",
        params![op, id, rev],
    )?;
    Ok(())
}

fn is_character(kind: &str) -> bool {
    matches!(
        kind,
        "Vampire"
            | "Werewolf"
            | "Mortal"
            | "Changeling"
            | "Wraith"
            | "Mage"
            | "Fera"
            | "Various"
            | "Mummy"
            | "KueiJin"
            | "Hunter"
            | "Demon"
            | "Risen"
    )
}
fn set_sheet_field(record: &mut Record, key: &str, value: Value) {
    crate::sheet::write_field(record, key, value);
}

fn apply_basic(record: &mut Record, edit: &Edit) -> Result<()> {
    if edit.name.trim().is_empty() {
        return Err(error(
            "EMPTY_NAME",
            "A character or record name cannot be empty.",
        ));
    }
    record.name = edit.name.trim().into();
    let sanitize = |value: &Value| {
        let safe = importer::redact(value);
        match record.st_markers.as_ref() {
            Some((start, end)) => importer::redact_custom(&safe, start, end),
            None => safe,
        }
    };
    record
        .public_fields
        .insert("Name".into(), sanitize(&json!(record.name)));
    record
        .public_fields
        .insert("Status".into(), sanitize(&json!(edit.status)));
    for (key, value) in [
        ("Name", record.name.as_str()),
        ("Status", edit.status.as_str()),
    ] {
        let raw = json!(value);
        if sanitize(&raw) != raw {
            record.private_fields.insert(key.into(), raw);
        } else {
            record.private_fields.remove(key);
        }
    }
    record
        .private_fields
        .insert("AppNotes".into(), json!(edit.notes));

    Ok(())
}

mod sessions_store;

pub use sessions_store::SessionReceipt;

mod points_store;

mod schedules_store;
pub use schedules_store::ScheduledSession;

mod world_store;

mod association_store;
mod operations_store;
mod plot_store;
mod story_store;

fn private_story_kind(kind: &str) -> bool {
    matches!(kind, "Plot" | "Action" | "Rumor")
}

mod ledger_legacy;

mod player_profile_store;

mod search_store;
pub use search_store::{SavedSearch, SearchIndex};

mod trait_reports_store;

mod boon_store;
mod vampire_status_store;

mod action_links_store;

mod chronicle_store;

mod advancement_draft_store;
mod combined_story_store;
mod published_reference_store;
mod purchase_batch_store;
mod rumor_audience_store;
mod trait_advancement_store;
mod trait_distribution_store;

mod met_reference_store;
mod vampire_creation_store;

mod character_lifecycle_store;

mod mixed_advancement_store;

mod mixed_advancement_draft_store;

mod legacy_profile_store;

mod gex_store;
mod legacy_calendar_store;
mod playtime_review_store;
