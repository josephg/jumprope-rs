
#[derive(Debug)]
pub enum RopeError {
    PositionOutOfBounds,
}

pub trait Rope {
    fn new() -> Self;

    fn insert_at(&mut self, pos: usize, contents: &str) -> Result<(), RopeError>;
    fn del_at(&mut self, pos: usize, len: usize) -> Result<(), RopeError>;

    // fn slice(&self, pos: usize, len: usize) -> Result<String, RopeError>;

    fn to_string(&self) -> String;
    
    fn len(&self) -> usize; // in bytes
    fn char_len(&self) -> usize; // in unicode values

    // This is really weird.
    fn check(&self);
    fn print(&self);
}