use super::*;
use crate::{
    searches::{Clause, Comparison, Field, Mode, Query},
    vampire_status::{self, Boon, Character, Snapshot, Trait},
};
use std::collections::BTreeMap;

fn text(record: &Record, key: &str) -> String {
    record
        .private_fields
        .get(key)
        .or_else(|| record.public_fields.get(key))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .into()
}
fn status_source_present(record: &Record) -> bool {
    if record.public_fields.contains_key("StatusList")
        || record.private_fields.contains_key("StatusList")
    {
        return true;
    }
    let Some(value) = record
        .private_fields
        .get("UnmappedTraitLists")
        .or_else(|| record.public_fields.get("UnmappedTraitLists"))
    else {
        return false;
    };
    let values: Vec<_> = value
        .as_array()
        .map(|rows| rows.iter().collect())
        .unwrap_or_else(|| vec![value]);
    values.iter().any(|list| list["Name"] == "Status")
}
impl Library {
    pub fn vampire_status_snapshot(
        &self,
        status: &str,
        include_boons: bool,
        include_descriptions: bool,
        role: Role,
    ) -> Result<Snapshot> {
        require_st(role)?;
        let clause = |field, value| Clause {
            field,
            comparison: Comparison::Equal,
            negate: false,
            value,
            trait_name: String::new(),
            number: 0,
        };
        let mut query = Query {
            mode: Mode::All,
            clauses: vec![clause(Field::Family, "Vampire".into())],
        };
        if !status.trim().is_empty() {
            query
                .clauses
                .push(clause(Field::Status, status.trim().into()));
        }
        let search = self.search_snapshot(&query, false, role)?;
        let records: BTreeMap<_, _> = self
            .records()?
            .into_iter()
            .map(|record| (record.id.clone(), record))
            .collect();
        let mut characters = Vec::with_capacity(search.rows.len());
        for row in &search.rows {
            let record = records.get(&row.id).ok_or_else(|| {
                error(
                    "STALE_REVISION",
                    "The selected characters changed. Prepare again.",
                )
            })?;
            let sheet = crate::sheet::extract(record);
            let statuses = sheet
                .lists
                .get("StatusList")
                .and_then(|list| list["Traits"].as_array())
                .into_iter()
                .flatten()
                .map(|entry| Trait {
                    name: entry["Name"].as_str().unwrap_or_default().into(),
                    value: entry["Value"].as_str().unwrap_or_default().into(),
                })
                .collect();
            let boons = if include_boons {
                self.boon_snapshot(&row.id, role)?
                    .entries
                    .into_iter()
                    .map(|entry| Boon {
                        kind: entry.edit.kind,
                        partner: entry.edit.partner,
                        owed_by_holder: entry.edit.owed_by_holder,
                        day: crate::xp::day_valid(&entry.edit.day).then_some(entry.edit.day),
                        description: if include_descriptions {
                            entry.edit.description
                        } else {
                            String::new()
                        },
                    })
                    .collect()
            } else {
                Vec::new()
            };
            characters.push(Character {
                name: row.name.clone(),
                title: text(record, "Title"),
                group: text(record, "Group"),
                statuses,
                unreadable_status: !sheet.lists.contains_key("StatusList")
                    && status_source_present(record),
                boons,
            });
        }
        if self.revision()? != search.revision {
            return Err(error(
                "STALE_REVISION",
                "The library changed. Prepare again.",
            ));
        }
        Ok(vampire_status::Snapshot {
            search,
            include_boons,
            include_descriptions: include_boons && include_descriptions,
            characters,
        })
    }
}
