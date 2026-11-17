# Inconnu 0.48.0-beta.2 — Windows Vampire beta

[Download the Windows beta](https://github.com/RAnthonyBeckett/inconnu/releases/tag/v0.48.0-beta.2) · [Beta guide](docs/BETA_QUICKSTART.md)

Product branding: Inconnu
A Frakd-Up Media App

A local Windows character and Storyteller utility. This beta supports legacy GEX/GV3
import from original Grapevine formats, character editing, reviewed Vampire
creation/advancement, XP ledgers and audit, spending previews, local chronicle tools,
sheets and backup/recovery.

## Run

Extract the entire release ZIP into a writable folder and open **Open Inconnu.cmd**. The
launcher starts **Inconnu.exe**.
No SDK, VM, cloud account or subscription is required. Choose or create your own library.
Use a testing copy during beta. Read BETA_QUICKSTART.md for the workflows and known limits.

## Build from source

Install stable Rust with the Windows MSVC toolchain and Microsoft C++ build tools and the Windows SDK resource compiler (rc.exe).
From app-rust, run `python tools/build_portable.py`. This retains the static Windows C
runtime and replaces workspace/user-directory paths in compiler output with neutral labels.
Run `cargo test --locked --all-targets`, `cargo clippy --locked --all-targets -- -D warnings`
and `cargo fmt --check` for source checks. Package with `python tools/package_public.py`.
The compatibility crate and fabricated fixtures are included. No original databases,
source books, third-party game art, developer journals or local library are needed to build or run.

## Beta scope

Windows and local Vampire play. Rules/content and original Grapevine parity remain partial;
network authentication and game-to-game sharing are future work. Local modes are not
identity security. This is not an official OWbN tool or approval of awards/transfers.
Source attribution stays with supported mechanics. Source books are not bundled.
Dependency licenses and notices accompany release packages; no new license grant over
third-party Grapevine or World of Darkness content is asserted here.

## Shared creature configuration

The shared catalogue and workflows now use the compiled source definitions in app-rust/data/met_powers.json and
app-rust/data/creature_profiles.json. Editing these definitions requires rebuilding. Forty existing reviewed Vampire powers retain their stable
identities, prices, saved wire formats and receipt/refund behavior. New catalogue powers
use stable IDs rather than requiring new Rust power-enum variants. Published book edition
tags and explicit template scopes can be added as data.

All thirteen imported character kinds have profile-driven forms and synthetic shared
purchase/refund coverage. Non-Vampire profile defaults are proposals, not verified rules:
creation and automated pricing remain blocked until the appropriate rules and source
citations are reviewed. Unpriced creation allocations and unsupported cases remain blocked.
This refactor adds no new real creature powers and makes no full-rule or organization-approval claim.
