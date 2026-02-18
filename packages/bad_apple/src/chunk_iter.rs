use std::mem;

pub trait TmpCollector<T> {
    fn new(w: usize) -> Self;
    fn push(&mut self, v: T);
}

pub struct ChunkIter<I, U>
where
    I: Iterator,
    U: TmpCollector<I::Item>,
{
    buf: Vec<U>,
    iter: I,
    w: usize,
    chunk_w: usize,
    chunk_h: usize,
    c_idx: usize,
}

/// if the last line is not complete, the last set of chunks is not guaranteed to be returned
pub trait ToChunks: Iterator + Sized {
    fn to_chunks<U>(self, w: usize, chunk_w: usize, chunk_h: usize) -> ChunkIter<Self, U>
    where
        U: TmpCollector<Self::Item>,
    {
        let n_cells = w.div_ceil(chunk_w);
        ChunkIter {
            buf: (0..n_cells).map(|_| U::new(chunk_w)).collect(),
            iter: self,
            w,
            chunk_w,
            chunk_h,
            c_idx: 0,
        }
    }
}

impl<I> ToChunks for I where I: Iterator {}

impl<I, U> Iterator for ChunkIter<I, U>
where
    I: Iterator,
    U: TmpCollector<I::Item>,
{
    type Item = U;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let r_value = self.iter.next();

            let line_x = self.c_idx % self.w;

            let chunk_x = line_x / self.chunk_w;
            let chunk_y = self.c_idx / self.w;

            self.c_idx += 1;

            if let Some(value) = r_value {
                self.buf[chunk_x].push(value);
            } else if line_x == 0 && chunk_y % self.chunk_h == 0 {
                return None;
            }

            let row_done = (chunk_y + 1) % self.chunk_h == 0;
            let col_done = (line_x + 1) % self.chunk_w == 0 || self.c_idx % self.w == 0;

            if row_done && col_done {
                let mut tmp = U::new(self.chunk_w);
                mem::swap(&mut tmp, &mut self.buf[chunk_x]);
                return Some(tmp);
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use super::{ChunkIter, TmpCollector, ToChunks};

    impl<T> TmpCollector<T> for Vec<T> {
        fn new(w: usize) -> Self {
            Vec::with_capacity(w)
        }
        fn push(&mut self, v: T) {
            self.push(v);
        }
    }

    impl TmpCollector<i32> for i32 {
        fn new(_w: usize) -> Self {
            0
        }
        fn push(&mut self, v: i32) {
            *self += v;
        }
    }

    #[test]
    fn test_chunk_iter_2x2() {
        let iter: ChunkIter<_, Vec<_>> = (0..16).to_chunks(4, 2, 2);

        let chunks: Vec<_> = iter.collect();

        let expected_chunks = vec![
            vec![0, 1, 4, 5],
            vec![2, 3, 6, 7],
            vec![8, 9, 12, 13],
            vec![10, 11, 14, 15],
        ];

        assert_eq!(chunks, expected_chunks);
    }

    #[test]
    fn test_chunk_iter_chaining() {
        let iter: ChunkIter<_, i32> = (0..16).to_chunks(4, 2, 2).to_chunks(2, 1, 2);

        let chunks: Vec<_> = iter.collect();

        let expected_chunks = vec![52, 68];

        assert_eq!(chunks, expected_chunks);
    }

    #[test]
    fn test_chunk_iter_incomplete() {
        let iter: ChunkIter<_, Vec<_>> = (0..9).to_chunks(3, 2, 2);

        let chunks: Vec<_> = iter.collect();

        let expected_chunks = vec![vec![0, 1, 3, 4], vec![2, 5], vec![6, 7], vec![8]];

        assert_eq!(chunks, expected_chunks);
    }
}
