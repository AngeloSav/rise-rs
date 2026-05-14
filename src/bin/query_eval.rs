use clap::Parser;
use rise::{
    IdxKind, QueryKind, ScorerKind, indexes::*, peek_idx_kind, peek_scorer_kind, queries::*,
    utils::init_logger,
};
use std::{
    fs,
    io::{BufRead, BufReader},
};

#[derive(Parser, Debug)]
#[command(
    version,
    about = "Run ranked queries and output results in TREC eval format"
)]
struct Args {
    /// Type of index (inferred from the file header if omitted)
    #[arg(short = 't', long)]
    index_kind: Option<IdxKind>,

    /// Path of the index file
    #[arg(short, long)]
    index_path: String,

    /// Ranked query algorithm to use
    #[arg(long)]
    query_kind: QueryKind,

    /// Path of the metadata file for scoring
    #[arg(short, long)]
    meta_path: String,

    /// Path of the query file (each line: "qid term1 term2 ...")
    #[arg(long)]
    query_path: String,

    /// Retrieve the top k documents
    #[arg(short, long, default_value_t = 10)]
    k: usize,

    /// Process the first n queries; if omitted, process all
    #[arg(short, long)]
    n_queries: Option<usize>,

    /// Tag identifying this run in the TREC output
    #[arg(long, default_value = "RISE")]
    run_tag: String,

    /// Scoring model to use (inferred from the metadata file if omitted)
    #[arg(long)]
    scorer: Option<ScorerKind>,
}

fn run_and_print<Q>(mut op: Q, idx: &impl InvertedIndex, qid: &str, terms: &[usize], run_tag: &str)
where
    Q: QueryOperator + RankedQueryOperator,
{
    op.query(idx, terms);

    // into_sorted_vec() is ascending (min-heap order); reverse for descending score
    let results = op.topk().into_sorted_vec();
    // results.reverse();

    for (rank, doc) in results.iter().enumerate() {
        println!(
            "{} Q0 {} {} {:.6} {}",
            qid,
            doc.docid,
            rank + 1,
            doc.frequency,
            run_tag
        );
    }
}

fn main() {
    let args = Args::parse();
    init_logger();

    let file = BufReader::new(fs::File::open(&args.query_path).expect("cannot open query file"));
    let mut lines = file.lines().map(|l| l.expect("read error"));

    let queries: Vec<(String, Vec<usize>)> = (&mut lines)
        .take(args.n_queries.unwrap_or(usize::MAX))
        .filter_map(|line| {
            let mut tokens = line.split_whitespace();
            let qid = tokens.next()?.to_owned();
            let terms: Vec<usize> = tokens
                .map(|t| t.parse::<usize>().expect("term ID must be an integer"))
                .collect();
            if terms.is_empty() {
                None
            } else {
                Some((qid, terms))
            }
        })
        .collect();

    log::info!("loaded {} queries", queries.len());

    macro_rules! eval_idx {
        ($t:path, $S:ty) => {{
            let idx = <$t>::load_index(&args.index_path);
            log::info!("index: {} docs, {} terms", idx.n_docs(), idx.n_terms());

            let p_data = BlockPostingMetadata::<$S>::load_file(&args.meta_path);

            for (qid, terms) in &queries {
                match args.query_kind {
                    QueryKind::RankedAnd => run_and_print(
                        RankedAnd::new(&p_data, args.k),
                        &idx,
                        qid,
                        terms,
                        &args.run_tag,
                    ),
                    QueryKind::RankedOr => run_and_print(
                        RankedOr::new(&p_data, args.k),
                        &idx,
                        qid,
                        terms,
                        &args.run_tag,
                    ),
                    QueryKind::Wand => {
                        run_and_print(Wand::new(&p_data, args.k), &idx, qid, terms, &args.run_tag)
                    }
                    QueryKind::Maxscore => run_and_print(
                        MaxScore::new(&p_data, args.k),
                        &idx,
                        qid,
                        terms,
                        &args.run_tag,
                    ),
                    QueryKind::BMWand => run_and_print(
                        BMWand::new(&p_data, args.k),
                        &idx,
                        qid,
                        terms,
                        &args.run_tag,
                    ),
                    QueryKind::BMMaxscore => run_and_print(
                        BMMaxScore::new(&p_data, args.k),
                        &idx,
                        qid,
                        terms,
                        &args.run_tag,
                    ),
                    QueryKind::BooleanAnd | QueryKind::BooleanOr => {
                        eprintln!(
                            "error: boolean queries are not supported; choose a ranked query kind"
                        );
                        std::process::exit(1);
                    }
                }
            }
        }};
    }

    macro_rules! with_scorer {
        ($idx_ty:path, $scorer:expr) => {
            match $scorer {
                ScorerKind::Bm25 => eval_idx!($idx_ty, BM25),
                ScorerKind::Dot => eval_idx!($idx_ty, DotScorer),
            }
        };
    }

    let index_kind = args
        .index_kind
        .unwrap_or_else(|| peek_idx_kind(&args.index_path));
    let scorer = args
        .scorer
        .unwrap_or_else(|| peek_scorer_kind(&args.meta_path));

    match index_kind {
        IdxKind::EFSingle => with_scorer!(EFIdx, scorer),
        IdxKind::UPEf => with_scorer!(UPEFIdx, scorer),
        IdxKind::Opt => with_scorer!(OptEFIdx, scorer),
        IdxKind::OptComp => with_scorer!(OptCompIdx, scorer),
        IdxKind::BlockVByte => with_scorer!(BlockVByteIdx, scorer),
        IdxKind::BlockInterpolative => with_scorer!(BlockInterpolativeIdx, scorer),
    }
}
