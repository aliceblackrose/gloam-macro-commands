#![allow(dead_code)]

#[derive(gloam_commands::CommandChoice)]
enum Mixed {
    #[choice(name = "Text", value = "text")]
    Text,
    #[choice(name = "Number", value = 1)]
    Number,
}

fn main() {}
