# Reproducing the Experiments

## Dataset layout

The datasets are available on Hugging Face at:

> **https://huggingface.co/datasets/AngeloSav/rise-datasets**

> **Warning:** the total size of the datasets is approximately **500 GB**. We recommend downloading them (or a subset of them) directly to an external or secondary disk and symlinking from there.

Download and extract them so the folder is at `/path/to/your/data` (or symlink any path to it):

```bash
pip install huggingface_hub

# Full download (both original and RGB-reordered collections)
hf download AngeloSav/rise-datasets --repo-type dataset --local-dir /path/to/your/data

# Original collections only (skip RGB — re-run step 1 yourself)
hf download AngeloSav/rise-datasets --repo-type dataset --local-dir /path/to/your/data \
  --include "*.bin" "*.queries"

# RGB-reordered collections only (skip step 1)
hf download AngeloSav/rise-datasets --repo-type dataset --local-dir /path/to/your/data \
  --include "*.bin.rgb" "*.queries"

# Symlink if the data is on a different disk:
ln -s /path/to/your/data ~/rise_data
```

The expected layout after download:

```
~/rise_data/
├── clueweb/
│   ├── clueweb.bin        # original DS2I input (.docs / .freqs pair)
│   ├── clueweb.bin.rgb    # RGB-reordered version (produced by step 1)
│   └── clueweb.queries
├── ccnews/
│   ├── ccnews.bin
│   ├── ccnews.bin.rgb
│   └── ccnews.queries
└── built_indexes/         # populated by steps 2–3 (build_index, create_posting_mdata)
```

Create the output directory before running:

```bash
mkdir -p ~/rise_data/built_indexes
```

If your data lives elsewhere, pass `--base-dir` explicitly to `run_exp.py` (see below).

---

## Building

All binaries must be compiled before running any experiment script.

```bash
just build        # from the repo root
```

The scripts reference binaries via relative paths (`../target/release/…`), so **run all `run_exp.py` commands from the `scripts/` subdirectory**.

---

## Running experiments

`run_exp.py` accepts an optional `--base-dir` argument that sets the root of the dataset tree. If omitted, it reads the `RISE_DATA_DIR` environment variable. All TOML paths use the `{RISE_DATA_DIR}` placeholder which is substituted at load time.

```bash
cd scripts/

# Either set the env var once …
export RISE_DATA_DIR=~/rise_data

# … or pass it per-invocation:
# python run_exp.py --base-dir=~/rise_data <config.toml>
```

Add `--dry-run` to any command to print the resolved commands without executing them.

---

## Step 1 — RGB reordering (optional)

Reorders the raw collection with graph-bisection to improve compression. Skip this step and edit the `input-path` in the build configs to point at the original `.bin` files instead of the `.bin.rgb` files if you want to index the unordered collection.

```bash
python run_exp.py rgb/perform_rgb.toml
```

---

## Step 2 — Build indexes

Each TOML builds all index variants for one collection. Uncomment the variants you want in the file before running.

```bash
python run_exp.py build_indexes/cw09_build_all.toml
python run_exp.py build_indexes/cc_build_all.toml
```

---

## Step 3 — Build score metadata

Required for ranked queries (WAND, MaxScore, BM-WAND, BM-MaxScore). Run after building the indexes.

```bash
python run_exp.py build_mdata/cw09.toml
python run_exp.py build_mdata/cc.toml
```

Each config produces a `static` metadata file (fixed 128-element blocks) and/or a `variable` one (variable-size blocks via λ, default 12). Uncomment the relevant sections as needed.

---

## Step 4 — Run query experiments

Outputs JSON-lines with per-algorithm latency statistics to stdout. Redirect to a file to save results.

```bash
python run_exp.py query_experiments/cw09.toml   > results/cw09.jsonl
python run_exp.py query_experiments/ccnews.toml > results/ccnews.jsonl
```

Each file runs all enabled index/algorithm combinations. Comment out groups you have not built. The `k` value (top-k) and `n-queries` / `n-runs` can be overridden in `[global]`.
