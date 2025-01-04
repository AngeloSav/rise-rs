use std::{
    fs,
    io::{BufRead, BufReader},
    time::Duration,
};

use clap::{Parser, ValueEnum};
use pef::{
    elias_fano::{
        indexed_seq::IndexedSequence, opt_partition::OptPartitionedSequence,
        uniform_partitioned_seq::UniformPartitionedSequence,
    },
    indexes::freq_index::{FreqIndex, PostingList},
    space_usage::SpaceUsage,
    utils::TimingQueries,
    EliasFano, IdxKind, IncreasingSequenceEnumerator,
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
            println!("Index contains {} docs, {} terms", idx._n_docs, idx.n_terms);

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


            for _ in 0..n_runs{
                check = 0;
                timer.start();
                for term in &parsed {
                    //test or
                    let x = boolean_or(&idx, term[0], term[1]);
                    check += x.len();
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

    const FIXED_BLOCK_SIZE: usize = 512;

    match args.idx_kind {
        IdxKind::EFSingle => query_idx!(FreqIndex<EliasFano, _>),
        IdxKind::UPEf => {
            query_idx!(FreqIndex<UniformPartitionedSequence<EliasFano, _, FIXED_BLOCK_SIZE>, _>)
        }
        IdxKind::UPIs => {
            query_idx!(
                FreqIndex<UniformPartitionedSequence<IndexedSequence, _, FIXED_BLOCK_SIZE>, _>
            )
        }
        IdxKind::Opt => {
            query_idx!(FreqIndex<OptPartitionedSequence<IndexedSequence, _>, _>)
        }
    }
}

#[allow(dead_code)]
#[inline(always)]
fn boolean_or<'a, T, S>(idx: &'a FreqIndex<T, S>, t1: usize, t2: usize) -> Vec<u64>
where
    T: PostingList<'a, S>,
    S: IncreasingSequenceEnumerator,
{
    let mut p1 = idx.get_plist_iter(t1);
    let mut p2 = idx.get_plist_iter(t2);

    let mut posting1 = p1.next_val();
    let mut posting2 = p2.next_val();

    let mut v = Vec::new();

    while posting1.is_some() && posting2.is_some() {
        if posting1.unwrap().0 == posting2.unwrap().0 {
            v.push(posting1.unwrap().0);
            //increment both
            posting1 = p1.next_val();
            posting2 = p2.next_val();
        } else if posting1.unwrap().0 < posting2.unwrap().0 {
            v.push(posting1.unwrap().0);
            posting1 = p1.next_val()
        } else {
            v.push(posting2.unwrap().0);
            posting2 = p2.next_val()
        }
    }

    //flush last list
    if let Some(posting1) = posting1 {
        v.push(posting1.0);
        while let Some(posting1) = p1.next_val() {
            v.push(posting1.0);
        }
    }
    if let Some(posting2) = posting2 {
        v.push(posting2.0);
        while let Some(posting2) = p2.next_val() {
            v.push(posting2.0);
        }
    }
    v
}

#[allow(dead_code)]
#[inline(always)]
fn boolean_or_multiterm<'a, T, S>(idx: &'a FreqIndex<T, S>, terms: &[usize]) -> Vec<u64>
where
    T: PostingList<'a, S>,
    S: IncreasingSequenceEnumerator,
{
    let mut plists: Vec<_> = terms
        .iter()
        .map(|&i| {
            let mut a = idx.get_plist_iter(i);
            (a.next_val(), a)
        })
        .collect();

    let mut v = Vec::new();

    while !plists.is_empty() {
        // push min to v
        let min = plists.iter().map(|(x, _)| x.unwrap().0).min().unwrap();
        v.push(min);

        // inc all that are min
        plists
            .iter_mut()
            .filter(|(x, _)| x.unwrap().0 == min)
            .for_each(|(x, it)| {
                *x = it.next_val();
            });

        // remove finished lists
        plists = plists
            .into_iter()
            .filter(|(x, _)| x.is_some())
            .collect::<Vec<_>>();
    }
    v
}
