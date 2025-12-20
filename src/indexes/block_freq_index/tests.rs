use crate::{
    gen_sequences::{gen_positive_sequence, gen_strictly_increasing_sequence},
    indexes::{
        block_freq_index::{
            block_codices::{
                interpolative_coding::InterpolativeCodec, vbyte_codec::VbyteCodec, BlockCodec,
            },
            block_posting_list::BlockPostingList,
        },
        freq_index::PostingListIter,
    },
};

fn test_codec_monotone<C: BlockCodec>(data: &[u64]) {
    let encoded = C::encode_monotone(data.iter().cloned());
    let mut decoded = vec![0u64; data.len()];

    println!("Encoded size: {} bytes", encoded.len());

    let read_bytes = C::decode_monotone(&encoded, data.len(), &mut decoded);

    assert_eq!(data, &decoded[..]);
    assert_eq!(encoded.len(), read_bytes);
}

fn test_codec<C: BlockCodec>(data: &[u64]) {
    let encoded = C::encode(data.iter().cloned());
    let mut decoded = vec![0u64; data.len()];
    let read_bytes = C::decode(&encoded, data.len(), &mut decoded);

    println!("Encoded size: {} bytes", encoded.len());

    assert_eq!(data, &decoded[..]);
    assert_eq!(encoded.len(), read_bytes);
}

#[test]
fn test_codec_vbyte() {
    let n = 4000;
    let u = 100_000;
    let v = gen_positive_sequence(n, u)
        .iter()
        .map(|&x| x as u64)
        .collect::<Vec<u64>>();

    test_codec::<VbyteCodec>(&v);

    let n = 4000;
    let v = gen_strictly_increasing_sequence(n, u)
        .iter()
        .map(|&x| x as u64)
        .collect::<Vec<u64>>();

    test_codec_monotone::<VbyteCodec>(&v);
}

#[test]
fn test_codec_interpolative() {
    let n = 4000;
    let u = 100_000;
    let v = gen_positive_sequence(n, u)
        .iter()
        .map(|&x| x as u64)
        .collect::<Vec<u64>>();

    test_codec::<InterpolativeCodec>(&v);

    let v = gen_strictly_increasing_sequence(n, u)
        .iter()
        .map(|&x| x as u64)
        .collect::<Vec<u64>>();

    test_codec_monotone::<InterpolativeCodec>(&v);
}

#[test]
fn test_bic_all_zeros() {
    let data = vec![0; 10000];
    test_codec::<InterpolativeCodec>(&data);
}

#[test]
fn test_bic_full_sequence() {
    let data = (0..10000).collect::<Vec<u64>>();
    test_codec_monotone::<InterpolativeCodec>(&data);
}

fn test_block_posting_list_iter<BC>()
where
    BC: BlockCodec,
{
    let n = 4000;
    let u = 100_000;
    let freqs = gen_positive_sequence(n, u)
        .iter()
        .map(|&x| x as u64)
        .collect::<Vec<u64>>();

    let docs = gen_strictly_increasing_sequence(n, u)
        .iter()
        .map(|&x| x as u64)
        .collect::<Vec<u64>>();

    let mut out = Vec::new();
    BlockPostingList::<BC>::write(&docs, &freqs, &mut out);

    println!("Encoded size: {} bytes", out.len());

    let mut it = BlockPostingList::<BC>::iter_from_slice(&out, u as u64);

    let mut cur_doc = it.current_doc();
    while cur_doc < u as u64 {
        let pos = it.current_pos();
        let cur_freq = it.freq();

        assert_eq!(docs[pos], cur_doc);
        assert_eq!(freqs[pos], cur_freq);

        it.next_doc();
        cur_doc = it.current_doc();
    }
}

#[test]
fn test_block_posting_list_iter_vbyte() {
    test_block_posting_list_iter::<VbyteCodec>();
}

#[test]
fn test_block_posting_list_iter_interpolative() {
    test_block_posting_list_iter::<InterpolativeCodec>();
}
