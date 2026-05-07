use clap::Parser;
use pef::{ScorerKind, queries::*, utils::init_logger};

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

    /// Block size to use for fixed-size blocks
    #[arg(short, long)]
    block_size: Option<usize>,

    /// Lambda value for variable-size blocks
    #[arg(short, long)]
    lambda: Option<f32>,

    /// Scoring model to use for block upper bounds
    #[arg(long, default_value = "bm25")]
    scorer: ScorerKind,
}

fn main() {
    let args = Args::parse();

    init_logger();

    macro_rules! run {
        ($S:ty) => {
            BlockPostingMetadata::<$S>::create_file(
                &args.input_path.as_str(),
                args.variable_block,
                args.block_size,
                args.lambda,
                &args.out_path.as_str(),
            )
        };
    }

    match args.scorer {
        ScorerKind::Bm25 => run!(BM25),
        ScorerKind::Dot => run!(DotScorer),
    }
}
