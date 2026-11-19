# Phase 7B checkpoint — November 18, 2026

## Scope delivered

The Phase 7B Python extractor reuses Phase 7A helpers and checks local PDF identities
against the reviewed source inventory. It cross-checks headings against the legacy menu
and generated 572 published records, 598 MET definitions and 25 new OWbN selectors.
The public data carries names, ranks, page coordinates, authority and source hashes.
Previous catalogue definitions remain byte-compatible as serialized records.

| Profile | Reviewed routes and source scope |
| --- | --- |
| Wraith | The requested local Psyche budget uses three Basic Arcanoi. Oblivion, printed p. 63 and PDF page 64, supplies the resource baseline, and its quick-start list specifies five Arcanoi. Shadow allocation requires its own review. Twenty-two Wr20 Thorn price records use the original point cost as Shadow XP, with explicit Storyteller confirmation to record spending in the existing local ledger. |
| Risen | Standalone creation and reviewed transcription from a Wraith sheet use the local starting profile. Potency limits starting Pathos, and the recorded inherited Corpus applies to the new Risen. The original Wraith and its ledger stay with the source record. Twenty natural Discipline entries bind to the original MET power pages and use the 2019 Risen packet cost schedule. |
| Werewolf | Revised Garou creation derives Gnosis, Rage and Willpower from breed, auspice and tribe. The paid Gift and Rite schedules come from Laws of the Wild (Revised), printed p. 173. The W20 extraction includes three Kinfolk Gifts as references. |
| Mage | Laws of Ascension supplies forty-five native Sphere ranks and the core starting pools. Avatar sets starting Quintessence and Arete caps starting Sphere rank. Six example Rotes use the per-rank XP rule. |
| Mortal | Ordinary mortal creation uses Laws of the Hunt (Revised), with native psychic Numina and selected sorcery rituals priced from its XP table. |
| Hunter | Six special Edges are searchable from the 2021 Imbued packet. The base MET creation and Edge advancement tables in Laws of the Reckoning are still needed to approve the profile. |

The catalogues cover reviewed subsets. Remaining work includes Hunter core Creed Edges
and pricing, Mage M20 and packet Rotes and Tradition options, additional Mortal paths and
rituals, additional W20 and packet variants, and bylaw cases that require conditional
membership, use, or rank predicates. Risen conversion uses an explicit transcription
review. A segregated Shadow XP balance is outside this implementation.

## Package verification

- Version 0.48.0-beta.3 was built with `tools/build_portable.py` and prepared with `tools/package_public.py`.
- The portable `Inconnu.exe` is Windows x64 and its executable version resources report 0.48.0-beta.3.
- The archive contains 562 files, including the manifest and `LICENSE-GRAPEVINE.md`. Each of the 561 manifest payloads matched its recorded size and SHA-256. Package checks screened for private workspace paths and excluded source PDFs, raw research and private libraries.
- `Inconnu-0.48.0-beta.3-windows-x64.zip` contains 10,596,758 bytes. SHA-256 `ee590cb2c842ec043ebb38246772f9e5a53eab31ce15eab2d5aed389689c5141`.
- Portable executable SHA-256 `0075f0f6307605e1d5628728841106553c24340124ae70a3901bedf1136ffeeb`.

## Source verification

The final Rust gate passed 470 tests across 104 targets with strict Clippy and
formatting. The focused tests cover reviewed creation, starting Free Trait math,
both purchase lists, exact original-cost refunds, separate Shadow review and
specific Risen Coordinator evidence. The unreviewed profiles and synthetic fixtures
retain their review gates. The frozen Vampire catalogue digest is
`7b5126a8085d7f72b5db720a542762443611bd7e348dc10844943d041f11d6c4`.
Regenerating both catalogues from the reviewed caches reproduced the files byte for byte.

## Native verification

The relocated beta.3 package executable passed all 152 synthetic native checks
at both 100% and 125% text size. The purchase smoke probe now targets the
clickable portion of a clipped control. A fresh 100% run completed after a
transient mixed-plan smoke timeout.
