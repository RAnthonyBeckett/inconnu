# Inconnu 0.48.0-beta.4 — Windows beta

<p align="center">
  <img src="assets/branding/inconnu.png" alt="Inconnu" height="80">
  <br>
  <img src="assets/branding/frakd-up-media.png" alt="Frakd-Up Media" height="90">
</p>

[Download the Windows beta](https://github.com/RAnthonyBeckett/inconnu/releases) · [Beta guide](docs/BETA_QUICKSTART.md)

Product branding: Inconnu
A Frakd-Up Media App
Based on Grapevine by Adam Cerling

Inconnu is a character and Storyteller tool for Mind's Eye Theatre games, and it
runs entirely on your own Windows PC. It opens your existing Grapevine files (GEX and GV3),
so a chronicle can move over without retyping anyone.

While intended for the OWbN gaming audience, Inconnu is not officially sponsored or
supported by OWbN (maybe one day!). It does not approve XP awards or transfers; those still
go through your Storytellers and your chronicle.

## What it does

The beta covers importing and editing characters, reviewed creation and advancement,
an XP ledger with an audit trail, spending previews before anything is bought,
local chronicle tools, Player sheets in English or Portuguese, and backup and recovery for your
library.

## Run it

Extract the whole release ZIP into a folder you can write to, then open
**Open Inconnu.cmd**, which starts **Inconnu.exe**. You do not need an SDK, a cloud account or
a subscription. Choose an existing library or create a new one. While this is a beta, work
on a copy of your real files. [BETA_QUICKSTART.md](docs/BETA_QUICKSTART.md) walks through the
workflows and lists the known limits.

## Beta limits

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

See the beta guide for workflow coverage.

## Build from source

You need stable Rust with the Windows MSVC toolchain, the Microsoft C++ build tools and the
Windows SDK resource compiler (rc.exe). From `app-rust`, run `python tools/build_portable.py`
to build, and `python tools/package_public.py` to package. Source checks are
`cargo test --locked --all-targets`, `cargo clippy --locked --all-targets -- -D warnings` and
`cargo fmt --check`. The compatibility crate and fabricated test files are included, so no
original databases, source books or game art are needed to build or run it.

Powers and creature profiles are data, in `app-rust/data/met_powers.json` and
`app-rust/data/creature_profiles.json`. New powers and book editions can be added there
without new Rust code, though a rebuild is needed after editing them.

## Credits

Inconnu is based on Grapevine by Adam Cerling. Cerling released the Grapevine 3.01 source
code to the community under the Creative Commons Attribution-NonCommercial 2.5 license.
His statement, the license and where to find the released source are in
[LICENSE-GRAPEVINE.md](LICENSE-GRAPEVINE.md).

## License

Inconnu's own code is licensed under the [Apache License 2.0](LICENSE). That license covers
this project's code only. It grants no rights to the original Grapevine software or to World
of Darkness or Mind's Eye Theatre content, which belong to their owners. Third-party
dependency licenses and notices ship with each release package.
