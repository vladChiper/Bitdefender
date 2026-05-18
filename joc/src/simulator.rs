use std::collections::HashMap;
use crate::protocol::{GameState};
use crate::utils;

/// Reprezintă o mutare pe care un erou o poate face în mintea botului.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    Wait,
    Move { x: i32, y: i32 },
    Shoot { x: i32, y: i32 },
}

/// O versiune simplificată a unui erou.
#[derive(Debug, Clone)]
pub struct SimHero {
    pub id: i32,
    pub owner_id: i32,
    pub x: i32,
    pub y: i32,
    pub hp: i32,
    pub cooldown: i32,
}

/// O versiune simplificată a unui proiectil în zbor.
#[derive(Debug, Clone)]
pub struct SimProjectile {
    pub owner_id: i32,
    pub origin_x: i32,
    pub origin_y: i32,
    pub aim_x: i32,
    pub aim_y: i32,
    pub ttl: i32,
    pub steps_taken: usize, // Cât de mult a zburat pe linia lui
}

/// "Universul paralel" în care botul face simulări.
#[derive(Debug, Clone)]
pub struct SimState {
    pub my_player_id: i32,
    pub my_heroes: Vec<SimHero>,
    pub enemy_heroes: Vec<SimHero>,
    pub projectiles: Vec<SimProjectile>,
    pub grid: Vec<Vec<i32>>,
    pub map_width: i32,
    pub map_height: i32,
}

/// Generează punctele de pe linia de tragere conform algoritmului Bresenham.
pub fn get_bresenham_line(x0: i32, y0: i32, x1: i32, y1: i32) -> Vec<(i32, i32)> {
    let mut path = Vec::new();
    let mut x = x0;
    let mut y = y0;
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        path.push((x, y));
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
    path
}

impl SimState {
    /// Construiește starea simulată plecând de la datele reale primite de la server.
    pub fn from_state(state: &GameState, grid: &Vec<Vec<i32>>, my_player_id: i32, width: i32, height: i32) -> Self {
        let mut my_heroes = Vec::new();
        let mut enemy_heroes = Vec::new();
        let mut projectiles = Vec::new();

        for h in &state.heroes {
            let sim_h = SimHero {
                id: h.id, owner_id: h.owner_id, x: h.x, y: h.y, hp: h.hp, cooldown: h.cooldown
            };
            if h.owner_id == my_player_id {
                my_heroes.push(sim_h);
            } else {
                enemy_heroes.push(sim_h);
            }
        }

        // Preluăm și proiectilele care se află deja pe hartă
        for p in &state.projectiles {
            projectiles.push(SimProjectile {
                owner_id: p.owner_id,
                origin_x: p.origin_x,
                origin_y: p.origin_y,
                aim_x: p.x, // O aproximație rapidă a direcției
                aim_y: p.y,
                ttl: p.ttl,
                steps_taken: 0, // Serverul a actualizat deja poziția, deci o luăm de la 0 de aici
            });
        }

        SimState {
            my_player_id,
            my_heroes,
            enemy_heroes,
            projectiles,
            grid: grid.clone(),
            map_width: width,
            map_height: height,
        }
    }

    /// Ce acțiuni LOGICE are la dispoziție acest erou acum?
    pub fn get_possible_actions(&self, hero: &SimHero) -> Vec<Action> {
        let mut actions = vec![Action::Wait];
        if hero.hp <= 0 { return actions; }
        
        let directions = [(0, 3), (0, -3), (3, 0), (-3, 0), (3, 3), (3, -3), (-3, 3), (-3, -3)];
        for (dx, dy) in directions.iter() {
            let nx = hero.x + dx;
            let ny = hero.y + dy;
            if utils::can_stand_at(&self.grid, nx, ny) {
                actions.push(Action::Move { x: nx, y: ny });
            }
        }

        if hero.cooldown == 0 {
            let targets = if hero.owner_id == self.my_player_id { &self.enemy_heroes } else { &self.my_heroes };
            for enemy in targets {
                if enemy.hp > 0 && utils::has_line_of_sight(&self.grid, hero.x, hero.y, enemy.x, enemy.y) {
                    actions.push(Action::Shoot { x: enemy.x, y: enemy.y });
                }
            }
        }
        actions
    }

