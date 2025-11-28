use clap::Parser;
use pef::{
    indexes::freq_index::{DocList, FreqIndex, FreqList},
    queries::{
        BMMaxScore, BMWand, BlockPostingMetadata, MaxScore, QueryOperator, RankedOr,
        RankedQueryOperator, Wand,
    },
    utils::init_logger,
    DocScorer, EFIdx, IdxKind, OptEFIdx, QueryKind, UPEFIdx, UPISIdx,
};
use std::io::BufRead;
use std::{fs, io::BufReader};

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
}

#[inline(always)]
fn check_query<'a, Q, T, S, D>(
    idx: &'a FreqIndex<T, S>,
    parsed_queries: &Vec<Vec<usize>>,
    mut query_strategy: Q,
    p_data: &BlockPostingMetadata<D>,
    k: usize,
) where
    Q: QueryOperator + RankedQueryOperator,
    T: DocList<'a>,
    S: FreqList<'a>,
    D: DocScorer,
{
    let mut r_or = RankedOr::new(p_data, k);

    log::info!("Checking correctness of {}", Q::query_name());

    for terms in parsed_queries {
        log::trace!("query: {:?}", terms);

        r_or.query(idx, terms);
        query_strategy.query(idx, terms);

        let (mut topk_or_docids, mut topk_or_frequencies): (Vec<_>, Vec<_>) = r_or
            .topk()
            .into_sorted_vec()
            .iter()
            .map(|x| (x.docid, x.frequency))
            .unzip();
        let (mut topk_query_docids, mut topk_query_frequencies): (Vec<_>, Vec<_>) = query_strategy
            .topk()
            .into_sorted_vec()
            .iter()
            .map(|x| (x.docid, x.frequency))
            .unzip();

        topk_or_docids.sort();
        topk_or_frequencies.sort_by(f32::total_cmp);

        topk_query_docids.sort();
        topk_query_frequencies.sort_by(f32::total_cmp);

        assert_eq!(
            topk_or_docids, topk_query_docids,
            "\nleft score: \t{:?}\nright score: \t{:?}",
            topk_or_frequencies, topk_query_frequencies
        );
    }

    println!("Everything is ok for {}", Q::query_name());
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

    macro_rules! query_idx {
        ($t:path) => {{
            let idx = <$t>::load_index(&args.index_path);
            log::info!("Index contains {} docs, {} terms", idx.n_docs, idx.n_terms);

            let p_data = BlockPostingMetadata::<pef::queries::bm25::BM25>::load_file(
                &args.meta_path.clone().expect("meta path not given"),
            );

            for &qk in &args.query_kind {
                match qk {
                    QueryKind::RankedOr => {
                        //just to check
                        let r_or = RankedOr::new(&p_data, args.k);
                        check_query(&idx, &parsed, r_or, &p_data, args.k);
                    }
                    QueryKind::Wand => {
                        let wand = Wand::new(&p_data, args.k);
                        check_query(&idx, &parsed, wand, &p_data, args.k);
                    }
                    QueryKind::Maxscore => {
                        let maxscore = MaxScore::new(&p_data, args.k);
                        check_query(&idx, &parsed, maxscore, &p_data, args.k);
                    }
                    QueryKind::BMWand => {
                        let bmwand = BMWand::new(&p_data, args.k);
                        check_query(&idx, &parsed, bmwand, &p_data, args.k);
                    }
                    QueryKind::BMMaxscore => {
                        let bmmaxscore = BMMaxScore::new(&p_data, args.k);
                        check_query(&idx, &parsed, bmmaxscore, &p_data, args.k);
                    }
                    _ => {
                        println!(
                            "doesn't make sense to compare {} against RankedOr",
                            stringify!(qk)
                        );
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
