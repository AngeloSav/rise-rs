use clap::{Parser, ValueEnum};
use pef::{
    elias_fano::{
        indexed_seq::IndexedSequence, opt_partition::OptPartitionedSequence,
        uniform_partitioned_seq::UniformPartitionedSequence,
    },
    indexes::freq_index::FreqIndex,
    EliasFano, IdxKind,
};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path of the collection (without ".docs" or similar)
    #[arg()]
    input_path: String,

    /// Type of index we want to build
    #[arg()]
    idx_kind: IdxKind,

    /// Path of the output index (optional)
    #[arg(short, long)]
    out_path: Option<String>,

    /// Rebuilds the index
    #[arg(short, long, default_value_t = false)]
    force_rebuild: bool,

    /// Checks the index against the original files
    #[arg(short, long, default_value_t = false)]
    check_correctness: bool,
}

fn main() {
    let args = Args::parse();

    let input_path = args.input_path;

    let out_path = match args.out_path {
        Some(x) => x,
        None => {
            let tail = match args.idx_kind {
                IdxKind::EFSingle => "ef",
                IdxKind::UPEf => "upef",
                IdxKind::UPIs => "upis",
                IdxKind::Opt => "opt",
            };
            format!("{}.{}.out", input_path, tail)
        }
    };

    macro_rules! build_idx {
        ($t:path) => {{
            let idx = <$t>::load_or_build_and_save(&input_path, &out_path, args.force_rebuild);
            println!("Index contains {} docs, {} terms", idx._n_docs, idx.n_terms);

            if args.check_correctness {
                idx.check_correctness(&input_path)
            }
        }};
    }

    const FIXED_BLOCK_SIZE: usize = 512;

    match args.idx_kind {
        IdxKind::EFSingle => build_idx!(FreqIndex<EliasFano, _>),
        IdxKind::UPEf => {
            build_idx!(FreqIndex<UniformPartitionedSequence<EliasFano, _, FIXED_BLOCK_SIZE>, _>)
        }
        IdxKind::UPIs => {
            build_idx!(
                FreqIndex<UniformPartitionedSequence<IndexedSequence, _, FIXED_BLOCK_SIZE>, _>
            )
        }
        IdxKind::Opt => {
            build_idx!(FreqIndex<OptPartitionedSequence<IndexedSequence, _>, _>)
        }
    }
}
