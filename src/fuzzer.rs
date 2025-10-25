use std::{
    fs::File,
    io::{self, Write},
};

use crate::grammar::Grammar;

pub struct Fuzzer<T: Grammar> {
    generator: T,
}

impl<T: Grammar> Fuzzer<T> {
    pub fn new(generator: T) -> Self {
        Self {
            generator: generator,
        }
    }

    pub fn write_to_file(&mut self, file_name: &str) -> io::Result<usize> {
        let output = self.generator.produce();
        let mut file = File::create(file_name).expect("Creating a output file should be possible");
        file.write(output.as_bytes())
    }
}
