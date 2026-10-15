"""Verify standalone embedded help, with no source tree or runtime resources.

Usage: python tools/check-cli-help.py [target/debug]
Build first with: cargo build --workspace --bins
"""

import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile


COMMANDS = {
    "zas": ["compile", "check", "link", "run"],
    "zcobc": ["compile", "run", "check", "inventory"],
    "zpli": ["compile", "run"],
    "zjob": ["submit", "catalog", "ims"],
    "zcov": ["report"],
    "zbms": [],
    "zcics": [],
    "zims": ["inspect", "import-fixed", "tm-list"],
    "zmq": ["put", "get"],
    "zconn": ["serve", "call", "script"],
    "zdeps": ["analyze", "masters"],
    "zeus-rt": [],
    "zeus-ccvs": ["extract", "run", "report", "manifest"],
    "zeus-hercdiff": ["run", "compare"],
}


def main():
    bin_dir = Path(sys.argv[1] if len(sys.argv) > 1 else "target/debug").resolve()
    suffix = ".exe" if os.name == "nt" else ""
    count = 0
    with tempfile.TemporaryDirectory(prefix="zeus-help-") as temp:
        root = Path(temp)
        env = os.environ.copy()
        for key in list(env):
            if key.startswith("ZEUS_") or key.startswith("DD_"):
                del env[key]
        env["ZEUS_HOME"] = str(root / "must-not-be-created")
        for command, subcommands in COMMANDS.items():
            # Only the executable is copied: include_str! manuals must not
            # depend on relative paths, adjacent files or the source tree.
            exe = root / (command + suffix)
            shutil.copy2(bin_dir / exe.name, exe)
            cases = [["--help"], ["-h"]]
            cases += [[subcommand, "--help"] for subcommand in subcommands]
            if command in ("zcics", "zconn"):
                cases.append(["--csd", "missing.csd", "--help"])
            for args in cases:
                before = set(root.iterdir())
                result = subprocess.run(
                    [str(exe), *args], cwd=root, env=env,
                    capture_output=True, text=True, timeout=10,
                )
                label = " ".join([command, *args])
                assert result.returncode == 0, (label, result.stderr)
                assert not result.stderr, (label, result.stderr)
                assert len(result.stdout) >= 800, (label, "help too short")
                assert "EXAMPLES" in result.stdout, (label, "examples missing")
                assert "EXIT STATUS" in result.stdout, (label, "exit status missing")
                assert set(root.iterdir()) == before, (label, "help created files")
                count += 1
            exe.unlink()
    print(f"{count} standalone help invocations passed across {len(COMMANDS)} commands")


if __name__ == "__main__":
    main()
