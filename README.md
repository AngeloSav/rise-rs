# 🌄 RISE — Rust Inverted Search Engine

A high-performance inverted index library written in Rust, implementing **Partitioned Elias-Fano** encoding and related compression schemes for information retrieval.

## Overview

RISE compresses posting lists (document IDs and term frequencies) using entropy-efficient bit sequences and supports multiple ranked and boolean query algorithms, including early-termination strategies (WAND, MaxScore, Block-Max variants) for fast top-k retrieval.

The library targets large-scale collections and is designed for research and benchmarking of inverted index compression and query processing.

## Building

```bash
# Recommended: use the provided justfile
just build

# Equivalent manual command
RUSTFLAGS="-C target-cpu=native" cargo build --release
```

> Requires a nightly Rust toolchain. A `rust-toolchain` file is included to pin the correct version.

## Reproducing the experiments

See [`scripts/EXPERIMENTS.md`](scripts/EXPERIMENTS.md) for a step-by-step guide to downloading the datasets and reproducing the paper's experiments.

## Workflow

There are five binaries. The core pipeline is:

```
build_index  →  create_posting_mdata  →  query / query_eval
```

`index_stats` is available at any point to inspect a built index.

### 1. Build an index

Reads a collection in **DS2I binary format** (`.docs` / `.freqs` pair) and writes a compressed index to disk.

```bash
./target/release/build_index \
    --input-path /path/to/collection \   # without file extension
    --idx-kind   opt \                   # see index types below
    --out-path   /path/to/output.idx \
    [--check-correctness]                # optional: verify encoding
```

**Index types (`--idx-kind`)**

| Value | Description |
|---|---|
| `ef` | Plain Elias-Fano (single partition) |
| `upef` | Uniformly-partitioned Elias-Fano |
| `opt` | Optimally-partitioned indexed sequence |
| `optcomp` | Optimally-partitioned complement Elias-Fano |
| `block_vbyte` | Block-based StreamVByte compression |
| `block_interpolative` | Block-based interpolative coding |

### 2. Build per-block score metadata

Required for ranked queries (WAND, MaxScore, BM-WAND, BM-MaxScore). Precomputes per-block score upper bounds from the raw collection.

```bash
./target/release/create_posting_mdata \
    --input-path /path/to/collection \
    --out-path   /path/to/output.mdata \
    [--variable-block]        # use variable-size blocks (default: fixed)
    [--block-size <n>]        # fixed block size (default from config)
    [--lambda <f>]            # lambda parameter for variable-size partitioning
    [--scorer bm25|dot]       # scoring model (default: bm25)
```

### 3. Run queries (benchmark mode)

Loads an index, reads queries from a file, and benchmarks one or more query algorithms. Outputs JSON lines with latency statistics.

```bash
./target/release/query \
    --index-path  /path/to/output.idx \
    --query-path  /path/to/queries.txt \
    --query-kind  boolean-and,wand,bm-maxscore \
    --meta-path   /path/to/output.mdata \
    --k           10 \
    --n-queries   1000 \
    --n-runs      5 \
    [--index-kind opt]         # inferred from the index file if omitted
    [--scorer bm25|dot]        # inferred from the metadata file if omitted
    [--has-qid]                # if query file contains qid as first element of each line
```

Output includes average query latency (µs), index size (MiB), and a checksum per algorithm.

### 4. Run queries (TREC eval mode)

Produces ranked result lists in standard TREC format for evaluation with `trec_eval`.

```bash
./target/release/query_eval \
    --index-path  /path/to/output.idx \
    --query-path  /path/to/queries.txt \
    --query-kind  bm-maxscore \
    --meta-path   /path/to/output.mdata \
    --k           1000 \
    [--index-kind opt]         # inferred from the index file if omitted
    [--scorer bm25|dot]        # inferred from the metadata file if omitted
    [--n-queries  <n>] \
    [--run-tag    my_run]
```

Query file format for this mode — each line begins with a query ID followed by term IDs:

```
301 42 17 305
302 0 1
303 88 200 14 7
```