    /// INIMA SIMULATORULUI: Aici avansăm timpul cu o rundă exact cum face serverul!
    pub fn apply_turn(&mut self, my_actions: &HashMap<i32, Action>, enemy_actions: &HashMap<i32, Action>) {
        let my_id = self.my_player_id;

        // --- 1. FAZA DE MIȘCARE ---
        for hero in self.my_heroes.iter_mut().chain(self.enemy_heroes.iter_mut()) {
            if hero.hp <= 0 { continue; }

            let action = if hero.owner_id == my_id {
                my_actions.get(&hero.id).unwrap_or(&Action::Wait)
            } else {
                enemy_actions.get(&hero.id).unwrap_or(&Action::Wait)
            };

            if let Action::Move { x: tx, y: ty } = action {
                let cx = hero.x;
                let cy = hero.y;
                
                // Dacă click-ul e în interiorul eroului (3x3), e no-op
                if (*tx - cx).abs() <= 1 && (*ty - cy).abs() <= 1 {
                    continue;
                }
                
                // Calculăm pașii de 3 unități
                let sx = (*tx - cx).signum();
                let sy = (*ty - cy).signum();
                let nx = cx + 3 * sx;
                let ny = cy + 3 * sy;

                if nx >= 1 && nx < self.map_width - 1 && ny >= 1 && ny < self.map_height - 1 {
                    if utils::can_stand_at(&self.grid, nx, ny) {
                        hero.x = nx;
                        hero.y = ny;
                    }
                }
            }
        }

        // --- 2. FAZA DE TRAGERE ȘI COOLDOWN ---
        for hero in self.my_heroes.iter_mut().chain(self.enemy_heroes.iter_mut()) {
            if hero.hp <= 0 { continue; }

            let action = if hero.owner_id == my_id {
                my_actions.get(&hero.id).unwrap_or(&Action::Wait)
            } else {
                enemy_actions.get(&hero.id).unwrap_or(&Action::Wait)
            };

            let mut shot_this_turn = false;

            if let Action::Shoot { x: tx, y: ty } = action {
                if hero.cooldown == 0 {
                    self.projectiles.push(SimProjectile {
                        owner_id: hero.owner_id,
                        origin_x: hero.x,
                        origin_y: hero.y,
                        aim_x: *tx,
                        aim_y: *ty,
                        ttl: 4,          // Ttl inițial conform protocolului
                        steps_taken: 0,
                    });
                    hero.cooldown = 4;   // Cooldown resetat
                    shot_this_turn = true;
                }
            }

            // Scădem cooldown-ul dacă nu am tras
            if !shot_this_turn && hero.cooldown > 0 {
                hero.cooldown -= 1;
            }
        }

        // --- 3. REZOLVAREA PROIECTILELOR (Fizica și Coliziunile) ---
        let mut dead_projectiles = std::collections::HashSet::new();
        
        for (p_idx, proj) in self.projectiles.iter_mut().enumerate() {
            let path = get_bresenham_line(proj.origin_x, proj.origin_y, proj.aim_x, proj.aim_y);
            
            let start_step = proj.steps_taken;
            // Proiectilul zboară maxim 6 unități pe tur
            let end_step = std::cmp::min(start_step + 6, path.len().saturating_sub(1));
            
            let mut hit_something = false;

            for step in start_step..=end_step {
                let (px, py) = path[step];
                proj.steps_taken = step; 
                
                // Verificăm dacă a lovit un perete sau a ieșit de pe hartă
                if px < 0 || px >= self.map_width || py < 0 || py >= self.map_height || self.grid[px as usize][py as usize] == 1 {
                    dead_projectiles.insert(p_idx);
                    hit_something = true;
                    break;
                }

                // Verificăm dacă a lovit un inamic (hitbox de 3x3)
                let target_list = if proj.owner_id == my_id { &mut self.enemy_heroes } else { &mut self.my_heroes };
                
                let mut hits = Vec::new();
                for (h_idx, hero) in target_list.iter().enumerate() {
                    if hero.hp > 0 && (hero.x - px).abs() <= 1 && (hero.y - py).abs() <= 1 {
                        hits.push((hero.id, h_idx)); // Reținem și ID-ul pentru tie-breaker
                    }
                }
                
                if !hits.is_empty() {
                    // Dacă a atins 2 eroi simultan, se lovește cel cu ID-ul cel mai mic
                    hits.sort_by_key(|h| h.0);
                    let target_idx = hits[0].1;
                    target_list[target_idx].hp -= 1000;
                    dead_projectiles.insert(p_idx);
                    hit_something = true;
                    break;
                }
                
                // Dacă a ajuns la destinația finală (ținta de pe pământ) și nu a lovit pe nimeni
                if step == path.len() - 1 {
                    dead_projectiles.insert(p_idx);
                    hit_something = true;
                    break;
                }
            }
            
            // Dacă glonțul e încă în aer (nu s-a lovit de nimic), scade timpul de viață
            if !hit_something {
                proj.ttl -= 1;
                if proj.ttl < 0 {
                    dead_projectiles.insert(p_idx);
                }
            }
        }

        // Curățăm proiectilele distruse din memorie
        let mut new_projs = Vec::new();
        for (p_idx, p) in self.projectiles.drain(..).enumerate() {
            if !dead_projectiles.contains(&p_idx) {
                new_projs.push(p);
            }
        }
        self.projectiles = new_projs;
    }

    /// Dă o NOTĂ acestei stări simulate (Prioritățile noastre)
    pub fn evaluate(&self) -> f32 {
        let mut my_score = 0.0;
        let mut enemy_score = 0.0;

        for h in &self.my_heroes {
            if h.hp > 0 {
                my_score += h.hp as f32;
                my_score += 10000.0; 
            } else {
                my_score -= 10000.0; 
            }
        }

        for e in &self.enemy_heroes {
            if e.hp > 0 {
                enemy_score += e.hp as f32;
                enemy_score += 10000.0; 
            } else {
                enemy_score -= 10000.0; 
            }
        }

        my_score - enemy_score
    }
}