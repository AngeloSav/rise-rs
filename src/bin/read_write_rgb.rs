use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path of the collection (without ".docs" or similar)
    #[arg(short, long)]
    input_path: String,

    /// Path of the output index
    #[arg(short, long)]
    out_path: String,
}

// example usage:
// ./target/release/read_write_rgb -i /data1/InvertedIndexes/inverted_indexes/gov2/gov2.sorted-text.bin -o ./tmp_test/gov2.test

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    // iterator of lists of docids
    let mut it_docs =
        pef::readers::BinaryCollectionIterator::new(format!("{}.docs", &args.input_path).as_str());

    // iterator of lists of freqs
    let mut it_freqs =
        pef::readers::BinaryCollectionIterator::new(format!("{}.freqs", &args.input_path).as_str());

    let mut it_sizes =
        pef::readers::BinaryCollectionIterator::new(format!("{}.sizes", &args.input_path).as_str());

    // new data to write back
    let n_docs = it_docs.next().unwrap().next().unwrap() as u32;
    let mut docs_new: Vec<Vec<u32>> = Vec::new();
    let mut freqs_new: Vec<Vec<u32>> = Vec::new();
    let mut sizes_new: Vec<u32> = Vec::new();

    // PROCESS DATA BELOW --------------------------------

    for list in it_docs.zip(it_freqs) {
        assert_eq!(list.0.len(), list.1.len());
        docs_new.push(list.0.map(|x| x as u32).collect());
        freqs_new.push(list.1.map(|x| x as u32).collect());
    }

    let sizes_list = it_sizes.next().unwrap();
    assert!(sizes_list.len() == n_docs as usize);
    assert!(it_sizes.next().is_none());

    sizes_new = sizes_list.map(|x| x as u32).collect();

    // writeback the data -----------------------------
    pef::readers::ds2i_reader::write_to_files(
        &args.out_path,
        n_docs,
        &docs_new,
        &freqs_new,
        &sizes_new,
    );

    // also save permutation

    Ok(())
}
