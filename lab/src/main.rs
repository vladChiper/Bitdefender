use std::fs::File;
use std::io;
use serde::Deserialize;
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]  
struct Position {
    x: u32,
    y: u32,
}

#[derive(Debug, Deserialize)]
struct GridCell {
    // Redenumim câmpul din JSON ("type") într-un nume valid în Rust
    #[serde(rename = "type")]
    cell_type: String,
    x: u32,
    y: u32,
}

#[derive(Debug, Deserialize)]
struct Labyrinth {
    width: u32,
    height: u32,
    start: Position,  
    goal: Position,   
    grid: Vec<GridCell>,
}

// Funcția care găsește cel mai scurt drum
fn solve_labyrinth(labyrinth: &Labyrinth) -> Option<Vec<Position>> {
    // 1. Punem toți pereții într-un HashSet.
    // HashSet-ul ne permite să verificăm instantaneu dacă o coordonată este zid.
    let mut walls = HashSet::new();
    for cell in &labyrinth.grid {
        if cell.cell_type == "wall" {
            walls.insert(Position { x: cell.x, y: cell.y });
        }
    }

    // 2. Coada pentru algoritmul BFS și o listă cu locurile pe unde am fost deja
    let mut queue = VecDeque::new();
    let mut visited = HashSet::new();
    
    // Aici vom salva de unde a venit fiecare celulă, ca să putem reconstrui drumul la final
    let mut came_from: HashMap<Position, Position> = HashMap::new();

    // Începem de la "start"
    queue.push_back(labyrinth.start);
    visited.insert(labyrinth.start);

    // Direcțiile posibile: Sus, Jos, Stânga, Dreapta
    let directions = [(0, -1), (0, 1), (-1, 0), (1, 0)];

    // 3. Bucla principală a algoritmului
    while let Some(current) = queue.pop_front() {
        // Dacă am ajuns la destinație, reconstruim drumul invers (de la finish la start)
        if current == labyrinth.goal {
            let mut path = Vec::new();
            let mut curr = current;
            
            while curr != labyrinth.start {
                path.push(curr);
                curr = *came_from.get(&curr).unwrap();
            }
            path.push(labyrinth.start);
            path.reverse(); // Întoarcem drumul ca să fie de la start la finish
            
            return Some(path);
        }

        // 4. Verificăm toți cei 4 vecini (Sus, Jos, Stânga, Dreapta)
        for (dx, dy) in &directions {
            let new_x = current.x as i32 + dx;
            let new_y = current.y as i32 + dy;

            // Ne asigurăm că vecinul este în interiorul hărții (nu iese din limite)
            if new_x >= 0 && new_y >= 0 && (new_x as u32) < labyrinth.width && (new_y as u32) < labyrinth.height {
                let neighbor = Position { x: new_x as u32, y: new_y as u32 };

                // Dacă vecinul NU este zid și NU a fost deja vizitat
                if !walls.contains(&neighbor) && !visited.contains(&neighbor) {
                    visited.insert(neighbor);
                    came_from.insert(neighbor, current); // Salvăm faptul că am ajuns aici din 'current'
                    queue.push_back(neighbor);           // Îl adăugăm în coadă pentru a-i explora și lui vecinii mai târziu
                }
            }
        }
    }

    // Dacă am golit coada și nu am găsit "goal"-ul, înseamnă că nu există drum
    None
}


// Funcție pentru afișarea vizuală a labirintului
fn print_maze(labyrinth: &Labyrinth, path: &[Position]) {
    let walls: HashSet<_> = labyrinth.grid.iter().map(|c| Position { x: c.x, y: c.y }).collect();
    let path_set: HashSet<_> = path.iter().copied().collect();

    println!("\nHarta labirintului:\n");
    for y in 0..labyrinth.height {
        for x in 0..labyrinth.width {
            let pos = Position { x, y };
            
            if pos == labyrinth.start {
                print!("🟢"); // Start
            } else if pos == labyrinth.goal {
                print!("🏁"); // Finish
            } else if path_set.contains(&pos) {
                print!("🟡"); // Drumul nostru
            } else if walls.contains(&pos) {
                print!("██"); // Perete
            } else {
                print!("  "); // Spațiu liber
            }
        }
        println!(); // Trecem la linia următoare
    }
    println!();
}

fn main() -> io::Result<()> {
    // Deschidem fișierul
    let file = File::open("labyrinth.json")
        .expect("Nu am putut deschide fișierul labyrinth.json");

    // Deserializăm datele
    let data: Labyrinth = serde_json::from_reader(file)
        .expect("Eroare la parsarea JSON-ului");

    // Câteva mesaje de test ca să ne asigurăm că a citit totul corect
    println!("✅ Labirint încărcat cu succes!");
   
   // Apelăm funcția de rezolvare
    match solve_labyrinth(&data) {
        Some(path) => {
            println!("✅ Drum găsit! Acesta are o lungime de {} pași.", path.len());
            // Afișăm reprezentarea grafică în consolă
            print_maze(&data, &path);
        }
        None => {
            println!("❌ Nu a putut fi găsit niciun drum viabil. Labirintul este blocat.");
        }
    }

    
    Ok(())
}