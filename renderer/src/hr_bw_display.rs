use std::io::{self, Write};

use crate::{
    chunk_iter::{Collector, TilesExt},
    traits::RenderTarget,
    types::Size,
};

const BLOCK_TABLE: [char; 4] = [' ', '▄', '▀', '█'];
const SEXTANT_TABLE: [char; 64] = [
    ' ', '🬞', '🬏', '🬭', '🬇', '🬦', '🬖', '🬵', '🬃', '🬢', '🬓', '🬱', '🬋', '🬩', '🬚', '🬹', '🬁', '🬠', '🬑',
    '🬯', '🬉', '▐', '🬘', '🬷', '🬅', '🬤', '🬔', '🬳', '🬍', '🬫', '🬜', '🬻', '🬀', '🬟', '🬐', '🬮', '🬈', '🬧',
    '🬗', '🬶', '🬄', '🬣', '▌', '🬲', '🬌', '🬪', '🬛', '🬺', '🬂', '🬡', '🬒', '🬰', '🬊', '🬨', '🬙', '🬸', '🬆',
    '🬥', '🬕', '🬴', '🬎', '🬬', '🬝', '█',
];
pub const DOTS_TABLE: [char; 256] = [
    '⠀', '⢀', '⡀', '⣀', '⠠', '⢠', '⡠', '⣠', '⠄', '⢄', '⡄', '⣄', '⠤', '⢤', '⡤', '⣤', '⠐', '⢐', '⡐',
    '⣐', '⠰', '⢰', '⡰', '⣰', '⠔', '⢔', '⡔', '⣔', '⠴', '⢴', '⡴', '⣴', '⠂', '⢂', '⡂', '⣂', '⠢', '⢢',
    '⡢', '⣢', '⠆', '⢆', '⡆', '⣆', '⠦', '⢦', '⡦', '⣦', '⠒', '⢒', '⡒', '⣒', '⠲', '⢲', '⡲', '⣲', '⠖',
    '⢖', '⡖', '⣖', '⠶', '⢶', '⡶', '⣶', '⠈', '⢈', '⡈', '⣈', '⠨', '⢨', '⡨', '⣨', '⠌', '⢌', '⡌', '⣌',
    '⠬', '⢬', '⡬', '⣬', '⠘', '⢘', '⡘', '⣘', '⠸', '⢸', '⡸', '⣸', '⠜', '⢜', '⡜', '⣜', '⠼', '⢼', '⡼',
    '⣼', '⠊', '⢊', '⡊', '⣊', '⠪', '⢪', '⡪', '⣪', '⠎', '⢎', '⡎', '⣎', '⠮', '⢮', '⡮', '⣮', '⠚', '⢚',
    '⡚', '⣚', '⠺', '⢺', '⡺', '⣺', '⠞', '⢞', '⡞', '⣞', '⠾', '⢾', '⡾', '⣾', '⠁', '⢁', '⡁', '⣁', '⠡',
    '⢡', '⡡', '⣡', '⠅', '⢅', '⡅', '⣅', '⠥', '⢥', '⡥', '⣥', '⠑', '⢑', '⡑', '⣑', '⠱', '⢱', '⡱', '⣱',
    '⠕', '⢕', '⡕', '⣕', '⠵', '⢵', '⡵', '⣵', '⠃', '⢃', '⡃', '⣃', '⠣', '⢣', '⡣', '⣣', '⠇', '⢇', '⡇',
    '⣇', '⠧', '⢧', '⡧', '⣧', '⠓', '⢓', '⡓', '⣓', '⠳', '⢳', '⡳', '⣳', '⠗', '⢗', '⡗', '⣗', '⠷', '⢷',
    '⡷', '⣷', '⠉', '⢉', '⡉', '⣉', '⠩', '⢩', '⡩', '⣩', '⠍', '⢍', '⡍', '⣍', '⠭', '⢭', '⡭', '⣭', '⠙',
    '⢙', '⡙', '⣙', '⠹', '⢹', '⡹', '⣹', '⠝', '⢝', '⡝', '⣝', '⠽', '⢽', '⡽', '⣽', '⠋', '⢋', '⡋', '⣋',
    '⠫', '⢫', '⡫', '⣫', '⠏', '⢏', '⡏', '⣏', '⠯', '⢯', '⡯', '⣯', '⠛', '⢛', '⡛', '⣛', '⠻', '⢻', '⡻',
    '⣻', '⠟', '⢟', '⡟', '⣟', '⠿', '⢿', '⡿', '⣿',
];

pub enum Res {
    Low,
    High,
    Extra,
}

impl Res {
    fn to_size(&self) -> Size {
        match self {
            Self::High => Size { w: 2, h: 3 },
            Self::Low => Size { w: 1, h: 2 },
            Self::Extra => Size { w: 2, h: 4 },
        }
    }

    /// this is a fake render, not from Renderable trait
    fn render(&self, v: u8) -> char {
        match self {
            Self::Extra => DOTS_TABLE[v as usize],
            Self::High => SEXTANT_TABLE[v as usize],
            Self::Low => BLOCK_TABLE[v as usize],
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

    fn empty(&mut self) {
        self.push(false)
    }
}

pub struct HighResBWScreen {
    w: usize,
    rw: usize,
    res: Res,
    tmp: Vec<u8>,
}

impl HighResBWScreen {
    pub fn new(w: usize, res: Res) -> Self {
        let max = w.div_ceil(res.to_size().w);
        let v = vec![0u8; max];

        Self {
            w,
            rw: w.div_ceil(res.to_size().w),
            res,
            tmp: v,
        }
    }
}

impl RenderTarget<bool> for HighResBWScreen {
    type Error = io::Error;

    fn init(&self) -> Result<(), Self::Error> {
        print!("\x1B[?1049h");
        print!("\x1B[?25l");
        print!("\x1B[2J\x1B[H");
        io::stdout().flush()
    }

    fn exit(&self) -> Result<(), Self::Error> {
        print!("\x1B[?1049l");
        print!("\x1b[?25h");
        io::stdout().flush()
    }

    fn draw<I>(&mut self, items: I) -> Result<(), Self::Error>
    where
        I: Iterator<Item = bool>,
    {
        let size = self.res.to_size();

        let mut scaled = items
            // not sure if this actually sped things up or no, but here we are
            .tiles_in::<u8>(self.w, size.w as u8, size.h as u8, &mut self.tmp)
            .map(|x| self.res.render(x));

        loop {
            let line = scaled.by_ref().take(self.rw).collect::<String>();

            if line.is_empty() {
                break;
            }

            print!("\n\r{}", line);
        }

        io::stdout().flush()
    }
}
