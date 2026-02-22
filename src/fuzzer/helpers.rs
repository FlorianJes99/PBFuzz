use std::fmt::Display;

pub fn print_block<T: Display>(heading: &str, item: T) {
    let amount_signs = 40;
    let len_heading = heading.chars().count();
    let remaining_signs = amount_signs - len_heading;
    let halved: f32 = remaining_signs as f32 / 2_f32;
    println!("");
    println!(
        "{}{heading}{}",
        "=".repeat(halved.floor() as usize),
        "=".repeat(halved.ceil() as usize),
    );
    println!("{item}");
    println!("{}", "=".repeat(amount_signs));
    println!("")
}
