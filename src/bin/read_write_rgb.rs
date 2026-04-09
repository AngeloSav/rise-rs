use std::fs::File;
use std::io::BufWriter;
use std::io::Write;

use clap::Parser;

use epserde::ser::Serialize;
use rgb::forward::Doc;

use rusty_perm::PermApply as _;
use rusty_perm::PermD;
use rusty_perm::PermFromSorting as _;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path of the collection (without ".docs" or similar)
    #[arg(short, long)]
    input_path: String,

    /// Path of the output index
    #[arg(short, long)]
    out_path: String,

    /// Minimum number of occurrences to consider
    #[arg(short, long, default_value = "4096")]
    min_len: usize,

    /// Maximum length to consider in percentage of documents in the index
    #[arg(short, long, default_value = "0.1")]
    cutoff_frequency: f32,

    /// Min partition size
    #[arg(short, long, default_value = "16")]
    recursion_stop: usize,

    /// Swap iterations
    #[arg(short, long, default_value = "20")]
    swap_iterations: usize,

    /// Depth where we switch from parallel processing to sequential processing
    #[arg(short, long, default_value = "10")]
    parallel_switch: usize,

    /// Sort leaf by identifier
    #[arg(long)]
    sort_leaf: bool,

    /// Maximum depth
    #[arg(long, default_value = "100")]
    max_depth: usize,
}

