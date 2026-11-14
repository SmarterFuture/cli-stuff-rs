use std::{marker::PhantomData, mem};

pub trait Collector<T> {
    fn new(w: usize) -> Self;
    fn push(&mut self, v: T);
    fn empty(&mut self);
}

pub struct Tiles<I, U, B>
where
    I: Iterator,
    U: Collector<I::Item>,
    B: AsMut<[U]>,
{
    buf: B,
    iter: I,
    w: usize,
    cidx: usize,
    chunk_w: u8,
    chunk_h: u8,
    // well maybe the where clause shouldn't be here but...
    _marker: PhantomData<U>,
}

/// if the last line is not complete, the last set of chunks is not guaranteed to be returned
pub trait TilesExt: Iterator + Sized {
    fn tiles<U>(self, w: usize, chunk_w: u8, chunk_h: u8) -> Tiles<Self, U, Vec<U>>
    where
        U: Collector<Self::Item>,
    {
        let cw = chunk_w as usize;
        let ch = chunk_h as usize;
        let n_cells = w.div_ceil(cw);
        Tiles {
            buf: (0..n_cells).map(|_| U::new(cw * ch)).collect(),
            iter: self,
            w,
            cidx: 0,
            chunk_w,
            chunk_h,
            _marker: PhantomData,
        }
    }

    fn tiles_in<U>(
        self,
        w: usize,
        chunk_w: u8,
        chunk_h: u8,
        buf: &mut [U],
    ) -> Tiles<Self, U, &mut [U]>
    where
        U: Collector<Self::Item>,
    {
        let cw = chunk_w as usize;
        let n_cells = w.div_ceil(cw);
        assert!(buf.len() >= n_cells, "Buffer too small");

        // for val in buf.iter_mut().take(max) {
        //     *val = U::new(chunk_w);
        // }

        Tiles {
            buf: &mut buf[..n_cells],
            iter: self,
            w,
            cidx: 0,
            chunk_w,
            chunk_h,
            _marker: PhantomData,
        }
    }
}

/// SAFETY: $idx < $buf.len() must hold
macro_rules! ragged_tail {
    ($iter:expr, $rag:expr, $end:expr, $buf:ident[$idx:expr]) => {{
        debug_assert!($idx < $buf.len());
        let val = unsafe { $buf.get_unchecked_mut($idx) };
        for _ in 0..$rag {
            let v = $iter.next()?;
            val.push(v);
        }
        for _ in $rag..$end {
            val.empty();
        }
    }};
}

impl<I> TilesExt for I where I: Iterator {}

impl<I, U, B> Iterator for Tiles<I, U, B>
where
    I: Iterator,
    U: Collector<I::Item>,
    B: AsMut<[U]>,
{
    type Item = U;

    fn next(&mut self) -> Option<Self::Item> {
        let cw = self.chunk_w as usize;
        let ch = self.chunk_h as usize;
        let buf = self.buf.as_mut();
        let full = self.w / cw;
        let rag = self.w - full * cw;

        // eagerly collect whole block
        if self.cidx == 0 {
            for _ in 0..self.chunk_h - 1 {
                for val in buf[..full].iter_mut() {
                    for _ in 0..self.chunk_w {
                        // we dont care about incomplete blocks
                        // FUTURE: maybe we should pad with empties instead
                        let v = self.iter.next()?;
                        val.push(v);
                    }
                }

                if full != buf.len() {
                    ragged_tail!(self.iter, rag, cw, buf[buf.len() - 1]);
                }
            }
        }

        let take = if rag != 0 && self.cidx == buf.len() - 1 {
            rag
        } else {
            cw
        };

        ragged_tail!(self.iter, take, cw, buf[self.cidx]);

        let mut tmp = U::new(cw * ch);
        // this should be always true
        let cur = unsafe { buf.get_unchecked_mut(self.cidx) };
        mem::swap(&mut tmp, cur);

        self.cidx = if self.cidx + 1 == buf.len() {
            0
        } else {
            self.cidx + 1
        };

        Some(tmp)
    }
}

#[cfg(test)]
mod tests {

    use super::{Collector, Tiles, TilesExt};

    impl<T> Collector<T> for Vec<T> {
        fn new(w: usize) -> Self {
            Vec::with_capacity(w)
        }
        fn push(&mut self, v: T) {
            self.push(v);
        }
        fn empty(&mut self) {}
    }

    impl Collector<i32> for i32 {
        fn new(_w: usize) -> Self {
            0
        }
        fn push(&mut self, v: i32) {
            *self += v;
        }
        fn empty(&mut self) {}
    }

    #[test]
    fn test_chunk_iter_2x2() {
        let iter: Tiles<_, Vec<_>, _> = (0..16).tiles(4, 2, 2);

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
        let iter: Tiles<_, i32, _> = (0..16).tiles(4, 2, 2).tiles(2, 1, 2);

        let chunks: Vec<_> = iter.collect();

        let expected_chunks = vec![52, 68];

        assert_eq!(chunks, expected_chunks);
    }

    #[test]
    #[ignore]
    fn test_chunk_iter_incomplete() {
        let iter: Tiles<_, Vec<_>, _> = (0..9).tiles(3, 2, 2);

        let chunks: Vec<_> = iter.collect();

        let expected_chunks = vec![vec![0, 1, 3, 4], vec![2, 5], vec![6, 7], vec![8]];

        assert_eq!(chunks, expected_chunks);
    }
}
