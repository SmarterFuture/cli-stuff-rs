use std::mem;

pub trait Collector<T> {
    fn new(w: usize) -> Self;
    fn push(&mut self, v: T);
}

pub struct ChunkIter<I, U>
where
    I: Iterator,
    U: Collector<I::Item>,
{
    buf: Vec<U>,
    iter: I,
    w: usize,
    chunk_w: usize,
    chunk_h: usize,
    cx: usize,
    rel_x: usize,
    rel_y: usize,
}

/// if the last line is not complete, the last set of chunks is not guaranteed to be returned
pub trait ToChunks: Iterator + Sized {
    fn to_chunks<U>(self, w: usize, chunk_w: usize, chunk_h: usize) -> ChunkIter<Self, U>
    where
        U: Collector<Self::Item>,
    {
        let n_cells = w.div_ceil(chunk_w);
        ChunkIter {
            buf: (0..n_cells).map(|_| U::new(chunk_w)).collect(),
            iter: self,
            w,
            chunk_w,
            chunk_h,
            cx: 0,
            rel_x: 0,
            rel_y: 0,
        }
    }
}

impl<I> ToChunks for I where I: Iterator {}

impl<I, U> Iterator for ChunkIter<I, U>
where
    I: Iterator,
    U: Collector<I::Item>,
{
    type Item = U;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let r_value = self.iter.next();

            let chunk_x = self.cx / self.chunk_w;

            if let Some(value) = r_value {
                self.buf[chunk_x].push(value);
            } else if self.cx == 0 && self.rel_y == 0 {
                return None;

            } 
            let row_done = self.rel_y + 1 == self.chunk_h;
            let col_done = self.rel_x + 1 == self.chunk_w || self.cx + 1 == self.w;

            self.rel_x += 1;
            if self.rel_x == self.chunk_w {
                self.rel_x = 0
            }

            self.cx += 1;
            if self.cx == self.w {
                self.cx = 0;
                self.rel_x = 0;
                self.rel_y += 1;
                
                if self.rel_y == self.chunk_h {
                    self.rel_y = 0
                }
            } 

            if row_done && col_done {
                let mut tmp = U::new(self.chunk_w * self.chunk_h);
                mem::swap(&mut tmp, &mut self.buf[chunk_x]);
                return Some(tmp);
            }
        }
    }
}


#[cfg(test)]
mod tests {

    use super::{ChunkIter, Collector, ToChunks};

    impl<T> Collector<T> for Vec<T> {
        fn new(w: usize) -> Self {
            Vec::with_capacity(w)
        }
        fn push(&mut self, v: T) {
            self.push(v);
        }
    }

    impl Collector<i32> for i32 {
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
        let iter: ChunkIter<_, Vec<_>> = (0..7).to_chunks(3, 2, 2);

        let chunks: Vec<_> = iter.collect();

        let expected_chunks = vec![vec![0, 1, 3, 4], vec![2, 5], vec![6, 7], vec![8]];

        assert_eq!(chunks, expected_chunks);
    }
}
