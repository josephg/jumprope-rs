extern crate cc;

fn main() {
    cc::Build::new()
        .file("rope.c")
        .compile("librope");
}