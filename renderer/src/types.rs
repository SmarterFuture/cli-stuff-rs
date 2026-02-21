#[derive(Debug, Clone, Copy)]
pub struct Size {
    pub w: usize,
    pub h: usize,
}

impl Size {
    pub fn new(w: usize, h: usize) -> Size {
        Size { w, h }
    }

    pub fn middle(&self) -> usize {
        self.h / 2 * self.w + self.w / 2
    }

    pub fn flatten(&self) -> usize {
        self.h * self.w
    }
}

#[derive(Eq, PartialEq)]
pub enum Quad {
    Left,
    Center,
    Right,
}
