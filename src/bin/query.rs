use clap::{command, Parser};
use pef::{
    indexes::freq_index::{DocList, FreqIndex, FreqList},
    queries::{And, Or, PostingMetadata, QueryOperator, RankedAnd, WAND},
    space_usage::SpaceUsage,
    utils::TimingQueries,
    EFIdx, IdxKind, OptEFIdx, QueryKind, UPEFIdx, UPISIdx,
};
use std::io::BufRead;
use std::{fs, io::BufReader, time::Duration};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Type of index we want to build
    #[arg()]
    idx_kind: IdxKind,

    // Query operator we want to use
    #[arg()]
    query_kind: QueryKind,

    /// Path of the index file
    #[arg()]
    index_path: String,

    /// Path of the file containing the queries
    #[arg()]
    query_path: String,

    /// path of the `.sizes` file containing the data used for scoring
    #[arg(short, long)]
    sizes_path: Option<String>,

    /// Retrieve the top k documents
    #[arg(short, long)]
    k: Option<usize>,

    /// Process the first n queries
    #[arg(short, long)]
    n_queries: Option<usize>,
}

#[inline(always)]
fn perform_query<'a, Q: QueryOperator, T, S>(
    idx: &'a FreqIndex<T, S>,
    parsed_queries: Vec<Vec<usize>>,
    mut query_strategy: Q,
) where
    T: DocList<'a>,
    S: FreqList<'a>,
{
    println!("starting testing!");

    let n_runs = 10;
    let n_queries = parsed_queries.len();
    let mut timer = TimingQueries::new(n_runs, parsed_queries.len());

    let mut res_vec = vec![0; idx.n_docs];

    //warmup
    let mut check = 0;
    for term in &parsed_queries {
        check += query_strategy.query(&idx, term, &mut res_vec);
    }
    println!("check_warmup: {}", check);

    for _ in 0..n_runs {
        // check = 0;
        timer.start();

        for term in &parsed_queries {
            check += query_strategy.query(&idx, term, &mut res_vec);
        }
        timer.stop();
    }

    println!(
        "RESULT {} [exp=boolean_and, n_queries={}, min={:?}, max={:?}, avg={:?}, space_usage_MiB={:.2}]",
        check,
        n_queries,
        Duration::from_nanos(timer.get().0.try_into().unwrap()),
        Duration::from_nanos(timer.get().1.try_into().unwrap()),
        Duration::from_nanos(timer.get().2.try_into().unwrap()),
        idx.space_usage_MiB()
    );
}

fn main() {
    let args = Args::parse();

    let queries_file =
        BufReader::new(fs::File::open(args.query_path).expect("can't open qury file"));

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

    macro_rules! query_idx {
        ($t:path) => {{
            let idx = <$t>::load_index(&args.index_path);
            println!("Index contains {} docs, {} terms", idx.n_docs, idx.n_terms);

            println!("preparing for query");
            match args.query_kind {
                QueryKind::BooleanAnd => perform_query(&idx, parsed, And),
                QueryKind::BooleanOr => perform_query(&idx, parsed, Or),
                QueryKind::RankedAnd => {
                    let p_data = PostingMetadata::<pef::queries::bm25::BM25>::load_file(
                        &idx,
                        &args.sizes_path.expect("size path not given"),
                    );
                    let r_and = RankedAnd::new(p_data, args.k.expect("k not specified"));
                    perform_query(&idx, parsed, r_and);
                }
                QueryKind::Wand => {
                    let p_data = PostingMetadata::<pef::queries::bm25::BM25>::load_file(
                        &idx,
                        &args.sizes_path.expect("size path not given"),
                    );
                    let wand = WAND::new(p_data, args.k.expect("k not specified"));
                    perform_query(&idx, parsed, wand);
                }
            }
        }};
    }

    match args.idx_kind {
        IdxKind::EFSingle => query_idx!(EFIdx),
        IdxKind::UPEf => query_idx!(UPEFIdx),
        IdxKind::UPIs => query_idx!(UPISIdx),
        IdxKind::Opt => query_idx!(OptEFIdx),
    }
}
