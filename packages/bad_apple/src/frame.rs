use std::fmt::Display;

use crate::chunk_iter::{TmpCollector, ToChunks};

pub enum Pixel {
    Empty,
    Top,
    Bottom,
    Full,
}

impl Pixel {
    pub fn new(top: bool, bot: bool) -> Self {
        match (top, bot) {
            (false, false) => Self::Empty,
            (true, false) => Self::Top,
            (false, true) => Self::Bottom,
            (true, true) => Self::Full,
        }
    }
}

impl Display for Pixel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Empty => " ",
                Self::Top => "▀",
                Self::Bottom => "▄",
                Self::Full => "█",
            }
        )
    }
}

struct PixelCol {
    top: bool,
    botom: bool,
    is_top: bool,
}

impl PixelCol {
    fn to_pixel(&self) -> Pixel {
        Pixel::new(self.top, self.botom)
    }
}

impl TmpCollector<bool> for PixelCol {
    fn new(_w: usize) -> Self {
        PixelCol {
            top: false,
            botom: false,
            is_top: true,
        }
    }

    fn push(&mut self, v: bool) {
        match self.is_top {
            true => self.top = v,
            false => self.botom = v,
        }
        self.is_top = !self.is_top;
    }
}

impl TmpCollector<bool> for u16 {
    fn new(_w: usize) -> Self {
        0
    }
    fn push(&mut self, v: bool) {
        *self += v as u16
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
}

pub trait ToFrames: Iterator<Item = bool> + Sized {
    fn to_frames(self, w: usize, h: usize, scale: usize) -> FramesIter<Self> {
        FramesIter {
            w,
            h,
            iter: self,
            scale,
        }
    }
}

impl<I> ToFrames for I where I: Iterator<Item = bool> {}

impl<I> Iterator for FramesIter<I>
where
    I: Iterator<Item = bool>,
{
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        let r_frame = self.iter.by_ref().take(self.w * self.h);

        let frame_w = self.w.div_ceil(self.scale);
        let threshold = (self.scale.pow(2) / 2) as u16;

        let pixels = r_frame
            .to_chunks::<u16>(self.w, self.scale, self.scale)
            .map(|x| x > threshold)
            .to_chunks::<PixelCol>(frame_w, 1, 2)
            .map(|x| x.to_pixel());

        let mut idx = 0;
        let mut out = String::new();

        for pixel in pixels {
            out.push_str(&format!("{}", pixel));
            idx += 1;
            if idx % frame_w == 0 {
                out.push_str("\n\r");
            }
        }

        if idx == 0 {
            return None;
        }

        Some(out)
    }
}
