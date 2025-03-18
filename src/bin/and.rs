use std::{
    fs,
    io::{BufRead, BufReader},
    time::Duration,
};

use clap::Parser;
use pef::{
    elias_fano::{
        indexed_seq::{IndexSequence, StrictSequence},
        opt_partition::OptPartitionedSequence,
        strict_ef::StrictEliasFano,
        uniform_partitioned_seq::UniformPartitionedSequence,
    },
    indexes::freq_index::{DocList, FreqIndex, FreqList},
    positive_sequences::positive_sequence::PositiveSequence,
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

            // let idx_check = FreqIndex::<EliasFano>::load_or_build_and_save(&input_path, format!("{}.ef.out", input_path).as_str(), false);


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

            let mut res_vec = vec![0; idx.n_docs];

            println!("starting testing!");
            for _ in 0..n_runs{
                check = 0;
                timer.start();
                for term in &parsed {
                    //test and
                    check += boolean_and(&idx, term[0], term[1], &mut res_vec);
                    // boolean_and_check(&idx, &idx_check, term[0], term[1], &mut res_vec);
                    // check += boolean_and_multiterm(&idx, term, &mut res_vec);
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
        }};
    }

    match args.idx_kind {
        IdxKind::EFSingle => query_idx!(FreqIndex<EliasFano, PositiveSequence<StrictEliasFano>>),
        IdxKind::UPEf => {
            query_idx!(
                FreqIndex<
                    UniformPartitionedSequence<EliasFano>,
                    PositiveSequence<UniformPartitionedSequence<StrictEliasFano>>,
                >
            )
        }
        IdxKind::UPIs => {
            query_idx!(
                FreqIndex<
                    UniformPartitionedSequence<IndexSequence>,
                    PositiveSequence<UniformPartitionedSequence<StrictSequence>>,
                >
            )
        }
        IdxKind::Opt => {
            query_idx!(
                FreqIndex<
                    OptPartitionedSequence<IndexSequence>,
                    PositiveSequence<OptPartitionedSequence<StrictSequence>>,
                >
            )
        }
    }
}

#[allow(dead_code)]
#[inline(always)]
fn boolean_and<'a, T, S>(idx: &'a FreqIndex<T, S>, t1: usize, t2: usize, v: &mut Vec<u64>) -> usize
where
    T: DocList<'a>,
    S: FreqList<'a>,
{
    let mut p1 = idx.get_plist_iter(t1);
    let mut p2 = idx.get_plist_iter(t2);

    let max = idx.n_docs as u64;

    let mut posting1 = p1.current_doc().unwrap_or(max);
    let mut posting2 = p2.current_doc().unwrap_or(max);

    let mut size = 0;

    while posting1 < max && posting2 < max {
        if posting1 == posting2 {
            unsafe { *v.get_unchecked_mut(size) = posting1 };
            size += 1;

            //increment both
            p1.next_doc();
            p2.next_doc();

            posting1 = p1.current_doc().unwrap_or(max);
            posting2 = p2.current_doc().unwrap_or(max);
        } else if posting1 < posting2 {
            p1.next_geq(posting2);
            posting1 = p1.current_doc().unwrap_or(max);
        } else {
            p2.next_geq(posting1);
            posting2 = p2.current_doc().unwrap_or(max);
        }
    }
    size
}

// #[allow(dead_code)]
// #[inline(always)]
// // used to check against an index `index_check` that we know is correct
// fn boolean_and_check<'a, T1, T2, S1, S2>(
//     idx: &'a FreqIndex<T1, S1>,
//     idx_check: &'a FreqIndex<T2, S2>,
//     t1: usize,
//     t2: usize,
//     v: &mut Vec<u64>,
// ) where
//     T1: DocList<'a>,
//     T2: DocList<'a>,
//     S1: FreqList<'a>,
//     S2: FreqList<'a>,
// {
//     let mut p1 = idx.get_plist_iter(t1);
//     let mut p2 = idx.get_plist_iter(t2);

//     let mut p1_check = idx_check.get_plist_iter(t1);
//     let mut p2_check = idx_check.get_plist_iter(t2);

//     p1_check.next_val();
//     p2_check.next_val();

//     let mut posting1 = p1.next_val();
//     let mut posting2 = p2.next_val();

//     while posting1.is_some() && posting2.is_some() {
//         if posting1.unwrap().0 == posting2.unwrap().0 {
//             v.push(posting1.unwrap().0);

//             //increment both
//             posting1 = p1.next_val();
//             posting2 = p2.next_val();

//             assert_eq!(posting1, p1_check.next_val());
//             assert_eq!(posting2, p2_check.next_val());
//         } else if posting1.unwrap().0 < posting2.unwrap().0 {
//             posting1 = p1.next_geq(posting2.unwrap().0);
//             assert_eq!(posting1, p1_check.next_geq(posting2.unwrap().0));
//         } else {
//             posting2 = p2.next_geq(posting1.unwrap().0);
//             assert_eq!(posting2, p2_check.next_geq(posting1.unwrap().0));
//         }
//     }
// }

// #[allow(dead_code)]
// #[inline(always)]
// fn boolean_and_multiterm<'a, T, S>(
//     idx: &'a FreqIndex<T, S>,
//     terms: &[usize],
//     v: &mut [u64],
// ) -> usize
// where
//     T: DocList<'a>,
//     S: FreqList<'a>,
// {
//     //contains pairs (cur_val, iterator)
//     let mut enums = Vec::with_capacity(terms.len());
//     for &term in terms {
//         let mut it = idx.get_plist_iter(term);
//         enums.push((it.next().unwrap_or(idx.n_docs as u64), it));
//     }

//     // sort by non-decreasing size
//     enums.sort_by_key(|(_, it)| it.len());

//     let mut candidate = enums[0].0;

//     let mut i = 1;
//     let mut size = 0;

//     while candidate < idx.n_docs as u64 {
//         for (cur_term_docid, it) in enums.iter_mut().skip(i) {
//             *cur_term_docid = it.next_geq(candidate).map_or(idx.n_docs as u64, |x| x.0);
//             if *cur_term_docid != candidate {
//                 candidate = *cur_term_docid;
//                 i = 0;
//                 break;
//             }
//             i += 1;
//         }

//         if i == enums.len() {
//             unsafe { *v.get_unchecked_mut(size) = candidate };
//             size += 1;
//             enums[0].0 = enums[0].1.next().unwrap_or(idx.n_docs as u64);
//             candidate = enums[0].0;
//             i = 1;
//         }
//     }
//     size
// }
