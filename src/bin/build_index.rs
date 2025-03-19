use clap::Parser;
use mem_dbg::{DbgFlags, MemDbg, MemSize, SizeFlags};
use pef::{
    elias_fano::{
        indexed_seq::{IndexSequence, StrictSequence},
        opt_partition::OptPartitionedSequence,
        strict_ef::StrictEliasFano,
        uniform_partitioned_seq::UniformPartitionedSequence,
    },
    indexes::freq_index::FreqIndex,
    positive_sequences::positive_sequence::PositiveSequence,
    space_usage::SpaceUsage,
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
            println!(
                "Index contains {} docs, {} terms, size: {} bytes ({} GiB)",
                idx.n_docs,
                idx.n_terms,
                idx.space_usage_byte(),
                idx.space_usage_GiB()
            );

            println!(
                "memsize says: {} bytes ({} GiB)",
                idx.mem_size(SizeFlags::default()),
                idx.mem_size(SizeFlags::default()) as f64 / (1024.0 * 1024.0 * 1024.0)
            );

            if args.check_correctness {
                idx.check_correctness(&input_path)
            }

            println!("memdbg output: ");
            // more verbose output
            // idx.mem_dbg(DbgFlags::default() | DbgFlags::HUMANIZE)
            //     .expect("error memdbg");
            idx.mem_dbg(DbgFlags::empty() | DbgFlags::PERCENTAGE)
                .expect("error memdbg");
        }};
    }

    match args.idx_kind {
        IdxKind::EFSingle => build_idx!(FreqIndex<EliasFano, PositiveSequence<StrictEliasFano>>),
        IdxKind::UPEf => {
            build_idx!(
                FreqIndex<
                    UniformPartitionedSequence<EliasFano>,
                    PositiveSequence<UniformPartitionedSequence<StrictEliasFano>>,
                >
            )
        }
        IdxKind::UPIs => {
            build_idx!(
                FreqIndex<
                    UniformPartitionedSequence<IndexSequence>,
                    PositiveSequence<UniformPartitionedSequence<StrictSequence>>,
                >
            )
        }
        IdxKind::Opt => {
            build_idx!(
                FreqIndex<
                    OptPartitionedSequence<IndexSequence>,
                    PositiveSequence<OptPartitionedSequence<StrictSequence>>,
                >
            )
        }
    }
}
