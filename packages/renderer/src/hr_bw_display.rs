use std::io;

use crate::{
    chunk_iter::{Collector, ToChunks},
    term_display::TermScreen,
    traits::{RenderTarget},
    types::Size,
};

const SEXTANT_TABLE: &str = " 🬀🬁🬂🬃🬄🬅🬆🬇🬈🬉🬊🬋🬌🬍🬎🬏🬐🬑🬒🬓🬔🬕🬖🬗🬘🬙🬚🬛🬜🬝🬞🬟🬠🬡🬢🬣🬤🬥🬦🬧🬨🬩🬪🬫🬬🬭🬮🬯🬰🬱🬲🬳🬴🬵🬶🬷🬸🬹🬺🬻█";
const BLOCK_TABLE: &str = " ▀▄█";

pub enum Res {
    Low,
    High,
}

impl Res {
    fn to_size(&self) -> Size {
        match self {
            Self::High => Size { w: 2, h: 3 },
            Self::Low => Size { w: 1, h: 2 },
        }
    }
    
    /// this is a fake render, not from Renderable trait
    fn render(&self, v: u8) -> char {
        match self {
            Self::High => SEXTANT_TABLE.chars().nth(v as usize).unwrap(),
            Self::Low => BLOCK_TABLE.chars().nth(v as usize).unwrap(),
        }
    }
}

impl Collector<bool> for u8 {
    fn new(_w: usize) -> Self {
        0
    }

    fn push(&mut self, v: bool) {
        *self = (*self << 1) | (v as u8);
    }
}

pub struct HighResBWScreen {
    inner: TermScreen,
    res: Res,
}

impl HighResBWScreen {
    pub fn new(w: usize, h: usize, res: Res) -> Self {
        Self { inner: TermScreen::new(w, h), res }
    }
}

impl RenderTarget<bool> for HighResBWScreen {
    type Error = io::Error;

    fn init(&self) -> Result<(), Self::Error> {
        self.inner.init()
    }

    fn exit(&self) -> Result<(), Self::Error> {
        self.inner.exit()
    }

    fn draw<I>(&mut self, items: I) -> Result<(), Self::Error>
    where
        I: Iterator<Item = bool>,
    {
        let size = self.res.to_size();

        let scaled = items
            .to_chunks::<u8>(self.inner.get_size().w, size.w, size.h)
            .map(|x| self.res.render(x));
        self.inner.draw(scaled)
    }
}
