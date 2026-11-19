# Inconnu 0.48.0-beta.4 configuration

app-rust/data/met_powers.json defines source-reviewed powers, creature scope, target list,
family/branch, stable identity, tier, pricing profile and exact source coordinates. Existing
legacy previews are compatibility snapshots; their frozen regression hashes protect old
receipts and wire formats. New powers omit legacy enum bindings. No new per-creature Rust
modules or enum variants are required to add a catalogue entry.

app-rust/data/creature_profiles.json declares thirteen creature profiles, identity/power
fields, creation pools, Free Trait costs, starting resources and XP bands. Primary and
secondary lists can have separate tier and affiliation costs. A reviewed configured profile needs valid book/system/edition,
page and PDF identity metadata and explicit local review. Runtime tables are compiled data;
a data update requires rebuilding the application. Exact legacy book scope is retained.

Published-source books support validated edition tags and explicit template_scope metadata.
A published dot level is never implicitly converted to a MET tier. Wraith branch labels
and other power-tree differences belong to catalogue data, without invented new powers.

Configured creation preserves supplied resources, never derives Vampire resource formulas
for another creature, and signs every applied sheet field. Resource-sharing changes require
separate main-editor review. Above-base spending, secondary starting powers and specializations require configured
prices and explicit review. New receipt list
bindings are persisted so refunds use the original list/cost even if catalogue data changes.

Reviewed creation and catalogue purchases cover Vampire, Werewolf, Mortal, Changeling,
Wraith, Mage, Fera, Mummy, Kuei-Jin, Hunter, Demon, and Risen. Various is the local-rules
umbrella and requires its own Storyteller configuration. The profiles cover defined
starting templates and powers, including Gurahl Fera and first-Balance Amenti. Hunter
uses a locally adopted 2021 Imbued packet and legacy menu baseline. Individual MET
book citations from Laws of the Reckoning (#5037) are pending.

Storytellers can export Haller-safe .gex XML, enable OWbN Character and Mechanics
Bylaws checks with packet authority, and load House Rules Packs. The packs configure
Challenge Pool Math for sheets. Export a Player Mobile Sheet Bundle as
.inconnu-sheet.json for a portable sheet without private Storyteller data.

Keep private inputs, original libraries, raw diagnostics and developer journals out of
source/release archives. Compile release binaries using build_portable.py and package with
package_public.py. Use only fabricated data in tests.
