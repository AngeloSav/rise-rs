use std::{hint::black_box, time::Duration};

use pef::{
    elias_fano::EliasFano,
    indexes::freq_index::{FreqIndex, PostingList},
    space_usage::SpaceUsage,
    utils::TimingQueries,
    IncreasingSequenceEnumerator,
};

/// replaces function in this code and takes time
macro_rules! time_function {
    ($f: expr) => {{
        let mut t = TimingQueries::new(1, 1);
        t.start();
        let res = black_box($f);
        t.stop();
        (res, t.get().2)
    }};
}

fn main() {
    let path = "/home/anglo/uni/ds2i/test/test_data/test_collection";
    // let idx: FreqIndex<EliasFano, _> = FreqIndex::from_files(path);
    let idx: FreqIndex<EliasFano, _> =
        FreqIndex::load_or_build_and_save(path, &format!("{}{}", path, ".idx.ef.out"), true);
    println!("Index contains {} docs, {} terms", idx.n_docs, idx.n_terms);
    // idx.check_correctness(path);

    println!("size of idx = {} MiB", idx.space_usage_MiB());

    println!("---------two terms------------");
    let t1 = 0;
    let t2 = 2;

    let (results_and, time_and) = time_function!(boolean_and(&idx, t1, t2));
    let (results_or, time_or) = time_function!(boolean_or(&idx, t1, t2));

    // println!("t1: {:?}", idx.get_plist_iter(t1).collect::<Vec<_>>());
    // println!("t2: {:?}", idx.get_plist_iter(t2).collect::<Vec<_>>());

    // println!("t1 len: {}", idx.get_plist_iter(t1).size());
    // println!("t2 len: {}", idx.get_plist_iter(t2).size());
    println!("AND result {}", results_and.len());
    println!("OR result {}", results_or.len());

    println!("---------multi term------------");
    let terms = vec![0, 1, 2, 5];
    let (results_multi_and, time_multi_and) = time_function!(boolean_and_multiterm(&idx, &terms));
    let (results_multi_or, time_multi_or) = time_function!(boolean_or_multiterm(&idx, &terms));

    // println!(
    //     "terms lens: {:?}",
    //     terms
    //         .iter()
    //         .map(|&i| idx.get_plist_iter(i).size())
    //         .collect::<Vec<_>>()
    // );
    println!("MULTI AND result: {}", results_multi_and.len());
    println!("MULTI OR result: {}", results_multi_or.len());

    println!("------------ times -----------------");
    println!(
        "{} = {:?}",
        stringify!(time_and),
        Duration::from_nanos(time_and as u64)
    );
    println!(
        "{} = {:?}",
        stringify!(time_or),
        Duration::from_nanos(time_or as u64)
    );
    println!(
        "{} = {:?}",
        stringify!(time_multi_and),
        Duration::from_nanos(time_multi_and as u64)
    );
    println!(
        "{} = {:?}",
        stringify!(time_multi_or),
        Duration::from_nanos(time_multi_or as u64)
    );

    //sanity checks (strictly increasing sequences)
    results_and.windows(2).for_each(|s| assert!(s[0] < s[1]));
    results_or.windows(2).for_each(|s| assert!(s[0] < s[1]));
    results_multi_and
        .windows(2)
        .for_each(|s| assert!(s[0] < s[1]));
    results_multi_or
        .windows(2)
        .for_each(|s| assert!(s[0] < s[1]));
}

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

fn boolean_and_multiterm<'a, T, S>(idx: &'a FreqIndex<T, S>, terms: &Vec<usize>) -> Vec<u64>
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

    while plists.iter().all(|x| x.0.is_some()) {
        if plists
            .iter()
            .all(|(x, _)| x.unwrap().0 == plists[0].0.unwrap().0)
        {
            //push common value
            v.push(plists[0].0.unwrap().0);

            //increment all plists
            for (x, it) in plists.iter_mut() {
                *x = it.next_val();
            }
        } else {
            //take max and nextgeq
            let max = plists.iter().map(|(x, _)| x.unwrap().0).max().unwrap();

            //increment all plists
            for (x, it) in plists.iter_mut() {
                *x = it.next_geq(max);
            }
        }
    }
    v
}

fn boolean_or_multiterm<'a, T, S>(idx: &'a FreqIndex<T, S>, terms: &Vec<usize>) -> Vec<u64>
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
