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

/// BFS adaptat pentru mișcări de câte 3 unități. Returnează următorul pas optim.
pub fn bfs_next_step(grid: &Vec<Vec<i32>>, start: (i32, i32), goal: (i32, i32)) -> (i32, i32) {
    let width = grid.len() as i32;
    let height = if width > 0 { grid[0].len() as i32 } else { 0 };

    let mut queue = VecDeque::new();
    let mut visited = vec![vec![false; height as usize]; width as usize];
    let mut parent = HashMap::new();

    queue.push_back(start);
    if start.0 >= 0 && start.0 < width && start.1 >= 0 && start.1 < height {
        visited[start.0 as usize][start.1 as usize] = true;
    }

    let mut found = false;
    let mut closest_node = start;
    let mut min_distance = (start.0 - goal.0).abs() + (start.1 - goal.1).abs();

    while let Some(curr) = queue.pop_front() {
        if curr == goal {
            found = true;
            break;
        }

        let directions = [
            (0, 3), (0, -3), (3, 0), (-3, 0),
            (3, 3), (3, -3), (-3, 3), (-3, -3)
        ];

        for (dx, dy) in directions.iter() {
            let nx = curr.0 + dx;
            let ny = curr.1 + dy;

            if nx >= 0 && nx < width && ny >= 0 && ny < height {
                if !visited[nx as usize][ny as usize] && can_stand_at(grid, nx, ny) {
                    visited[nx as usize][ny as usize] = true;
                    parent.insert((nx, ny), curr);
                    queue.push_back((nx, ny));

                    // Ținem minte cel mai apropiat nod în caz că nu ajungem exact la destinație
                    let dist = (nx - goal.0).abs() + (ny - goal.1).abs();
                    if dist < min_distance {
                        min_distance = dist;
                        closest_node = (nx, ny);
                    }
                }
            }
        }
    }

    let target = if found { goal } else { closest_node };

    if target == start {
        return start; // Suntem deja acolo sau complet blocați
    }

    // Reconstruim drumul pentru a afla *primul* pas de făcut
    let mut temp = target;
    let mut first_step = target;
    while temp != start {
        first_step = temp; // Reținem ultimul nod înainte de start
        if let Some(p) = parent.get(&temp) {
            temp = *p;
        } else {
            break;
        }
    }

    first_step
}