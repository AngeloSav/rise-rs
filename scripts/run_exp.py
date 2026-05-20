#!/usr/bin/env python3
"""
Experiment runner supporting TOML nested tables:

[env]
[global]
[ef]              → group defaults
[ef.cc_url]       → experiment
[ef.blockmax_static]
...

Paths in TOML files may use the placeholder {RISE_DATA_DIR}, which is
substituted at load time.  Set it via --base-dir or the RISE_DATA_DIR
environment variable.
"""

import os
import sys
import subprocess
import pprint
from pathlib import Path

try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib


def build_args(values: dict) -> list:
    args = []
    for key, value in values.items():
        if key == "bin":
            continue

        if value is None:
            continue

        if isinstance(value, list):
            for item in value:
                args.append(f"--{key}={item}")
            continue

        if value == "":
            args.append(f"--{key}")
            continue

        args.append(f"--{key}={value}")

    return args


def main(path: str, dry=False, base_dir: str = ""):
    # Resolve {RISE_DATA_DIR} placeholder
    if base_dir:
        os.environ["RISE_DATA_DIR"] = base_dir
    base_dir = os.environ.get("RISE_DATA_DIR", "")

    # Load TOML, substituting {RISE_DATA_DIR} before parsing
    raw = Path(path).read_text()
    if "{RISE_DATA_DIR}" in raw:
        if not base_dir:
            print(
                "ERROR: TOML uses {RISE_DATA_DIR} but neither --base-dir nor the "
                "RISE_DATA_DIR environment variable is set.",
                file=sys.stderr,
            )
            sys.exit(1)
        raw = raw.replace("{RISE_DATA_DIR}", base_dir.rstrip("/"))
    cfg = tomllib.loads(raw)

    # 1. Set environment variables
    env = cfg.get("env", {})
    for k, v in env.items():
        os.environ[k] = str(v)

    # 2. Global defaults
    global_defaults = cfg.get("global", {})

    # 3. Groups = all top-level tables except env/global
    groups = {
        name: vals for name, vals in cfg.items()
        if name not in ("env", "global") and isinstance(vals, dict)
    }

    # 4. Identify experiments (nested subtables)
    experiments = []   # list of (group_name, exp_name, values)

    for gname, gvals in groups.items():
        # Anything nested is an experiment
        for subname, subvals in gvals.items():
            if isinstance(subvals, dict):
                experiments.append((gname, subname, subvals))

    if not experiments:
        print("No experiments found (you need subtables like [ef.cc_url])", file=sys.stderr)
        return 0

    # 5. Run experiments
    for group, exp, exp_vals in experiments:
        merged = {}

        # global
        merged.update(global_defaults)

        # group-level defaults
        for k, v in groups[group].items():
            if not isinstance(v, dict):  # skip subtables (experiments)
                if v is None:
                    merged.pop(k, None)
                else:
                    merged[k] = v

        # experiment-level
        for k, v in exp_vals.items():
            if v is None:
                merged.pop(k, None)
            else:
                merged[k] = v

        # must have bin
        if "bin" not in merged:
            print(f"Skipping {group}.{exp}: no 'bin' found after merging", file=sys.stderr)
            pprint.pprint(merged, stream=sys.stderr)
            continue

        bin_path = merged["bin"]
        args = build_args(merged)
        cmd = [bin_path] + args

        print("\n================================================", file=sys.stderr)
        print(f"Experiment: {group}.{exp}", file=sys.stderr)
        print("Merged config:", file=sys.stderr)
        pprint.pprint(merged, stream=sys.stderr)
        print("Command:", file=sys.stderr)
        print(" ".join(cmd), file=sys.stderr)

        if dry:
            print("(dry-run)", file=sys.stderr)
            continue

        try:
            subprocess.run(cmd, check=True)
        except Exception as e:
            print(f"Error running {group}.{exp}: {e}", file=sys.stderr)


if __name__ == "__main__":
    args = sys.argv[1:]
    dry_run = "--dry-run" in args
    args = [a for a in args if a != "--dry-run"]

    base_dir = ""
    for a in args:
        if a.startswith("--base-dir="):
            base_dir = a.split("=", 1)[1]
            args.remove(a)
            break

    if not args:
        print("Usage: run_exp.py [--base-dir=PATH] [--dry-run] <config.toml>", file=sys.stderr)
        sys.exit(1)

    cfg_path = args[0]
    main(cfg_path, dry=dry_run, base_dir=base_dir)
