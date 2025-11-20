use clap::Parser;
use mem_dbg::{DbgFlags, MemDbg, MemSize, SizeFlags};
use pef::space_usage::SpaceUsage;
use pef::utils::init_logger;
use pef::{EFIdx, IdxKind, OptEFIdx, UPEFIdx, UPISIdx};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path of the index file
    #[arg()]
    index_path: String,

    /// Type of index
    #[arg()]
    idx_kind: IdxKind,
}

fn main() {
    let args = Args::parse();
    init_logger();

    let index_path = args.index_path;

    macro_rules! build_idx {
        ($t:path) => {{
            let idx = <$t>::load_index(&index_path);
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
    }
}