// example usage:
// ./target/release/read_write_rgb -i /data1/InvertedIndexes/inverted_indexes/gov2/gov2.sorted-text.bin -o ./tmp_test/gov2.rgb

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // iterator of lists of docids
    println!(
        "Reading posting lists from {}",
        format!("{}.docs", &args.input_path).as_str()
    );
    let mut it_docs =
        pef::readers::BinaryCollectionIterator::new(format!("{}.docs", &args.input_path).as_str());

    let it_freqs =
        pef::readers::BinaryCollectionIterator::new(format!("{}.freqs", &args.input_path).as_str());

    // new data to write back
    let n_docs = it_docs.next().unwrap().next().unwrap() as usize;

    // PROCESS DATA BELOW --------------------------------

    // Construct the forward index
    println!("Constructing forward index for {} documents", n_docs);

    let mut it_sizes =
        pef::readers::BinaryCollectionIterator::new(format!("{}.sizes", &args.input_path).as_str())
            .next()
            .unwrap();

    let mut docs = Vec::with_capacity(n_docs);
    let mut avg_doc_len: f64 = 0.0;

    for doc_id in 0..n_docs {
        let doc_size = it_sizes.next().unwrap() as usize;
        avg_doc_len += doc_size as f64;
        docs.push(Doc {
            terms: Vec::with_capacity(256),
            freqs: Vec::with_capacity(256),
            doc_len: doc_size as u32,
            org_id: doc_id as u32,
            gain: 0.0,
            leaf_id: -1,
        });
    }

    let avg_doc_len = avg_doc_len / (n_docs as f64);

    assert!(it_sizes.next().is_none());
    drop(it_sizes);

    println!("Computing RGB partitioning with min_len={}", args.min_len);
    let mut uniq_terms: usize = 0;
    let mut term_id: usize = 0;
    let mut n_terms: usize = 0;

    let it = it_docs.zip(it_freqs);

    for (doc_ids, freqs) in it {
        n_terms += 1;
        if doc_ids.len() < args.min_len {
            continue;
        }

        for (doc_id, freq) in doc_ids.zip(freqs) {
            docs[doc_id as usize].terms.push(term_id as u32);
            docs[doc_id as usize].freqs.push(freq as u32);
        }

        uniq_terms += 1;
        term_id += 1;
    }

    println!(
        "Total terms: {}, unique terms considered: {}",
        n_terms, uniq_terms
    );

    for doc in docs.iter_mut() {
        doc.terms.shrink_to_fit();
        doc.freqs.shrink_to_fit();
    }

    docs.sort_by_key(|a| std::cmp::Reverse(a.terms.len()));
    let num_non_empty = docs.iter().filter(|d| !d.terms.is_empty()).count();

    docs[..num_non_empty].sort_by_key(|a| a.org_id);
    docs[num_non_empty..].sort_by_key(|a| a.org_id);

    println!(
        "Processing {} non empty documents out of {}",
        num_non_empty,
        docs.len()
    );

    // Use iterative processing
    rgb::recursive_graph_bisection_iterative(
        &mut docs[..num_non_empty],
        uniq_terms,
        args.swap_iterations,
        args.recursion_stop,
        args.max_depth,
        args.parallel_switch,
        1,
        args.sort_leaf,
        1,
        avg_doc_len,
    );

    let mut permutation = vec![0u32; docs.len()];
    for (new_id, comp) in docs.iter().enumerate() {
        permutation[comp.org_id as usize] = new_id as u32;
    }

    drop(docs);

    // iterator of lists of docids
    let mut it_docs =
        pef::readers::BinaryCollectionIterator::new(format!("{}.docs", &args.input_path).as_str());

    // iterator of lists of freqs
    println!(
        "Reading frequencies lists from {}",
        format!("{}.freqs", &args.input_path).as_str()
    );
    let it_freqs =
        pef::readers::BinaryCollectionIterator::new(format!("{}.freqs", &args.input_path).as_str());

    println!(
        "Reading sizes lists from {}",
        format!("{}.sizes", &args.input_path).as_str()
    );
    let mut it_sizes =
        pef::readers::BinaryCollectionIterator::new(format!("{}.sizes", &args.input_path).as_str());

    // new data to write back
    println!("Permuting and writing permuted index to {}", &args.out_path);

    let _n_docs = it_docs.next().unwrap().next().unwrap() as usize;

    let mut output_docs_file = BufWriter::new(
        File::create(format!("{}.docs", &args.out_path))
            .expect("could not create docs output file"),
    );
    let mut output_freqs_file = BufWriter::new(
        File::create(format!("{}.freqs", &args.out_path))
            .expect("could not create freqs output file"),
    );
    let mut output_sizes_file = BufWriter::new(
        File::create(format!("{}.sizes", &args.out_path))
            .expect("could not create sizes output file"),
    );

    let push_binary_list_to_file = |writer: &mut BufWriter<File>, list: Vec<u32>| {
        let sz = list.len() as u32;
        writer
            .write_all(&sz.to_le_bytes())
            .expect("could not write docs size");
        for v in list {
            writer
                .write_all(&v.to_le_bytes())
                .expect("could not write docs value");
        }
    };

    // let mut docs_new: Vec<Vec<u32>> = Vec::with_capacity(n_terms as usize);
    // let mut freqs_new: Vec<Vec<u32>> = Vec::with_capacity(n_terms as usize);

    // push n docs to `.docs` file
    push_binary_list_to_file(&mut output_docs_file, vec![n_docs as u32]);

    for list in it_docs.zip(it_freqs) {
        assert_eq!(list.0.len(), list.1.len());
        let mut doc_ids: Vec<u32> = list.0.map(|x| permutation[x as usize] as u32).collect();
        let mut freqs = list.1.map(|x| x as u32).collect::<Vec<u32>>();

        let doc_perm = PermD::from_sort(doc_ids.as_slice());
        doc_perm.apply(&mut doc_ids).unwrap();
        doc_perm.apply(&mut freqs).unwrap();

        push_binary_list_to_file(&mut output_docs_file, doc_ids);
        push_binary_list_to_file(&mut output_freqs_file, freqs);

        // docs_new.push(doc_ids);
        // freqs_new.push(freqs);
    }

    let sizes_list = it_sizes.next().unwrap();
    assert!(sizes_list.len() == n_docs as usize);
    assert!(it_sizes.next().is_none());

    let mut sizes_new = vec![0u32; n_docs as usize];

    for (old_id, size) in sizes_list.enumerate() {
        let new_id = permutation[old_id];
        sizes_new[new_id as usize] = size as u32;
    }

    push_binary_list_to_file(&mut output_sizes_file, sizes_new);
    // writeback the data -----------------------------
    // pef::readers::ds2i_reader::write_to_files(
    //     &args.out_path,
    //     n_docs as u32,
    //     &docs_new,
    //     &freqs_new,
    //     &sizes_new,
    // );

    // Leggi e permuta le query e salvale con la stessa estensione del dataset permutato
    // also save permutation.

    let perm_file = format!("{}.perm", &args.out_path);
    println!("Writing permutation to {}", &perm_file);

    unsafe { permutation.store(perm_file)? };

    Ok(())
}
