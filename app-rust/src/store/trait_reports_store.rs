use super::*;
use crate::{
    searches::Query,
    trait_reports::Snapshot,
    trait_statistics::{self, Character, Entry, Kind},
};
use std::collections::BTreeMap;

fn source_has_list(record: &Record, key: &str) -> bool {
    if record.private_fields.contains_key(key) || record.public_fields.contains_key(key) {
        return true;
    }
    let Some(unmapped) = record
        .private_fields
        .get("UnmappedTraitLists")
        .or_else(|| record.public_fields.get("UnmappedTraitLists"))
    else {
        return false;
    };
    let candidates: Vec<_> = unmapped
        .as_array()
        .map(|rows| rows.iter().collect())
        .unwrap_or_else(|| vec![unmapped]);
    candidates.iter().any(|list| {
        list["Name"]
            .as_str()
            .and_then(crate::importer::xml_list_key)
            == Some(key)
    })
}
pub(super) struct TraitProjection {
    pub characters: Vec<Character>,
    pub omitted_lists: usize,
    pub unnamed_entries: usize,
}

impl Library {
    pub(super) fn trait_projection(
        &self,
        search: &crate::searches::Snapshot,
        selected_lists: &[&str],
    ) -> Result<TraitProjection> {
        let mut characters = Vec::with_capacity(search.rows.len());
        let records: BTreeMap<_, _> = self
            .records()?
            .into_iter()
            .map(|record| (record.id.clone(), record))
            .collect();
        let mut omitted_lists = 0usize;
        let mut unnamed_entries = 0usize;
        let mut total_entries = 0usize;
        for row in &search.rows {
            let record = records.get(&row.id).ok_or_else(|| {
                error(
                    "STALE_REVISION",
                    "The search records changed. Run the search again.",
                )
            })?;
            let sheet = crate::sheet::extract(record);
            let mut lists = BTreeMap::new();
            for key in selected_lists {
                let Some(list) = sheet.lists.get(*key) else {
                    if source_has_list(record, key) {
                        omitted_lists += 1;
                    }
                    continue;
                };
                let mut entries = Vec::new();
                for trait_row in list["Traits"].as_array().into_iter().flatten() {
                    total_entries += 1;
                    if total_entries > 200_000 {
                        return Err(error(
                            "TRAIT_STATISTICS_LIMIT",
                            "The selected trait lists exceed the report limit.",
                        ));
                    }
                    let name = trait_row["Name"].as_str().unwrap_or_default();
                    if name.trim().is_empty() {
                        unnamed_entries += 1;
                        continue;
                    }
                    entries.push(Entry {
                        name: name.into(),
                        value: trait_row["Value"].as_str().unwrap_or_default().into(),
                    });
                }
                lists.insert((*key).into(), entries);
            }
            characters.push(Character {
                name: row.name.clone(),
                family: row.family.clone(),
                lists,
            });
        }
        Ok(TraitProjection {
            characters,
            omitted_lists,
            unnamed_entries,
        })
    }

    pub fn trait_statistics_snapshot(
        &self,
        query: &Query,
        include_players: bool,
        kind: Kind,
        role: Role,
    ) -> Result<Snapshot> {
        require_st(role)?;
        let search = self.search_snapshot(query, include_players, role)?;
        let projected = self.trait_projection(&search, trait_statistics::selected_lists(kind))?;
        let report = trait_statistics::build(&projected.characters, kind)?;
        if self.revision()? != search.revision {
            return Err(error(
                "STALE_REVISION",
                "The library changed. Run the search again.",
            ));
        }
        Ok(Snapshot {
            search,
            kind,
            report,
            omitted_lists: projected.omitted_lists,
            unnamed_entries: projected.unnamed_entries,
        })
    }
}
