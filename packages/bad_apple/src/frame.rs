use std::ops::Deref;

use renderer::{
    chunk_iter::{Collector, ToChunks},
    traits::RenderTarget,
};

struct Threshold(u16);

impl Deref for Threshold {
    type Target = u16;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Collector<bool> for Threshold {
    fn new(_w: usize) -> Self {
        Self(0)
    }
    fn push(&mut self, v: bool) {
        self.0 += v as u16
    }
}

pub struct FramesIter<I>
where
    I: Iterator<Item = bool>,
{
    w: usize,
    h: usize,
    iter: I,
    scale: usize,
    threshold: u16,
}

pub trait ToFrames: Iterator<Item = bool> + Sized {
    fn to_frames(self, w: usize, h: usize, scale: usize) -> FramesIter<Self> {
        FramesIter {
            w,
            h,
            iter: self,
            scale,
            threshold: (scale.pow(2) / 2) as u16,
        }
    }
}

impl<I> ToFrames for I where I: Iterator<Item = bool> {}

impl<I> Iterator for FramesIter<I>
where
    I: Iterator<Item = bool>,
{
    type Item = Frame;

    fn next(&mut self) -> Option<Self::Item> {
        let r_frame = self.iter.by_ref().take(self.w * self.h);

        let pixels: Vec<bool> = r_frame
            .to_chunks::<Threshold>(self.w, self.scale, self.scale)
            .map(|x| *x > self.threshold)
            .collect();

        if pixels.is_empty() {
            return None;
        }

        Some(Frame(pixels))
    }
}

pub struct Frame(Vec<bool>);

impl Frame {
    pub fn draw_frame_to<R>(self, target: &mut R) -> Result<(), R::Error>
    where
        R: RenderTarget<bool>,
    {
        target.draw(self.0.into_iter())
    }
}
