#!/usr/bin/env python3

import tomllib
import os
import subprocess
import sys

def build_args(values):
    args = []
    for k, v in values.items():
        if k == "bin":
            continue

        # Handle lists
        if isinstance(v, list):
            args.append(f"--{k}={','.join(v)}")
        # Empty string → flag
        elif v == "":
            args.append(f"--{k}")
        else:
            args.append(f"--{k}={v}")
    return args

def main(toml_path):
    # Load TOML
    with open(toml_path, "rb") as f:
        data = tomllib.load(f)

    # Set environment variables
    for k, v in data.get("env", {}).items():
        os.environ[k] = str(v)

    global_values = data.get("global", {})

    # Run experiments
    for section, values in data.items():
        if section in ("env", "global"):
            continue

        # Merge global values first
        merged_values = dict(global_values)
        merged_values.update(values)

        # Determine bin
        if "bin" not in merged_values:
            print(f"Skipping {section}: missing 'bin'")
            continue

        bin_path = merged_values.pop("bin")  # Remove from args

        args = build_args(merged_values)
        cmd = [bin_path] + args

        print(f"\nExperiment name: {section} ---------------------------", file=sys.stderr)
        print(f">>> Running command: {' '.join(cmd)}\n\n", file=sys.stderr)
        subprocess.run(cmd, check=True)

if __name__ == "__main__":
    if len(sys.argv) < 2:
        print(f"Usage: python {sys.argv[0]} <config.toml ...>")
        sys.exit(1)

    for file in sys.argv[1:]:
        main(file)
