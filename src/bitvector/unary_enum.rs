use crate::{BitSliceWithOffset, utils::select_in_word};

#[derive(Debug, Clone, Default)]
pub struct UnaryEnumerator<'a> {
    pub bs: &'a [u64],
    pub offset: usize,
    cur_position: usize,
    pub buf: u64,
}

impl<'a> UnaryEnumerator<'a> {
    #[inline]
    pub fn with_pos(bs: &BitSliceWithOffset<'a>, pos: usize) -> Self {
        assert!(bs.offset < 64);
        let cur_position = pos + bs.offset;
        let mut buf = bs.data[cur_position / 64];
        buf &= !0_u64 << (cur_position % 64);

        Self {
            bs: bs.data,
            offset: bs.offset,
            cur_position,
            buf,
        }
    }

    pub fn position(&self) -> usize {
        // dbg!(self.cur_position, self.offset);
        self.cur_position - self.offset
    }

    #[inline]
    pub fn next_one(&mut self) -> usize {
        let mut pos_in_word;
        let mut buf = self.buf;

        loop {
            pos_in_word = buf.trailing_zeros() as usize;

            if pos_in_word < 64 {
                break;
            }

            self.cur_position += 64;
            buf = self.bs[self.cur_position / 64];
        }

        self.buf = buf & (buf - 1);
        self.cur_position = (self.cur_position & !63) + pos_in_word;
        self.position()
    }

    pub fn skip1(&mut self, k: usize) -> usize {
        let mut skipped = 0;
        let mut buf = self.buf;
        let mut w;

        loop {
            w = buf.count_ones() as usize;

            if skipped + w > k {
                break;
            }

            skipped += w;
            self.cur_position += 64;
            buf = self.bs[self.cur_position / 64];
        }

        assert!(buf != 0);
        let pos_in_word = select_in_word(buf, (k - skipped) as u64) as usize;
        self.buf = buf & (!0_u64 << pos_in_word);
        self.cur_position = (self.cur_position & !63) + pos_in_word;
        self.position()
    }

    pub fn skip0(&mut self, k: usize) -> usize {
        let mut skipped = 0;
        let pos_in_word = self.cur_position % 64;
        let mut buf = !self.buf & (!0_u64 << pos_in_word);
        let mut w;

        loop {
            w = buf.count_ones() as usize;

            if skipped + w > k {
                break;
            }

            skipped += w;
            self.cur_position += 64;
            buf = !self.bs[self.cur_position / 64];
        }

        assert!(buf != 0);
        let pos_in_word = select_in_word(buf, (k - skipped) as u64) as usize;
        self.buf = !buf & (!0_u64 << pos_in_word);
        self.cur_position = (self.cur_position & !63) + pos_in_word;
        self.position()
    }
}
