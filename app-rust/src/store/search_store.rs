use super::*;
use crate::searches::{self, Input, Query, Row, Snapshot, Trait};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedSearch {
    pub id: String,
    pub name: String,
    pub query: Query,
}
pub struct SearchIndex {
    pub revision: i64,
    pub entries: Vec<SavedSearch>,
}

pub(super) fn entries(conn: &Connection) -> Result<Vec<SavedSearch>> {
    let mut statement = conn.prepare("SELECT id,data FROM saved_searches ORDER BY id")?;
    let encoded = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let mut entries = Vec::with_capacity(encoded.len());
    for (id, encoded) in encoded {
        let entry: SavedSearch = serde_json::from_str(&encoded)?;
        if entry.id != id
            || id.is_empty()
            || id.len() > 128
            || entry.name.trim().is_empty()
            || entry.name.len() > 256
            || entry.name.contains('\0')
        {
            return Err(error(
                "SEARCH_INVALID",
                "A saved search has invalid identity or name.",
            ));
        }
        searches::validate(&entry.query)?;
        entries.push(entry);
    }
    entries.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then(a.id.cmp(&b.id))
    });
    Ok(entries)
}
fn field<'a>(record: &'a Record, key: &str) -> Option<&'a Value> {
    record
        .private_fields
        .get(key)
        .or_else(|| record.public_fields.get(key))
}
fn text(record: &Record, key: &str) -> String {
    field(record, key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .into()
}
fn npc(record: &Record) -> Option<bool> {
    let value = field(record, "NPC")?;
    value
        .as_bool()
        .or_else(|| value["value"].as_bool())
        .or_else(|| match value.as_str()?.to_ascii_lowercase().as_str() {
            "true" | "yes" | "1" | "-1" => Some(true),
            "false" | "no" | "0" => Some(false),
            _ => None,
        })
}
impl Library {
    pub fn saved_searches(&self, role: Role) -> Result<SearchIndex> {
        require_st(role)?;
        let revision = self.revision()?;
        let entries = entries(&self.conn)?;
        if self.revision()? != revision {
            return Err(error(
                "STALE_REVISION",
                "The library changed. Reload saved searches.",
            ));
        }
        Ok(SearchIndex { revision, entries })
    }
    pub fn save_search(
        &mut self,
        id: Option<&str>,
        name: &str,
        query: &Query,
        expected: i64,
        role: Role,
    ) -> Result<String> {
        require_st(role)?;
        searches::validate(query)?;
        let name = name.trim();
        if name.is_empty() || name.len() > 256 || name.contains('\0') {
            return Err(error(
                "SEARCH_NAME_INVALID",
                "Enter a short saved search name.",
            ));
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let entries = entries(&tx)?;
        if id.is_some_and(|id| !entries.iter().any(|e| e.id == id)) {
            return Err(error(
                "SEARCH_NOT_FOUND",
                "The saved search is unavailable. Reload the list.",
            ));
        }
        if entries
            .iter()
            .any(|e| Some(e.id.as_str()) != id && e.name.to_lowercase() == name.to_lowercase())
        {
            return Err(error(
                "SEARCH_NAME_EXISTS",
                "Select the existing search to update it, or choose another name.",
            ));
        }
        if id.is_none() && entries.len() >= 1000 {
            return Err(error(
                "SEARCH_LIMIT",
                "The library has reached the saved search limit.",
            ));
        }
        let id = id
            .map(str::to_owned)
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let entry = SavedSearch {
            id: id.clone(),
            name: name.into(),
            query: query.clone(),
        };
        advance(&tx, expected)?;
        tx.execute("INSERT INTO saved_searches(id,data) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET data=excluded.data", params![id,serde_json::to_string(&entry)?])?;
        audit(&tx, "save_search", Some(&id), expected + 1)?;
        tx.commit()?;
        Ok(id)
    }
    pub fn delete_search(&mut self, id: &str, expected: i64, role: Role) -> Result<i64> {
        require_st(role)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        advance(&tx, expected)?;
        if tx.execute("DELETE FROM saved_searches WHERE id=?1", [id])? != 1 {
            return Err(error(
                "SEARCH_NOT_FOUND",
                "The saved search is unavailable. Reload the list.",
            ));
        }
        audit(&tx, "delete_search", Some(id), expected + 1)?;
        tx.commit()?;
        Ok(expected + 1)
    }
    pub fn search_snapshot(
        &self,
        query: &Query,
        include_players: bool,
        role: Role,
    ) -> Result<Snapshot> {
        require_st(role)?;
        searches::validate(query)?;
        let revision = self.revision()?;
        let records = self.records()?;
        let players: BTreeMap<_, _> = records
            .iter()
            .filter(|r| r.kind == "Player" && r.build.is_none())
            .map(|r| (r.id.as_str(), record_name(r, Role::Storyteller)))
            .collect();
        let mut rows = Vec::new();
        for record in records
            .iter()
            .filter(|r| r.build.is_none() && crate::sheet::is_character_kind(&r.kind))
        {
            let status = text(record, "Status");
            let row = Row {
                id: record.id.clone(),
                name: record_name(record, Role::Storyteller),
                family: record.kind.clone(),
                player: record
                    .player_id
                    .as_deref()
                    .and_then(|id| players.get(id))
                    .cloned()
                    .unwrap_or_else(|| text(record, "Player")),
                status: if status.is_empty() {
                    text(record, "PlayStatus")
                } else {
                    status
                },
                group: text(record, "Group"),
                subgroup: text(record, "Subgroup"),
                npc: npc(record),
            };
            let traits = crate::sheet::extract(record)
                .lists
                .into_iter()
                .map(|(key, value)| {
                    let traits = value["Traits"]
                        .as_array()
                        .map(|rows| {
                            rows.iter()
                                .map(|row| Trait {
                                    name: row["Name"].as_str().unwrap_or_default().into(),
                                    rating: row["Value"]
                                        .as_str()
                                        .and_then(|value| value.trim().parse::<i64>().ok()),
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    (key, traits)
                })
                .collect();
            let input = Input { row, traits };
            if searches::matches(query, &input) {
                let mut row = input.row;
                if !include_players {
                    row.player.clear();
                }
                rows.push(row);
            }
        }
        rows.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then(a.id.cmp(&b.id))
        });
        if self.revision()? != revision {
            return Err(error(
                "STALE_REVISION",
                "The library changed. Run the search again.",
            ));
        }
        let mut projected_query = query.clone();
        if !include_players {
            for clause in &mut projected_query.clauses {
                if clause.field == searches::Field::Player && !clause.value.is_empty() {
                    clause.value = "[private filter]".into();
                }
            }
        }
        Ok(Snapshot {
            title: self.title.clone(),
            revision,
            query: projected_query,
            include_players,
            rows,
        })
    }
}
