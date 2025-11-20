use clap::Parser;
use pef::{
    indexes::freq_index::{DocList, FreqIndex, FreqList},
    queries::{
        And, BMMaxScore, BMWand, BlockPostingMetadata, MaxScore, Or, QueryOperator, RankedAnd,
        RankedOr, Wand,
    },
    space_usage::SpaceUsage,
    utils::{init_logger, TimingQueries},
    EFIdx, IdxKind, OptEFIdx, QueryKind, UPEFIdx, UPISIdx,
};
use std::io::BufRead;
use std::path::Path;
use std::{fs, io::BufReader, time::Duration};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Type of index we want to build
    #[arg(long)]
    index_kind: IdxKind,

    /// Path of the index file
    #[arg(long)]
    index_path: String,

    /// Path of the file containing the queries
    #[arg(long)]
    query_path: String,

    /// Query algorithms we want to use
    #[arg(long, value_delimiter = ',')]
    query_kind: Vec<QueryKind>,

    /// path of the metadata file containing the data used for scoring
    #[arg(short, long)]
    meta_path: Option<String>,

    /// Retrieve the top k documents
    #[arg(short, long, default_value_t = 10)]
    k: usize,

    /// Process the first n queries, if not given process all queries
    #[arg(short, long)]
    n_queries: Option<usize>,

    /// Perform test n times
    #[arg(short = 'r', long, default_value_t = 10)]
    n_runs: usize,
}

#[inline(always)]
fn perform_query<'a, Q: QueryOperator, T, S>(
    idx: &'a FreqIndex<T, S>,
    parsed_queries: &Vec<Vec<usize>>,
    mut query_strategy: Q,
    n_runs: usize,
    index_ty: &str,
    mdata_filename: &str,
) where
    T: DocList<'a>,
    S: FreqList<'a>,
{
    log::info!("starting testing! query type: {}", Q::query_name());

    let n_queries = parsed_queries.len();
    let mut timer = TimingQueries::new(n_runs, parsed_queries.len());

    //warmup
    let mut check = 0;
    for term in parsed_queries {
        check += query_strategy.query(&idx, term);
    }
    log::info!("check_warmup: {}", check);

    for _ in 0..n_runs {
        // log::info!("run {}/{}", i + 1, n_runs);
        // check = 0;
        timer.start();

        for term in parsed_queries {
            check += query_strategy.query(&idx, term);
        }
        timer.stop();
    }

    println!(
        "RESULT {} [exp={}, index_ty={}, n_queries={}, avg={:?}, mdata_filename={}, space_usage_MiB={:.2}]",
        check,
        Q::query_name(),
        index_ty,
        n_queries,
        Duration::from_nanos(timer.get().2.try_into().unwrap()),
        mdata_filename,
        idx.space_usage_MiB()
    );
}

fn main() {
    let args = Args::parse();

    init_logger();

    let queries_file =
        BufReader::new(fs::File::open(args.query_path).expect("can't open query file"));

    let queries = if let Some(x) = args.n_queries {
        queries_file
            .lines()
            .take_while(|a| a.is_ok())
            .take(x)
            .collect::<Vec<_>>()
    } else {
        queries_file.lines().collect::<Vec<_>>()
    };

    let parsed: Vec<_> = queries
        .into_iter()
        .map(|l| {
            l.unwrap()
                .split_whitespace()
                .map(|x| x.parse::<usize>().expect("can't parse number"))
                .collect::<Vec<_>>()
        })
        .collect();

    let n_runs = args.n_runs;

    macro_rules! query_idx {
        ($t:path) => {{
            let idx = <$t>::load_index(&args.index_path);
            log::info!("Index contains {} docs, {} terms", idx.n_docs, idx.n_terms);

            let p_data = BlockPostingMetadata::<pef::queries::bm25::BM25>::load_file(
                &args.meta_path.clone().expect("meta path not given"),
            );

            let index_ty = stringify!($t);
            let mdata_filename = Path::new(args.meta_path.as_ref().unwrap())
                .file_name()
                .unwrap()
                .to_str()
                .unwrap();

            for &qk in &args.query_kind {
                match qk {
                    QueryKind::BooleanAnd => {
                        perform_query(&idx, &parsed, And, n_runs, index_ty, mdata_filename)
                    }
                    QueryKind::BooleanOr => {
                        perform_query(&idx, &parsed, Or, n_runs, index_ty, mdata_filename)
                    }
                    QueryKind::RankedAnd => {
                        let r_and = RankedAnd::new(&p_data, args.k);
                        perform_query(&idx, &parsed, r_and, n_runs, index_ty, mdata_filename);
                    }
                    QueryKind::RankedOr => {
                        let r_or = RankedOr::new(&p_data, args.k);
                        perform_query(&idx, &parsed, r_or, n_runs, index_ty, mdata_filename);
                    }
                    QueryKind::Wand => {
                        let wand = Wand::new(&p_data, args.k);
                        perform_query(&idx, &parsed, wand, n_runs, index_ty, mdata_filename);
                    }
                    QueryKind::Maxscore => {
                        let maxscore = MaxScore::new(&p_data, args.k);
                        perform_query(&idx, &parsed, maxscore, n_runs, index_ty, mdata_filename);
                    }
                    QueryKind::BMWand => {
                        let bmwand = BMWand::new(&p_data, args.k);
                        perform_query(&idx, &parsed, bmwand, n_runs, index_ty, mdata_filename);
                    }
                    QueryKind::BMMaxscore => {
                        let bmmaxscore = BMMaxScore::new(&p_data, args.k);
                        perform_query(&idx, &parsed, bmmaxscore, n_runs, index_ty, mdata_filename);
                    }
                }
            }
        }};
    }

    match args.index_kind {
        IdxKind::EFSingle => query_idx!(EFIdx),
        IdxKind::UPEf => query_idx!(UPEFIdx),
        IdxKind::UPIs => query_idx!(UPISIdx),
        IdxKind::Opt => query_idx!(OptEFIdx),
    }
}
