use std::collections::HashMap;
use std::time::{Duration, Instant};
use rand::seq::SliceRandom;
use rand::Rng;

use crate::simulator::{Action, SimState};

/// O funcție ajutătoare: Ia starea și ne generează toate perechile posibile de mutări
/// pentru echipa noastră (ex: Erou 0 face X, Erou 1 face Y)
fn get_team_actions(state: &SimState, is_my_team: bool) -> Vec<Vec<(i32, Action)>> {
    let heroes = if is_my_team { &state.my_heroes } else { &state.enemy_heroes };
    if heroes.is_empty() { return vec![vec![]]; } // Dacă au murit toți, nu facem nicio mutare
    
    // Luăm acțiunile posibile pentru primul erou
    let actions_h0 = state.get_possible_actions(&heroes[0]);
    let mut result = Vec::new();
    
    if heroes.len() == 1 {
        // Dacă a rămas doar unul în viață, combinăm doar pentru el
        for a in actions_h0 {
            result.push(vec![(heroes[0].id, a)]);
        }
    } else {
        // Dacă trăiesc amândoi, facem PRODUS CARTEZIAN (toate combinațiile posibile între Erou 0 și Erou 1)
        let actions_h1 = state.get_possible_actions(&heroes[1]);
        for a0 in &actions_h0 {
            for a1 in &actions_h1 {
                result.push(vec![(heroes[0].id, *a0), (heroes[1].id, *a1)]);
            }
        }
    }
    result
}

/// Un Nod din Arborele nostru de decizie (reprezintă un viitor posibil)
pub struct MctsNode {
    pub visits: f32, // De câte ori ne-am imaginat acest viitor
    pub score: f32,  // Ce punctaj total am obținut din el
    pub state: SimState, // Cum arată harta în acest viitor
    pub children: HashMap<Vec<(i32, Action)>, usize>, // Ramificațiile viitorului (cheia e mutarea noastră, valoarea e indexul copilului)
    pub untried_my_actions: Vec<Vec<(i32, Action)>>,  // Mutări pe care NU le-am simulat încă
    pub parent: Option<usize>, // Nodul de unde am venit
}

/// Creierul principal: Arborele MCTS
pub struct Mcts {
    nodes: Vec<MctsNode>,
}

impl Mcts {
    /// Inițializăm creierul cu starea curentă a hărții ca "Rădăcină"
    pub fn new(root_state: SimState) -> Self {
        let untried = get_team_actions(&root_state, true);
        let root = MctsNode {
            visits: 0.0,
            score: 0.0,
            state: root_state,
            children: HashMap::new(),
            untried_my_actions: untried,
            parent: None,
        };
        Mcts { nodes: vec![root] }
    }

