use clap::Parser;
use mem_dbg::{DbgFlags, MemDbg, MemSize, SizeFlags};
use rise::indexes::InvertedIndex;
use rise::indexes::*;
use rise::utils::init_logger;
use rise::{IdxKind, peek_idx_kind};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path of the index file
    #[arg(short, long)]
    index_path: String,

    /// Type of index (inferred from the file header if omitted)
    #[arg(short = 't', long)]
    idx_kind: Option<IdxKind>,
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
                idx.n_docs(),
                idx.n_terms(),
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

    let idx_kind = args.idx_kind.unwrap_or_else(|| peek_idx_kind(&index_path));

    match idx_kind {
        IdxKind::EFSingle => build_idx!(EFIdx),
        IdxKind::UPEf => build_idx!(UPEFIdx),
        IdxKind::Opt => build_idx!(OptEFIdx),
        IdxKind::BlockVByte => build_idx!(BlockVByteIdx),
        IdxKind::BlockInterpolative => build_idx!(BlockInterpolativeIdx),
        IdxKind::OptComp => build_idx!(OptCompIdx),
    }
}
