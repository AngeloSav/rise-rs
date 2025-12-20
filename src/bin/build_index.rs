use clap::Parser;
use mem_dbg::{DbgFlags, MemDbg, MemSize, SizeFlags};
use pef::indexes::freq_index::InvertedIndex;
use pef::indexes::{BlockInterpolativeIdx, BlockVByteIdx};
use pef::utils::init_logger;
use pef::{EFIdx, IdxKind, OptEFIdx, UPEFIdx, UPISIdx};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path of the collection (without ".docs" or similar)
    #[arg(short, long)]
    input_path: String,

    /// Type of index we want to build
    #[arg(short = 't', long)]
    idx_kind: IdxKind,

    /// Path of the output index
    #[arg(short, long)]
    out_path: String,

    /// Checks the index against the original files
    #[arg(short, long, default_value_t = false)]
    check_correctness: bool,
}

fn main() {
    let args = Args::parse();
    init_logger();

    let input_path = args.input_path;
    let out_path = args.out_path;

    macro_rules! build_idx {
        ($t:path) => {{
            let idx = <$t>::load_or_build_and_save(&input_path, &out_path, true);
            println!(
                "Index contains {} docs, {} terms, size: {} bytes ({} GiB)",
                idx.n_docs(),
                idx.n_terms(),
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
        IdxKind::EFSingle => build_idx!(EFIdx),
        IdxKind::UPEf => build_idx!(UPEFIdx),
        IdxKind::UPIs => build_idx!(UPISIdx),
        IdxKind::Opt => build_idx!(OptEFIdx),
        IdxKind::BlockVByte => build_idx!(BlockVByteIdx),
        IdxKind::BlockInterpolative => build_idx!(BlockInterpolativeIdx),
    }
}
