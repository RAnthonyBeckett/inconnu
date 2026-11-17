"""Run Cargo with portable compiler paths, retaining the Windows static C runtime."""
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def main():
    flags = ["-C", "target-feature=+crt-static"] if os.name == "nt" else []
    roots = [(ROOT.parent.resolve(), "/workspace/inconnu"),
             (Path.home().resolve(), "/build/user")]
    for key in ("CARGO_HOME", "RUSTUP_HOME", "CARGO_TARGET_DIR", "TEMP", "TMP"):
        if os.environ.get(key):
            path = Path(os.environ[key])
            if not path.is_absolute():
                path = ROOT / path
            roots.append((path.resolve(), "/build/" + key.lower()))
    for source, replacement in sorted(roots, key=lambda item: len(str(item[0])), reverse=True):
        for spelling in sorted({str(source), source.as_posix()}):
            flags.append(f"--remap-path-prefix={spelling}={replacement}")
    env = dict(os.environ)
    env.pop("RUSTFLAGS", None)
    env["CARGO_ENCODED_RUSTFLAGS"] = "\x1f".join(flags)
    args = sys.argv[1:] or ["build", "--release", "--locked"]
    return subprocess.run(["cargo", *args], cwd=ROOT, env=env).returncode


if __name__ == "__main__":
    raise SystemExit(main())
