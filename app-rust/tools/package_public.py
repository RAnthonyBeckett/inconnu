"""Package a portable beta without developer journals, raw logs or private inputs."""
import argparse
import hashlib
import json
import os
import shutil
import subprocess
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
VERSION = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))["package"]["version"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-root", type=Path, default=ROOT.parent / "releases")
    out = parser.parse_args().output_root.resolve() / ("working-build-" + VERSION)
    out.mkdir(parents=True, exist_ok=False)
    target=Path(os.environ.get("CARGO_TARGET_DIR", str(ROOT / "target")))
    if not target.is_absolute(): target=ROOT / target
    shutil.copy2(target / "release/Inconnu.exe", out)
    shutil.copy2(ROOT / "Open Inconnu.cmd", out)
    for source, name in [("PUBLIC_README.md", "README.md"),
                         ("PUBLIC_VALIDATION.md", "VALIDATION.md"),
                         ("BETA_QUICKSTART.md", "BETA_QUICKSTART.md"),
                         ("CONFIGURATION.md", "CONFIGURATION.md")]:
        shutil.copy2(ROOT.parent / "docs" / source, out / name)
        if name == "README.md":
            readme = out / name
            readme.write_text(readme.read_text(encoding="utf-8").replace("../assets/", "assets/"), encoding="utf-8")
    shutil.copy2(ROOT.parent / "LICENSE", out / "LICENSE.txt")
    shutil.copy2(ROOT.parent / "LICENSE-GRAPEVINE.md", out)
    shutil.copytree(ROOT.parent / "assets/branding", out / "assets/branding")
    examples = out / "Synthetic Examples"
    examples.mkdir()
    shutil.copy2(ROOT / "tests/fixtures/synthetic-game.gv3", examples / "Synthetic Game.gv3")
    (examples / "Example.gex").write_text(
        '<?xml version="1.0"?><grapevine version="3"><player name="Example Player" id="p1"/>'
        '<vampire name="Example Character" player="Example Player" id="c1" status="Draft">'
        '<notes>Fabricated private Storyteller note.</notes></vampire></grapevine>', encoding="utf-8")
    (examples / "README.txt").write_text("Fabricated compatibility examples only.\n", encoding="utf-8")
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--locked", "--offline", "--format-version", "1"], cwd=ROOT))
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    wanted, pending = set(), [metadata["resolve"]["root"]]
    while pending:
        node_id = pending.pop()
        if node_id in wanted:
            continue
        wanted.add(node_id)
        pending.extend(dep["pkg"] for dep in nodes[node_id]["deps"]
                       if any(kind["kind"] != "dev" for kind in dep["dep_kinds"]))
    licenses = out / "Third-party licenses"
    licenses.mkdir()
    notices = ["# Component notices", "", "Dependency licenses do not grant rights to third-party source content.", ""]
    for package in sorted(metadata["packages"], key=lambda p: (p["name"], p["version"])):
        if package["id"] not in wanted or not package["source"]:
            continue
        target = licenses / (package["name"] + "-" + package["version"])
        target.mkdir()
        for path in Path(package["manifest_path"]).parent.iterdir():
            if path.is_file() and path.name.upper().startswith(("LICENSE", "LICENCE", "COPYING", "NOTICE", "COPYRIGHT")):
                shutil.copy2(path, target / path.name)
        notices.append(f'- {package["name"]} {package["version"]}: {package.get("license")}; '
                       f'{package.get("repository") or package.get("homepage") or "crates.io"}')
    sysroot = Path(subprocess.check_output(["rustc", "--print", "sysroot"], text=True).strip())
    stdlib = licenses / "Rust standard library"
    stdlib.mkdir()
    for name in ("LICENSE-MIT", "LICENSE-APACHE", "COPYRIGHT"):
        path = sysroot / "share/doc/rust" / name
        if path.is_file():
            shutil.copy2(path, stdlib / name)
    (out / "THIRD_PARTY_NOTICES.md").write_text("\n".join(notices) + "\n", encoding="utf-8")
    manifest = {p.relative_to(out).as_posix(): {"bytes": p.stat().st_size,
                "sha256": hashlib.sha256(p.read_bytes()).hexdigest()}
                for p in sorted(out.rglob("*")) if p.is_file()}
    (out / "PACKAGE_MANIFEST.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    print(f"Packaged {len(manifest)} verified files for {VERSION}")


if __name__ == "__main__":
    main()
