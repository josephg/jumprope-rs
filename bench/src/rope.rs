// use std::ops::RangeBounds;

// #[derive(Debug)]
// pub enum RopeError {
//     PositionOutOfBounds,
// }

pub trait Rope {
    const NAME: &'static str;

    fn new() -> Self;

    fn insert_at(&mut self, pos: usize, contents: &str);// -> Result<(), RopeError>;
    fn del_at(&mut self, pos: usize, len: usize);// -> Result<(), RopeError>;
    fn edit_at(&mut self, pos: usize, del_len: usize, ins_content: &str) {
        if del_len > 0 {
            self.del_at(pos, del_len);
        }
        if !ins_content.is_empty() {
            self.insert_at(pos, ins_content);
        }
    }

    // fn del_at<R: RangeBounds<usize>>(&mut self, range: R) -> Result<(), RopeError>;

    // fn slice(&self, pos: usize, len: usize) -> Result<String, RopeError>;

    fn to_string(&self) -> String;
    
    // fn len(&self) -> usize; // in bytes
    fn char_len(&self) -> usize; // in unicode values
}