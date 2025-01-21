use std::{
    fs,
    io::{BufRead, BufReader},
    time::Duration,
};

use clap::Parser;
use pef::{
    elias_fano::{
        indexed_seq::IndexedSequence, opt_partition::OptPartitionedSequence,
        uniform_partitioned_seq::UniformPartitionedSequence,
    },
    indexes::freq_index::{FreqIndex, PostingList},
    space_usage::SpaceUsage,
    utils::TimingQueries,
    EliasFano, IdxKind,
};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path of the collection (without ".docs" or similar)
    #[arg()]
    input_path: String,

    /// Type of index we want to build
    #[arg()]
    idx_kind: IdxKind,

    /// Path of the file containing the queries
    #[arg()]
    query_path: String,

    /// Process the first n queries
    #[arg(short, long)]
    n_queries: Option<usize>,

    /// Path of the output index (optional)
    #[arg(short, long)]
    out_path: Option<String>,
}

fn main() {
    let args = Args::parse();

    let input_path = args.input_path;

    let out_path = match args.out_path {
        Some(x) => x,
        None => {
            let tail = match args.idx_kind {
                IdxKind::EFSingle => "ef",
                IdxKind::UPEf => "upef",
                IdxKind::UPIs => "upis",
                IdxKind::Opt => "opt",
            };
            format!("{}.{}.out", input_path, tail)
        }
    };

    // let queries = fs::read_to_string(args.query_path).expect("can't open qury file");
    let queries_file =
        BufReader::new(fs::File::open(args.query_path).expect("can't open qury file"));

    macro_rules! query_idx {
        ($t:path) => {{
            let idx = <$t>::load_or_build_and_save(&input_path, &out_path, false);
            println!("Index contains {} docs, {} terms", idx.n_docs, idx.n_terms);

            let queries = if let Some(x) = args.n_queries {
                queries_file.lines().take_while(|a| a.is_ok()).take(x).collect::<Vec<_>>()
            } else {
                queries_file.lines().collect::<Vec<_>>()
            };
            let n_queries = queries.len();

            let n_runs = 4;
            let mut timer = TimingQueries::new(n_runs, queries.len());
            let mut check = 0;

            let parsed: Vec<_> = queries.into_iter().map(|l| {
                l.unwrap()
                .split_whitespace()
                .map(|x| x.parse::<usize>().expect("can't parse number"))
                .collect::<Vec<_>>()
            }).collect();


            let mut res_vec = Vec::with_capacity(idx.n_docs);
            for _ in 0..n_runs{
                check = 0;
                timer.start();
                for term in &parsed {
                    //test or
                    boolean_or_multiterm(&idx, term, &mut res_vec);
                    check += res_vec.len();
                }
                timer.stop();
            }

            println!(
                "RESULT {} [exp=boolean_or, n_queries={}, min={:?}, max={:?}, avg={:?}, space_usage_MiB={:.2}]",
                check,
                n_queries,
                Duration::from_nanos(timer.get().0.try_into().unwrap()),
                Duration::from_nanos(timer.get().1.try_into().unwrap()),
                Duration::from_nanos(timer.get().2.try_into().unwrap()),
                idx.space_usage_MiB()
            );
        }};
    }

    match args.idx_kind {
        IdxKind::EFSingle => query_idx!(FreqIndex<EliasFano>),
        IdxKind::UPEf => {
            query_idx!(FreqIndex<UniformPartitionedSequence<EliasFano>>)
        }
        IdxKind::UPIs => {
            query_idx!(FreqIndex<UniformPartitionedSequence<IndexedSequence>>)
        }
        IdxKind::Opt => {
            query_idx!(FreqIndex<OptPartitionedSequence<IndexedSequence>>)
        }
    }
}

fn boolean_or_multiterm<'a, T>(idx: &'a FreqIndex<T>, terms: &[usize], v: &mut Vec<u64>)
where
    T: PostingList<'a>,
{
    //contains pairs (cur_val, iterator)
    let mut enums = Vec::with_capacity(terms.len());
    for &term in terms {
        let mut it = idx.get_plist_iter(term);
        enums.push((it.next(), it));
    }

    let mut cur_doc = enums.iter().filter_map(|(x, _)| x.map(|x1| x1)).min();
    //we clear the vec
    v.clear();

    while cur_doc.is_some() {
        // println!("new round ---------------------");
        // println!("pushing {:?}", cur_doc);
        v.push(cur_doc.unwrap());
        let mut next_doc = None;
        for (cur_term_docid, it) in enums.iter_mut() {
            // println!("new term ---");
            // println!("cur_docid = {:?}", cur_term_docid);
            if *cur_term_docid == cur_doc {
                // println!("update cur!");
                *cur_term_docid = it.next();
            }

            // println!("check less ---");
            // println!("cur_doc = {:?}", cur_doc);
            // println!("cur_term_docid = {:?}", cur_term_docid);
            if cur_term_docid.is_some() && (next_doc.is_none() || *cur_term_docid < next_doc) {
                next_doc = *cur_term_docid
            }
        }
        cur_doc = next_doc;
        // println!("nextdoc is {:?}", cur_doc);
    }
}
