#![allow(dead_code)]

#[derive(gloam_commands::CommandChoice)]
enum Mode {
    #[choice(name = "Too long")]
    AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA,
}

fn main() {}
