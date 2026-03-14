use clap::Parser;
use pef::{
    queries::{BlockPostingMetadata, bm25::BM25},
    utils::init_logger,
};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path of the base directory containing the index files
    #[arg(short, long)]
    input_path: String,

    /// Output metadata file
    #[arg(short, long)]
    out_path: String,

    /// Flag to use variable-size blocks (default: false, i.e., use fixed-size blocks)
    #[arg(short, long, default_value_t = false)]
    variable_block: bool,
}

fn main() {
    let args = Args::parse();

    init_logger();

    BlockPostingMetadata::<BM25>::create_file(
        &args.input_path.as_str(),
        args.variable_block,
        &args.out_path.as_str(),
    );
}