Output format per result: `qid Q0 docid rank score run_tag`

### 5. Inspect index statistics

```bash
./target/release/index_stats \
    --index-path /path/to/output.idx \
    [--index-kind opt]         # inferred from the index file if omitted
```

Prints document count, term count, total size in bytes/GiB, and a per-component memory breakdown.

---

**Query algorithms (`--query-kind`)**

| Value | Type | Description |
|---|---|---|
| `boolean-and` | Boolean | Intersection of posting lists |
| `boolean-or` | Boolean | Union of posting lists |
| `ranked-and` | Ranked | Exhaustive top-k over AND result |
| `ranked-or` | Ranked | Exhaustive top-k over OR result |
| `wand` | Ranked | WAND early-termination |
| `maxscore` | Ranked | MaxScore early-termination |
| `bm-wand` | Ranked | Block-Max WAND |
| `bm-maxscore` | Ranked | Block-Max MaxScore |

Multiple algorithms can be passed as a comma-separated list in benchmark mode.

**Scoring models (`--scorer`)**

| Value | Description |
|---|---|
| `bm25` | BM25 (default) |
| `dot` | Raw dot product — no IDF or length normalisation |

The scorer is embedded in the metadata file's type header. Both `query` and `query_eval` infer it automatically — but if you pass `--scorer` explicitly it must match what was used during `create_posting_mdata`.

**Query file format (benchmark mode)**

One query per line; each line is a whitespace-separated list of integer term IDs (0-indexed):

```
42 17 305
0 1
88 200 14 7
```

If the query file contains query-ids (necessary for `query_eval`), it's possible to use `--has-qid` when running performance tests to ignore the frist element of the line.

## Architecture

```
src/
├── bitvector/            Bit vectors (mutable/immutable), BitVecCollection, gamma/unary coding
├── elias_fano/           EF variants: plain, strict, indexed, complement, uniform/optimal partitioned, RankedBv
├── indexes/              InvertedIndex / InvertedIndexBuilder traits; FreqIndex<DocSeq,FreqSeq>;
│   │                     concrete aliases (EFIdx, UPEFIdx, OptEFIdx, OptCompIdx, ...)
│   └── block_freq_index/ Block-codec indexes (StreamVByte, interpolative)
├── queries/              Boolean and ranked operators; BM25 / DotScorer; BlockPostingMetadata; TopKHeap
├── positive_sequences/   PositiveSequence<Base> wraps any sequence to encode frequencies as cumulative sums
├── readers/              BinaryCollectionIterator — DS2I .docs/.freqs reader
├── config.rs             Tuning constants (sampling rates, block sizes, partition parameters)
└── utils.rs              Timing, bit utilities (msb, select_in_word), progress bars
```

### Key traits

| Trait | Role |
|---|---|
| `WriteBitvector` | Encode a sequence into a `BitVec` |
| `SequenceEnumerator` / `NextGEQ` | Iterate posting lists; seek to a value >= target |
| `PartitionableSequence` / `EstimateSpace` | Partitioning and space estimation |
| `DocScorer` | Pluggable scoring function (BM25, DotProduct) |
| `QueryOperator` / `RankedQueryOperator` | Pluggable query execution |
| `InvertedIndex` / `InvertedIndexBuilder` | Index loading and construction |
| `PostingListIter` | Cursor over a single posting list |

### Index type aliases

| Alias | Document encoding |
|---|---|
| `EFIdx` | `EliasFano` |
| `UPEFIdx` | `UniformPartitionedSequence<IndexSequence>` |
| `OptEFIdx` | `OptPartitionedSequence<IndexSequence>` | 
| `OptCompIdx` | `OptPartitionedSequence<IndexCompSequence>` | 
| `BlockVByteIdx` | `StreamVByteCodec` | 
| `BlockInterpolativeIdx` | `InterpolativeCodec` | 

## Authors

- Angelo Savino — a.savino6@studenti.unipi.it
- Rossano Venturini — rossano.venturini@unipi.it
