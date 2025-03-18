use crate::{EliasFano, EnumeratorFromBitSlice, WriteBitvector};

use super::positive_sequence::PositiveSequence;

#[test]
fn increasing_sequence() {
    let v = [1, 4, 43, 0, 5, 321];

    type TY = PositiveSequence<EliasFano>;

    let s = TY::write_bitvector(&v, v.len(), 0);
    let it = TY::iter_from_slice_with_data(s.as_bitslice(), v.len(), 0);

    println!("{:?}", it.collect::<Vec<_>>());
}
