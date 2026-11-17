# Inconnu shared configuration contract

app-rust/data/met_powers.json defines source-reviewed powers, creature scope, target list,
family/branch, stable identity, tier, pricing profile and exact source coordinates. Existing
legacy previews are compatibility snapshots; their frozen regression hashes protect old
receipts and wire formats. New powers omit legacy enum bindings. No new per-creature Rust
modules or enum variants are required to add a catalogue entry.

app-rust/data/creature_profiles.json declares thirteen creature profiles, identity/power
fields, creation pool proposals, XP bands and tier/affiliation costs. Proposed defaults
are not source approval. A reviewed configured profile needs valid book/system/edition,
page and PDF identity metadata and explicit local review. Runtime tables are compiled data;
a data update requires rebuilding the application. Exact legacy book scope is retained.

Published-source books support validated edition tags and explicit template_scope metadata.
A published dot level is never implicitly converted to a MET tier. Wraith branch labels
and other power-tree differences belong to catalogue data, without invented new powers.

Configured creation preserves supplied resources, never derives Vampire resource formulas
for another creature, and signs every applied sheet field. Resource-sharing changes require
separate main-editor review. Unknown rules, unpriced above-base spending, secondary starting
powers and specializations remain explicitly blocked rather than guessed. New receipt list
bindings are persisted so refunds use the original list/cost even if catalogue data changes.

Keep private inputs, original libraries, raw diagnostics and developer journals out of
source/release archives. Compile release binaries using build_portable.py and package with
package_public.py. Use only fabricated data in tests.
