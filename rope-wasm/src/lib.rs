use wasm_bindgen::prelude::*;
extern crate wee_alloc;

use jumprope::JumpRope;
// use ropey::Rope as Ropey;

// Use `wee_alloc` as the global allocator. This saves 6kb in binary size.
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
pub struct Rope(JumpRope);

#[wasm_bindgen]
impl Rope {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self(JumpRope::new())
    }

    #[wasm_bindgen]
    pub fn from_str(s: &str) -> Self {
        Self(JumpRope::from_str(s))
    }

    #[wasm_bindgen]
    pub fn insert(&mut self, pos: usize, content: &str) {
        self.0.insert_at(pos, content);
    }

    #[wasm_bindgen]
    pub fn delete(&mut self, pos: usize, del_len: usize) {
        self.0.del_at(pos, del_len);
    }

    #[wasm_bindgen]
    pub fn as_string(&self) -> String {
        self.0.to_string()
    }

    #[wasm_bindgen(getter)]
    pub fn length(&self) -> usize {
        self.0.char_len()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
