pub trait Renderble {
    type Primitive;
    fn render(&self) -> impl Iterator<Item = Self::Primitive>;
}

pub trait RenderTarget<P> {
    type Error;

    fn init(&self) -> Result<(), Self::Error>;
    fn exit(&self) -> Result<(), Self::Error>;
    fn draw<I>(&mut self, items: I) -> Result<(), Self::Error>
    where
        I: Iterator<Item = P>;
}
