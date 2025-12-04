#!/usr/bin/env python3
"""
Experiment runner supporting TOML nested tables:

[env]
[global]
[ef]              → group defaults
[ef.cc_url]       → experiment
[ef.blockmax_static]
...
"""

import os
import sys
import subprocess
import pprint

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


def main(path: str, dry=False):
    # Load TOML
    with open(path, "rb") as f:
        cfg = tomllib.load(f)

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
        print("No experiments found (you need subtables like [ef.cc_url])")
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
            print(f"Skipping {group}.{exp}: no 'bin' found after merging")
            pprint.pprint(merged)
            continue

        bin_path = merged["bin"]
        args = build_args(merged)
        cmd = [bin_path] + args

        print("\n================================================")
        print(f"Experiment: {group}.{exp}")
        print("Merged config:")
        pprint.pprint(merged)
        print("Command:")
        print(" ".join(cmd))

        if dry:
            print("(dry-run)")
            continue

        try:
            subprocess.run(cmd, check=True)
        except Exception as e:
            print(f"Error running {group}.{exp}: {e}")


if __name__ == "__main__":
    dry_run = "--dry-run" in sys.argv
    cfg_path = sys.argv[1]
    main(cfg_path, dry=dry_run)
