#[derive(Debug)]
struct Foo<T: ?Sized> {
	blah: i8,
	arr: T,
}

#[cfg(test)]
mod tests {
    use super::Foo;

    #[test]
    fn it_works() {
    	let y: &Foo<[i32]> = &Foo { blah: 10, arr: [1,2,3] };
    	println!("{:?}, {}", y, y.arr.len());
    	assert_eq!(y.blah, 10);
    }
}

#[derive(Debug)]
pub enum RopeError {
	PositionOutOfBounds,
	InvalidCodepoint,
}

pub trait Rope {
	// TODO: These should return Results.
    fn insert(&mut self, pos: usize, contents: &str) -> Result<(), RopeError>;
    fn del(&mut self, pos: usize, len: usize) -> Result<(), RopeError>;

    fn slice(&self, pos: usize, len: usize) -> Result<&str, RopeError>;
	fn to_string(&self) -> String;
    fn num_chars(&self) -> usize;

	fn len(&self) -> usize; // in bytes
	fn char_len(&self) -> usize; // in unicode values
}

#[derive(Debug)]
pub struct JumpRope {

}

impl JumpRope {
	pub fn new() -> Self {
		JumpRope {}
	}
}

//impl Ord for JumpRope
// impl Eq
// impl Clone
// impl FromIterator
// impl Write
// impl FromStr

impl Rope for JumpRope {
	fn insert(&mut self, pos: usize, contents: &str) -> Result<(), RopeError> {
		unimplemented!();
	}
    fn del(&mut self, pos: usize, len: usize) -> Result<(), RopeError> {
		unimplemented!();
	}

    fn slice(&self, pos: usize, len: usize) -> Result<&str, RopeError> {
	   	unimplemented!();
   	}
	fn to_string(&self) -> String {
		unimplemented!();
	}
    fn num_chars(&self) -> usize {
		unimplemented!();
	}
	fn len(&self) -> usize { unimplemented!(); } // in bytes
	fn char_len(&self) -> usize { unimplemented!(); }

}
