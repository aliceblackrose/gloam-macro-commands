#![allow(dead_code)]

#[derive(gloam_commands::CommandChoice)]
enum Mode {
    #[choice(name = "Fast", value = "fast")]
    Fast,
    Safe,
}

fn main() {}
