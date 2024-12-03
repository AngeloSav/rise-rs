use std::{fs, time::Duration};

use clap::{Parser, ValueEnum};
use pef::{
    elias_fano::{
        indexed_seq::IndexedSequence, uniform_partitioned_seq::UniformPartitionedSequence,
    },
    indexes::freq_index::{FreqIndex, PostingList},
    space_usage::SpaceUsage,
    utils::TimingQueries,
    EliasFano, IncreasingSequenceEnumerator,
};

#[derive(ValueEnum, Clone, Debug)]
enum IdxKind {
    EFSingle,
    UPEf,
    UPIs,
}

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

    /// Checks the index against the original files
    #[arg(short, long, default_value_t = false)]
    check_correctness: bool,
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
            };
            format!("{}.{}.out", input_path, tail)
        }
    };

    let queries = fs::read_to_string(args.query_path).expect("can't open qury file");

    macro_rules! query_idx {
        ($t:path) => {{
            let idx = <$t>::load_or_build_and_save(&input_path, &out_path, false);
            println!("Index contains {} docs, {} terms", idx._n_docs, idx.n_terms);

            let n_lines = queries.lines().collect::<Vec<_>>().len();
            let n_queries = if let Some(x) = args.n_queries {
                x.min(n_lines)
            } else {
                n_lines
            };
            let mut timer = TimingQueries::new(1, n_queries);

            for l in queries.lines().take(n_queries) {
                let parsed: Vec<_> = l
                    .split_whitespace()
                    .map(|x| x.parse::<usize>().expect("can't parse number"))
                    .collect();
                let (t1, t2) = (parsed[0], parsed[1]);

                //test and
                timer.start();
                boolean_and(&idx, t1, t2);
                timer.stop();
            }

            println!(
                "RESULT [exp=boolean_and, min={:?}, max={:?}, avg={:?}, space_usage_MiB={:.2}]",
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
    }
}

#[inline(always)]
fn boolean_and<'a, T, S>(idx: &'a FreqIndex<T, S>, t1: usize, t2: usize) -> Vec<u64>
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
            posting1 = p1.next_geq(posting2.unwrap().0)
        } else {
            posting2 = p2.next_geq(posting1.unwrap().0)
        }
    }
    v
}
