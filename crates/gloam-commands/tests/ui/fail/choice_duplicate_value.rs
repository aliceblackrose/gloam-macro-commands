#![allow(dead_code)]

#[derive(gloam_commands::CommandChoice)]
enum Mode {
    #[choice(name = "First", value = "same")]
    First,
    #[choice(name = "Second", value = "same")]
    Second,
}

fn main() {}