    /// FUNCȚIA SUPREMĂ: Gândește timp de X milisecunde și returnează cea mai bună pereche de mutări
    pub fn search(&mut self, duration: Duration) -> Vec<(i32, Action)> {
        let start = Instant::now();
        let mut rng = rand::thread_rng();
        let mut iterations = 0;

        // Cât timp mai avem timp la dispoziție în runda asta (ex: 200ms)
        while start.elapsed() < duration {
            iterations += 1;
            
            // --- PASUL 1: SELECȚIA ---
            // Coborâm în arbore folosind matematica UCB1 până găsim un nod care mai are mutări neexplorate
            let mut node_idx = 0;
            while self.nodes[node_idx].untried_my_actions.is_empty() && !self.nodes[node_idx].children.is_empty() {
                node_idx = self.select_best_child_ucb1(node_idx);
            }

            // --- PASUL 2: EXPANSIUNEA ---
            // Dacă am găsit o mutare neexplorată, o extragem și creăm un univers paralel nou
            if !self.nodes[node_idx].untried_my_actions.is_empty() {
                let action_idx = rng.gen_range(0..self.nodes[node_idx].untried_my_actions.len());
                let my_action = self.nodes[node_idx].untried_my_actions.remove(action_idx);
                
                // Pentru a ști cum va arăta viitorul, trebuie să presupunem că inamicul face și el ceva 
                // (alegem o mutare semi-random logică pentru el)
                let enemy_actions = get_team_actions(&self.nodes[node_idx].state, false);
                let enemy_action = enemy_actions.choose(&mut rng).cloned().unwrap_or_default();
                
                let mut my_map = HashMap::new();
                for (id, a) in &my_action { my_map.insert(*id, *a); }
                let mut enemy_map = HashMap::new();
                for (id, a) in &enemy_action { enemy_map.insert(*id, *a); }

                // Apelăm Arbitrul ca să mute piesele pe hartă
                let mut next_state = self.nodes[node_idx].state.clone();
                next_state.apply_turn(&my_map, &enemy_map);

                let new_untried = get_team_actions(&next_state, true);
                
                let new_node = MctsNode {
                    visits: 0.0,
                    score: 0.0,
                    state: next_state,
                    children: HashMap::new(),
                    untried_my_actions: new_untried,
                    parent: Some(node_idx),
                };
                
                // Salvăm noul viitor în memorie
                let new_idx = self.nodes.len();
                self.nodes.push(new_node);
                self.nodes[node_idx].children.insert(my_action.clone(), new_idx);
                node_idx = new_idx;
            }

            // --- PASUL 3: SIMULAREA (The Rollout) ---
            // Ne imaginăm ce se întâmplă în următoarele 5 runde dacă toată lumea se mișcă la întâmplare
            let mut sim_state = self.nodes[node_idx].state.clone();
            for _ in 0..5 { 
                // Dacă toți ai mei mor, sau toți ai lui mor, oprim simularea
                let my_alive = sim_state.my_heroes.iter().any(|h| h.hp > 0);
                let enemy_alive = sim_state.enemy_heroes.iter().any(|h| h.hp > 0);
                if !my_alive || !enemy_alive { break; }

                let my_acts = get_team_actions(&sim_state, true).choose(&mut rng).cloned().unwrap_or_default();
                let en_acts = get_team_actions(&sim_state, false).choose(&mut rng).cloned().unwrap_or_default();
                
                let mut my_map = HashMap::new();
                for (id, a) in my_acts { my_map.insert(id, a); }
                let mut en_map = HashMap::new();
                for (id, a) in en_acts { en_map.insert(id, a); }
                
                sim_state.apply_turn(&my_map, &en_map);
            }
            
            // Dăm o notă acestui viitor. 
            // O ÎMPĂRȚIM LA 30.000 (Viața maximă posibilă pe hartă) pentru a o normaliza între -1.0 și 1.0!
            let score = sim_state.evaluate() / 30000.0;

            // --- PASUL 4: ÎNVĂȚAREA (Backpropagation) ---
            // Ne ducem din nod în nod înapoi până sus și adăugăm nota primită
            let mut curr = Some(node_idx);
            while let Some(idx) = curr {
                self.nodes[idx].visits += 1.0;
                self.nodes[idx].score += score;
                curr = self.nodes[idx].parent;
            }
        }

        // --- DECIZIA FINALĂ ---
        // Timpul a expirat! Ne uităm la opțiunile noastre inițiale din Rădăcină.
        // O alegem pe cea în care MCTS-ul a avut curaj să coboare de cele mai multe ori (Cele mai multe vizite)
        let root = &self.nodes[0];
        let mut best_action = vec![];
        let mut best_visits = -1.0;

        for (action, &child_idx) in &root.children {
            let child = &self.nodes[child_idx];
            if child.visits > best_visits {
                best_visits = child.visits;
                best_action = action.clone();
            }
        }
        
        println!("🧠 MCTS a rulat {} mutari posibile! Mutarea aleasă a fost explorată de {} ori.", iterations, best_visits);
        best_action
    }

    /// FORMULA MAGICĂ UCB1 (Upper Confidence Bound)
    /// Botul se întreabă mereu: Aprofundez varianta asta care e bună (Exploitation)? 
    /// Sau arunc o privire și pe varianta asta pe care nu o cunosc prea bine (Exploration)?
    fn select_best_child_ucb1(&self, node_idx: usize) -> usize {
        let node = &self.nodes[node_idx];
        let mut best_child = 0;
        let mut best_ucb1 = f32::NEG_INFINITY;

        let exploration_constant = 1.41;

        for &child_idx in node.children.values() {
            let child = &self.nodes[child_idx];
            if child.visits == 0.0 { return child_idx; } // Explorează ce nu e explorat!
            
            // Formula matematică care a stat la baza victoriei AlphaGo
            let exploitation = child.score / child.visits;
            let exploration = exploration_constant * (node.visits.ln() / child.visits).sqrt();
            let ucb1 = exploitation + exploration;

            if ucb1 > best_ucb1 {
                best_ucb1 = ucb1;
                best_child = child_idx;
            }
        }
        best_child
    }
}