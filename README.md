# 🔥 FIIR — Fast Inverted Indexes in Rust

A high-performance inverted index library written in Rust, implementing **Partitioned Elias-Fano** encoding and related compression schemes for information retrieval.

## Overview

FIIR compresses posting lists (document IDs and term frequencies) using entropy-efficient bit sequences and supports multiple ranked and boolean query algorithms, including early-termination strategies (WAND, MaxScore, Block-Max variants) for fast top-k retrieval.

The library targets large-scale collections (Gov2, ClueWeb09, CC-News) and is designed for research and benchmarking of inverted index compression and query processing.

## Building

```bash
# Recommended: use the provided justfile
just build

# Equivalent manual command
RUSTFLAGS="-C target-cpu=native" cargo build --release
```

## Workflow

There are three binaries, which are run in sequence:

```
build_index  →  create_posting_mdata  →  query
```

### 1. Build an index

Reads a collection in **DS2I binary format** (`.docs` / `.freqs` pair) and writes a compressed index to disk.

```bash
./target/release/build_index \
    --input-path /path/to/collection \   # without file extension
    --idx-kind   opt \                   # see index types below
    --out-path   /path/to/output.pef.out \
    [--check-correctness]                # optional: verify encoding
```

**Index types (`--idx-kind`)**

| Value | Description |
|---|---|
| `ef` | Plain Elias-Fano (single partition) |
| `upef` | Uniform-partitioned Elias-Fano |
| `upis` | Uniform-partitioned with `IndexedSequence` (auto-selects best encoding per block) |
| `opt` | Optimally-partitioned Elias-Fano |
| `optcomp` | Optimally-partitioned with complement Elias-Fano |
| `block_vbyte` | Block-based VByte compression |
| `block_interpolative` | Block-based interpolative coding |

### 2. Build per-block score metadata

Required for ranked queries (WAND, MaxScore, BM-WAND, BM-MaxScore). Precomputes BM25 upper bounds per block.

```bash
./target/release/create_posting_mdata \
    --input-path /path/to/collection \
    --out-path   /path/to/output.mdata \
    [--variable-block]                  # use variable-size blocks (default: fixed)
```

### 3. Run queries

Loads an index, reads queries from a file, and benchmarks one or more query algorithms.

```bash
./target/release/query \
    --index-kind  opt \
    --index-path  /path/to/output.pef.out \
    --query-path  /path/to/queries.txt \
    --query-kind  boolean-and,wand,bm-maxscore \
    --meta-path   /path/to/output.mdata \
    --k           10 \
    --n-queries   1000 \
    --n-runs      5
```

**Query algorithms (`--query-kind`, comma-separated)**

| Value | Type | Description |
|---|---|---|
| `boolean-and` | Boolean | Intersection of posting lists |
| `boolean-or` | Boolean | Union of posting lists |
| `ranked-and` | Ranked | Top-k over AND result |
| `ranked-or` | Ranked | Top-k over OR result |
| `wand` | Ranked | WAND early-termination |
| `maxscore` | Ranked | MaxScore early-termination |
| `bm-wand` | Ranked | Block-Max WAND |
| `bm-maxscore` | Ranked | Block-Max MaxScore |

Query results are printed as JSON lines to stdout, including average query latency (µs), index size (MiB), and a checksum.

#### Query file format

One query per line; each line is a whitespace-separated list of integer term IDs (0-indexed):

```
42 17 305
0 1
88 200 14 7
```

## Architecture

```
src/
├── bitvector/          Bit vectors (mutable/immutable),
├── elias_fano/         EF and partitioned variants, complement EF, RankedBv
├── indexes/            Generic InvertedIndex trait; concrete implementations
│   └── block_freq_index/  Block-codec indexes (VByte, interpolative)
├── queries/            Boolean and ranked query operators; BM25 scorer; top-k heap
├── positive_sequences/ Frequency encoding as cumulative sums
├── readers/            DS2I binary collection reader
├── config.rs           Tuning constants (sampling rates, block sizes)
└── utils.rs            Timers, bit utilities, progress bars
```

### Key traits

| Trait | Role |
|---|---|
| `WriteBitvector` | Encode a sequence into a `BitVec` |
| `SequenceEnumerator` / `NextGEQ` | Iterate posting lists; seek to ≥ a value |
| `PartitionableSequence` / `EstimateSpace` | Partitioning and space estimation |
| `DocScorer` | Pluggable scoring function |
| `QueryOperator` / `RankedQueryOperator` | Pluggable query execution |

## Authors

- Angelo Savino — a.savino6@studenti.unipi.it
- Rossano Venturini — rossano.venturini@unipi.it
