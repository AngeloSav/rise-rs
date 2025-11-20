use clap::Parser;
use pef::{
    queries::{bm25::BM25, BlockPostingMetadata},
    utils::init_logger,
};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path of the base directory containing the index files
    #[arg()]
    base_path: String,

    /// Flag to use variable-size blocks (default: false, i.e., use fixed-size blocks)
    #[arg(short, long, default_value_t = false)]
    variable_block: bool,

    /// Output metadata file
    #[arg()]
    out_file: String,
}

fn main() {
    let args = Args::parse();

    init_logger();

    BlockPostingMetadata::<BM25>::create_file(
        args.base_path.as_str(),
        args.variable_block,
        args.out_file.as_str(),
    );
}
