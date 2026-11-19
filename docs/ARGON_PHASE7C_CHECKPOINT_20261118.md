# Argon Phase 7C checkpoint · November 18, 2026

Phase 7C adds reviewed creation and advancement for four families. Hunter remains gated until the Imbued creation and Edge XP tables from *Laws of the Reckoning* can be checked. The 2021 Imbued packet contributes the six existing reference-only Edges. The new Shih selector covers the separate OWbN PC restriction. Various remains the intentionally unreviewed umbrella profile for local Storyteller rules.

## Reviewed creation and advancement

- **Fera.** The starting template covers Homid and Ursine Gurahl Arcas. The template requires recorded breed review and uses the Gurahl creation tables with the explicitly adopted Revised MET Gift and Rite XP schedules. The source batch contains 87 Gurahl Gifts and five Rites. Other Fera breeds still require their own creation tables and catalogue review.
- **Kuei-Jin.** The template covers starting Dharma, the six-point Yin/Yang allocation and separate permanent soul and Chi pools. It adds ten Smoke and Storm Shintai powers and 32 Rites. The 2021 packet's purchase tier headings are distinct from the OWbN rarity categories. Higher starting Dharma and the rest of the Shintai catalogue remain outside the reviewed batch.
- **Demon.** The starting Fallen template derives Willpower from Virtues and Torment from House. The batch covers 11 reviewed Common and House Lore entries and 48 packet Rituals. Each Ritual needs its own source XP cost. Other Lore families remain to be reviewed.
- **Mummy.** The first-Balance Amenti template requires review of the mortal stage and the favored Hekau. It covers 30 Hekau ranks and 151 Spells and Rituals with prerequisite and Balance checks. The MET creation and XP rules control prices, including the nonfavored adjustment shown in the 2024 Genre packet. The Magic packet is used for the catalogue and has a differing nonfavored pricing description. The optional Ba and Ka Backgrounds start at zero unless allocated.

## Vampire sources and OWbN policy

The batch adds 132 priced Vampire Ritual definitions, including 17 native Revised MET Thaumaturgy rituals and 115 V20/DA20 ritual entries. The additional blood-magic ritual schedule is 2/4/6 XP under the applicable OWbN rules. For the 20th Anniversary entries, the printed level is kept and the MET tier is labeled as a conversion. The 35 V20/DA20 bloodline and path entries are published references. Native Vampire creation is unchanged. The ritual batch uses the existing secondary purchase flow.

The policy table gains 42 sourced selectors. These include breed, Kuei-Jin and Rite categories, Fallen rank, selected Mummy dynasties and Hekau, and the Hunter Shih restriction. Staff must still review conditional cases beyond the exact selectors. [OWbN Character Bylaws](https://council.owbn.net/en/charter-bylaws/character-bylaws/)

## Reproduction and verification

The Python extractor reuses the Phase 7B source and PDF layout helpers. It compares normalized names against the legacy menu XML and regenerates 541 published rows and 506 MET definitions. Both catalogue files reproduced byte for byte. The generated data contains public metadata and page locators.

- Focused tests cover creation and Free Trait math, both purchase lists, original-cost refunds, policy and starting-resource gates, and source citation resolution.
- The compatibility check preserves the original 40 native Vampire catalogue entries and SHA-256 digest **7b5126a8085d7f72b5db720a542762443611bd7e348dc10844943d041f11d6c4**.
- All-target verification passed 473 tests across 105 targets. Strict Clippy and formatting passed. The first gate exposed an outdated serialization fixture and a Clippy match guard warning. Both were corrected and the complete gate was rerun.
- The release and native UI checks are deferred to the beta.4 packaging session. Latest reported starting allowance was 42% five-hour and 29% weekly.

## Remaining evidence

The Hunter blocker is an indexed, readable copy of *Laws of the Reckoning* (5037) covering character creation, Creed Edges and advancement. The current books and library index did not provide these tables. Hunter creation and automated Edge pricing remain gated. The request to verify complete creation and both purchase lists for all 12 concrete families therefore remains open.
