use wasm_bindgen::prelude::*;
use jumprope::JumpRope;

#[wasm_bindgen]
pub struct Rope(JumpRope);

#[wasm_bindgen]
impl Rope {
    /// Create a new rope, optionally with initial content.
    #[wasm_bindgen(constructor)]
    pub fn new(s: Option<String>) -> Self {
        // Can't use Option<&str> in wasm-bindgen for some reason. It doesn't matter much -
        // the passed string will be heap allocated anyway.

        let mut r = if cfg!(feature = "ddos_protection") {
            // Generating a rope from entropy adds 5kb to the binary size.
            JumpRope::new()
        } else {
            JumpRope::new_from_seed(321)
        };
        if let Some(str) = s {
            r.insert(0, &str);
        }
        Self(r)
    }

    #[wasm_bindgen]
    pub fn from(s: String) -> Self {
        Self::new(Some(s))
    }

    /// Insert new content at the specified position.
    #[wasm_bindgen]
    pub fn insert(&mut self, pos: usize, content: &str) {
        self.0.insert(pos, content);
    }

    /// Remove (splice out) rope content of length del_len at the specified position.
    #[wasm_bindgen]
    pub fn remove(&mut self, pos: usize, del_len: usize) {
        self.0.remove(pos..pos+del_len);
    }

    #[wasm_bindgen(js_name=toString)]
    pub fn as_string(&self) -> String {
        self.0.to_string()
    }

    #[wasm_bindgen(getter)]
    pub fn length(&self) -> usize {
        self.0.len_chars()
    }
}

#[cfg(test)]
mod tests {
    use crate::Rope;

    #[test]
    fn smoke_test() {
        let mut r: Rope = Rope::new(None);
        assert_eq!(r.as_string(), "");
        r.insert(0, "hi there");
        assert_eq!(r.as_string(), "hi there");
        r.remove(2, 4);
        assert_eq!(r.as_string(), "hire");
    }
}
