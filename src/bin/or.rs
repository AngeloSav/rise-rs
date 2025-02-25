#![allow(internal_features)]
#![feature(core_intrinsics)]
#![feature(array_windows)]

use std::{
    fs,
    io::{BufRead, BufReader},
    time::Duration,
};

use clap::Parser;
use pef::{
    elias_fano::{
        indexed_seq::IndexSequence, opt_partition::OptPartitionedSequence,
        uniform_partitioned_seq::UniformPartitionedSequence,
    },
    indexes::freq_index::{FreqIndex, PostingList},
    space_usage::SpaceUsage,
    utils::TimingQueries,
    EliasFano, IdxKind, NextGEQ, SequenceEnumerator,
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

    /// Rebuilds the index
    #[arg(short, long, default_value_t = false)]
    force_rebuild: bool,
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

    const EST_LEN: bool = false;

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


            let mut res_vec = vec![0; idx.n_docs];
            for _ in 0..n_runs{
                check = 0;
                let mut error = 0;
                timer.start();
                for term in &parsed {
                    //test or
                    let x = boolean_or_multiterm(&idx, term, &mut res_vec);
                    check += x;
                    if EST_LEN {
                        let est_len = estimate_res_len::<_, 1, 24622347>(&idx, term);
                        error += est_len;
                    }
                }
                timer.stop();
                if EST_LEN {
                    println!("avg estimated: {} | check {} | error {}", error, check, (error as isize - check as isize).abs() as f64 / check as f64);
                }
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
            query_idx!(FreqIndex<UniformPartitionedSequence<IndexSequence>>)
        }
        IdxKind::Opt => {
            query_idx!(FreqIndex<OptPartitionedSequence<IndexSequence>>)
        }
    }
}

fn boolean_or_multiterm<'a, T>(idx: &'a FreqIndex<T>, terms: &[usize], v: &mut [u64]) -> usize
where
    T: PostingList<'a>,
{
    //contains pairs (cur_val, iterator)
    let mut enums = Vec::with_capacity(terms.len());
    for &term in terms {
        let mut it = idx.get_plist_iter(term);
        enums.push((it.next().unwrap_or(idx.n_docs as u64), it));
    }

    let mut cur_doc = enums.iter().map(|x| x.0).min().unwrap();
    let mut size = 0;

    while cur_doc < idx.n_docs as u64 {
        // println!("new round ---------------------");
        // println!("pushing {:?}", cur_doc);
        unsafe { *v.get_unchecked_mut(size) = cur_doc };
        size += 1;

        let mut next_doc = idx.n_docs as u64;

        for (cur_term_docid, it) in enums.iter_mut() {
            // println!("new term ---");
            // println!("cur_docid = {:?}", cur_term_docid);
            if core::intrinsics::likely(*cur_term_docid == cur_doc) {
                // println!("update cur!");
                *cur_term_docid = it.next().unwrap_or(idx.n_docs as u64);
            }

            // println!("check less ---");
            // println!("cur_doc = {:?}", cur_doc);
            // println!("cur_term_docid = {:?}", cur_term_docid);
            if core::intrinsics::likely(*cur_term_docid < next_doc) {
                next_doc = *cur_term_docid
            }
        }
        cur_doc = next_doc;
        // println!("nextdoc is {:?}", cur_doc);
    }
    size
}

fn do_or_rounds<T: SequenceEnumerator>(
    enums: &mut [(Option<u64>, T)],
    limit: u64,
    n_rounds: usize,
) -> (usize, u64) {
    let mut v = Vec::new();
    let mut cur_doc = enums.iter().filter_map(|(x, _)| x.map(|x1| x1)).min();
    let mut size = 0;

    for _ in 0..n_rounds {
        if cur_doc.is_none() || unsafe { cur_doc.unwrap_unchecked() } >= limit {
            //got to the end of our slot
            break;
        }

        v.push(unsafe { cur_doc.unwrap_unchecked() });
        size += 1;

        let mut next_doc = None;

        for (cur_term_docid, it) in enums.iter_mut() {
            if core::intrinsics::likely(*cur_term_docid == cur_doc) {
                *cur_term_docid = it.next();
            }

            if core::intrinsics::likely(
                cur_term_docid.is_some() && (next_doc.is_none() || *cur_term_docid < next_doc),
            ) {
                next_doc = *cur_term_docid
            }
        }
        cur_doc = next_doc;
    }
    //maybe size+1
    (size + 1, *v.last().unwrap_or(&limit).min(&limit))
}

fn estimate_res_len<'a, T, const N_SPLITS: usize, const N_ROUNDS: usize>(
    idx: &'a FreqIndex<T>,
    terms: &[usize],
) -> usize
where
    T: PostingList<'a, IterType: NextGEQ>,
{
    let mut enums = Vec::with_capacity(terms.len());
    for &term in terms {
        let mut it = idx.get_plist_iter(term);
        enums.push((it.next(), it));
    }

    let longest_seq = enums.iter().map(|(_, x)| x.len()).max().unwrap();

    let mut round_starting_points = (0..longest_seq)
        .step_by(longest_seq / N_SPLITS)
        .collect::<Vec<_>>();

    round_starting_points.push(longest_seq);

    let mut expected_len = 0;

    for &[sp_start, sp_end] in round_starting_points.array_windows::<2>() {
        //advance all iters to starting point
        for (x, it) in enums.iter_mut() {
            *x = it.next_geq(sp_start as u64).map(|(val, _pos)| val);
        }

        // now do or rounds
        let (len, last_res) = do_or_rounds(&mut enums, sp_end as u64, N_ROUNDS);

        //get density of section
        let d = len as f64 / (last_res + 1 - sp_start as u64) as f64;

        // println!("in range [{sp_start}, {sp_end}]");
        // println!("last got: {last_res} | got {len} | density is {d}");

        expected_len += (d * (sp_end - sp_start) as f64).ceil() as usize;
    }

    expected_len
}
