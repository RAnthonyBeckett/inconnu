# Inconnu contributor instructions

Use Inconnu as the current product name. Refer to Grapevine only for historical source,
format and compatibility context.

Attribution is required, never optional: Inconnu is based on Grapevine by Adam Cerling,
whose 3.01 source was released under CC BY-NC 2.5. Keep the "Based on Grapevine by Adam
Cerling" credit in README.md and docs/PUBLIC_README.md, keep LICENSE-GRAPEVINE.md in the
repo, and ship LICENSE-GRAPEVINE.md in every release package beside LICENSE.txt.

Keep source IDs, original imports, historical XP/receipt data and transactional safety.
Keep Player output separate from private Storyteller evidence. Local modes are not authentication.
Preserve English/Portuguese language independence. Unknown rules require explicit review.
Use fabricated fixtures only; never commit libraries, original game files, source books,
credentials, raw diagnostics or local machine journals. Compile distributable binaries with
app-rust/tools/build_portable.py. Keep packages portable and omit developer-specific paths.
Run locked tests, strict Clippy and formatting for changes; preserve scoped native/recovery proof.
