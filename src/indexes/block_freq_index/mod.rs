pub mod block_codices;
pub mod block_posting_list;

struct BlockFreqIndex<BlockCodec> {
    _phantom: std::marker::PhantomData<BlockCodec>,
}

impl<BlockCodec> BlockFreqIndex<BlockCodec> {
    pub fn new() -> Self {
        BlockFreqIndex {
            _phantom: std::marker::PhantomData,
        }
    }
}

#[cfg(test)]
mod tests;
