use rand::seq::SliceRandom;

/// Verifică dacă există obstacole (pereți) între două puncte folosind algoritmul Bresenham.
/// Returnează `true` dacă glonțul poate ajunge la țintă (nu lovește pereți).
pub fn has_line_of_sight(grid: &Vec<Vec<i32>>, x0: i32, y0: i32, x1: i32, y1: i32) -> bool {
    let mut x = x0;
    let mut y = y0;
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    let width = grid.len() as i32;
    let height = if width > 0 { grid[0].len() as i32 } else { 0 };

    loop {
        // Verificăm să nu ieșim din hartă accidental în timpul calculului
        if x >= 0 && x < width && y >= 0 && y < height {
            if grid[x as usize][y as usize] == 1 {
                return false; // Glonțul se oprește într-un perete
            }
        }
        
        if x == x1 && y == y1 { break; }
        
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
    true
}

/// Verifică dacă un erou (care ocupă un spațiu de 3x3) poate sta în siguranță cu centrul la (cx, cy)
pub fn can_stand_at(grid: &Vec<Vec<i32>>, cx: i32, cy: i32) -> bool {
    let width = grid.len() as i32;
    let height = if width > 0 { grid[0].len() as i32 } else { 0 };

    // Eroul ocupă 3x3, verificăm toate cele 9 căsuțe
    for dx in -1..=1 {
        for dy in -1..=1 {
            let nx = cx + dx;
            let ny = cy + dy;
            
            if nx < 0 || nx >= width || ny < 0 || ny >= height {
                return false; // Căsuța iese din hartă
            }
            if grid[nx as usize][ny as usize] == 1 {
                return false; // Se suprapune cu un perete
            }
        }
    }
    true
}

/// Alege o mutare aleatorie validă în care eroul se poate deplasa
pub fn get_random_valid_move(grid: &Vec<Vec<i32>>, cx: i32, cy: i32) -> (i32, i32) {
    // Cele 8 direcții posibile, mișcări cu 3 unități conform regulilor (grid aliniat)
    let directions = [
        (0, 3), (0, -3), (3, 0), (-3, 0),
        (3, 3), (3, -3), (-3, 3), (-3, -3)
    ];
    
    let mut valid_moves = Vec::new();
    
    for (dx, dy) in directions.iter() {
        let nx = cx + dx;
        let ny = cy + dy;
        
        // Dacă blocul 3x3 al destinației este liber, e o mutare validă
        if can_stand_at(grid, nx, ny) {
            valid_moves.push((nx, ny));
        }
    }
    
    if valid_moves.is_empty() {
        (cx, cy) // Dacă este complet blocat, rămâne pe loc
    } else {
        let mut rng = rand::thread_rng();
        *valid_moves.choose(&mut rng).unwrap_or(&(cx, cy))
    }
}