use super::*;
use crate::{
    searches::Query,
    trait_distribution::{self, Request},
    trait_distribution_report::Snapshot,
};

impl Library {
    pub fn trait_distribution_snapshot(
        &self,
        query: &Query,
        include_players: bool,
        request: &Request,
        role: Role,
    ) -> Result<Snapshot> {
        require_st(role)?;
        trait_distribution::build(&[], request)?;
        let search = self.search_snapshot(query, include_players, role)?;
        let projected = self.trait_projection(&search, &[request.list.as_str()])?;
        let report = trait_distribution::build(&projected.characters, request)?;
        if self.revision()? != search.revision {
            return Err(error(
                "STALE_REVISION",
                "The library changed. Run the search again.",
            ));
        }
        let snapshot = Snapshot {
            search,
            request: request.clone(),
            report,
            omitted_lists: projected.omitted_lists,
            unnamed_entries: projected.unnamed_entries,
        };
        if serde_json::to_vec(&snapshot)?.len() > 8 * 1024 * 1024 {
            return Err(error(
                "TRAIT_DISTRIBUTION_LIMIT",
                "Narrow the search before preparing this trait distribution.",
            ));
        }
        Ok(snapshot)
    }
}
