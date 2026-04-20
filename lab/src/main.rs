use std::io::{self, Read, Seek};
use std::io::copy;
use std::io::stdout;
use std::{env, fs::File};
use serde::{Deserialize};



#[derive(Debug, Deserialize)]
struct GridCell {
    x: u32,
    y: u32,
}

#[derive(Debug, Deserialize)]
struct Labyrinth {
    width: u32,
    height: u32,
    start: GridCell,
    goal: GridCell,
    grid: Vec<GridCell>,
}

fn main()  -> io::Result<()>{
    let mut file = File::open("labyrinth.json").unwrap();


    let data: Labyrinth = serde_json::from_reader(file).unwrap();


    Ok(())
}
