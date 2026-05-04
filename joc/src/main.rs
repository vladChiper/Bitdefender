use anyhow::Context;
use futures_util::{SinkExt, StreamExt, stream::SplitSink};
use serde::{Deserialize, Serialize};
use std::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};
mod protocol;
use crate::protocol::{Hero, StartMatchArgs, MoveArgs, ShootArgs, StartTurnArgs, ErrorArgs};
use rand::Rng;
use crate::protocol::Command;
use std::collections::VecDeque;

#[derive(Debug, Serialize, Deserialize)]
pub struct WebSocketMessage {
    command: Command,
    args: serde_json::Value,
}

async fn send_command<
    S: SinkExt<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
>(
    write: &mut S,
    msg: WebSocketMessage,
) -> anyhow::Result<()> {
    let msg_deserialized = serde_json::to_string(&msg).context("serialize message")?;
    write
        .send(Message::Text(msg_deserialized.into()))
        .await
        .context("send message")?;
    Ok(())
}

fn bfs(grid: &Vec<Vec<i32>>, start: (i32, i32), goal: (i32, i32)) -> Option<Vec<(i32, i32)>> {
    let rows = grid.len() as i32;
    let cols = if rows > 0 { grid[0].len() as i32 } else { 0 };
    let mut queue = VecDeque::new();
    let mut visited = vec![vec![false; cols as usize]; rows as usize];
    let mut parent = std::collections::HashMap::new();

    queue.push_back(start);
    visited[start.0 as usize][start.1 as usize] = true;

   while let Some(curr) = queue.pop_front() {
        if curr == goal {
            // RECONSTRUCȚIA DRUMULUI:
            let mut path = Vec::new();
            let mut temp = curr;
            while temp != start {
                path.push(temp);
                temp = *parent.get(&temp).unwrap();
            }
            path.push(start);
            path.reverse();
            return Some(path); // Acum returnăm drumul găsit!
        }

        // Neighbors: Up, Down, Left, Right
        let directions = [(0, 1), (0, -1), (1, 0), (-1, 0), (1, 1), (1, -1), (-1, 1), (-1, -1)]; // Include diagonals
        for (dr, dc) in directions {
            let nr = curr.0 + dr;
            let nc = curr.1 + dc;

            if grid[curr.0 as usize][curr.1 as usize] == 1 {
                continue; // Dacă suntem pe un obstacol, nu ne mișcăm
            }

            if nr >= 0 && nr < 90 && nc >= 0 && nc < 51 {
                let next = (nr, nc);
                if grid[next.0 as usize][next.1 as usize] == 1 {
                    continue; // Dacă suntem pe un obstacol, nu ne mișcăm
                }   
                if !visited[next.0 as usize][next.1 as usize] && grid[next.0 as usize][next.1 as usize] == 0 { // 0 = path
                    visited[next.0 as usize][next.1 as usize] = true;
                    parent.insert(next, curr);
                    queue.push_back(next);
                }
            }
        }
    }
    None // No path found
}

// Această funcție returnează coordonatele către care eroul trebuie să facă următorul pas
fn get_next_step_bfs(
    start: (i32, i32), 
    goal: (i32, i32), 
    grid: &Vec<Vec<i32>> // 0 pentru liber, 1 pentru obstacol
) -> (i32, i32) {
    if let Some(path) = bfs(grid, start, goal) {
        // Luăm al doilea element din drum (primul fiind poziția curentă)
        path.get(1).cloned().unwrap_or(start)
    } else {
        // Dacă nu există drum, stăm pe loc sau mergem aleatoriu
        start
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let url = "wss://bitdefenders.cvjd.me/ws";
    let (ws, _) = connect_async(url).await.unwrap();
    let (mut write, mut read) = ws.split();

    println!("connected");

    let mut my_player_id = 0; // O vom seta corect la StartMatch
    let mut last_known_state: Option<protocol::GameState> = None;
    
    // Declarăm gridul lumii
    let mut world_grid: Vec<Vec<i32>> = Vec::new();

    while let Some(msg) = read.next().await {

        let msg = msg.unwrap();
        let text = match msg {
            Message::Text(text) => text,
            Message::Ping(payload) => {
                write.send(Message::Pong(payload)).await.unwrap();
                continue;
            }
            Message::Pong(_) => {
                println!("pong");
                continue;
            }
            Message::Binary(_) => {
                println!("binary message ignored");
                continue;
            }
            Message::Close(frame) => {
                println!("closed: {frame:?}");
                break;
            }
            Message::Frame(_) => continue,
        };
        
        let message: WebSocketMessage = serde_json::from_str(text.as_str()).unwrap();
        println!("{message:?}");
        match message.command {
            Command::Hello => {
                // Send login
                if let Err(e) = send_command(
                    &mut write,
                    WebSocketMessage {
                        command: Command::Login,
                        args: serde_json::json!({"version": 1, "name": "vladChiper"}),
                    },
                )
                .await {
                    println!("Failed to send login command: {e}");
                    break;
                }
            }
            Command::Login => {
                panic!("What are you doing here?");
            },
            Command::Error => {
                println!("Error: {message:?}");
                break;
            }
            Command::Ready => {
                println!("You are ready to play! Trimit comanda de Practice...");
                
                let practice_argv = protocol::PracticeArgs {
                    seed: None,
                };

                let setup_msg = WebSocketMessage {
                    command: Command::Practice,
                    args: serde_json::to_value(practice_argv).unwrap(),
                };

                if let Err(e) = send_command(&mut write, setup_msg).await {
                    println!("Failed to send command: {e}");
                    break;
                }
            },
            
            Command::Challenge => {
                println!("You have been challenged!");
            },
            Command::Practice => {
                println!("You are in practice mode!");
            },
            Command::StartMatch => {
                let args: StartMatchArgs = serde_json::from_value(message.args.clone())
                    .context("Parsing StartMatchArgs")?;

                println!("Meci început! ID: {}", args.match_id);
                
                // Salvăm corect ID-ul nostru de jucător
                my_player_id = args.your_player_id;

                let width = args.config.width;
                let height = args.config.height;
                // Inițializăm gridul lumii
                world_grid = vec![vec![0; height as usize]; width as usize];


                // Populăm gridul cu pereții, ținând cont că au dimensiunea 3x3
                for wall in &args.state.walls {
                    for dx in -1i32..=1i32 {
                        for dy in -1i32..=1i32 {
                            let wx = wall.x + dx;
                            let wy = wall.y + dy;
                            
                            // Ne asigurăm că nu ieșim din marginile hărții
                            if wx >= 0 && wx < width && wy >= 0 && wy < height {
                                world_grid[wx as usize][wy as usize] = 1; // 1 = obstacol

                            }
                        }
                    }
                }

                 
                
                println!("Grid generat cu succes! Dimensiuni: {}x{}", width, height);
            }
            Command::StartTurn => {
                let args: StartTurnArgs = serde_json::from_value(message.args.clone())
                    .context("Parsing StartTurnArgs")?;

                println!("Turnul: {}", args.turn);
                let mut rng = rand::thread_rng();

                // 1. Împărțim eroii vizibili între ai noștri și inamici
                let my_heroes: Vec<&Hero> = args.state.heroes.iter()
                    .filter(|h| h.owner_id == my_player_id)
                    .collect();

                let enemy_heroes: Vec<&Hero> = args.state.heroes.iter()
                    .filter(|h| h.owner_id != my_player_id)
                    .collect();

                // 2. Iterăm prin fiecare erou pe care îl controlăm
                for hero in my_heroes {
                    let mut action_sent = false;

                    // Dacă arma este încărcată (cooldown == 0) și VEDEM un inamic
                    if hero.cooldown == 0 && !enemy_heroes.is_empty() {
                        // Țintim primul inamic vizibil din listă
                        let target = enemy_heroes[0];

                        let shoot_command = WebSocketMessage {
                            command: Command::Shoot,
                            args: serde_json::to_value(ShootArgs {
                                hero_id: hero.id,
                                x: target.x,
                                y: target.y,
                            })?,
                        };

                        send_command(&mut write, shoot_command).await?;
                        println!("💥 Eroul {} trage spre inamicul de la ({}, {})!", hero.id, target.x, target.y);
                        action_sent = true;
                    }

                    if !action_sent { // daca nu am tras

                        let goal_x = 25; 
                        let goal_y = 89;

                        // 2. Rulează BFS pentru a găsi următoarea poziție optimă
                        let start_nod: (i32, i32) = (hero.x / 3, hero.y / 3);
                        let goal_nod: (i32, i32) = (goal_x / 3, goal_y / 3);

                        println!(" Eroul {} este la ({}, {})", hero.id, hero.x/3, hero.y/3);
                        let next_node = get_next_step_bfs(start_nod, goal_nod, world_grid.as_ref());

                        let final_x = next_node.0 * 3 ;
                        let final_y = next_node.1 * 3 ;

                        println!(" Eroul {} vrea sa se miște la  ({}, {})", hero.id, final_x, final_y);

                        let target_x: i32 = hero.x + (final_x - hero.x % 3);
                        let target_y: i32 = hero.y + (final_y - hero.y % 3);

                        let move_command = WebSocketMessage {
                            command: Command::Move,
                            args: serde_json::to_value(MoveArgs {
                                hero_id: hero.id,
                                x: target_x,
                                y: target_y,
                            })?,
                        };



                        send_command(&mut write, move_command).await?;
                        println!("🏃 Eroul {} se mișcă cu intenția spre ({}, {})", hero.id, target_x, target_y);
                    }
                }
            },
            Command::Move => {
                println!("Move command received!");
            },
            Command::Shoot => {
                println!("Shoot command received!");
            }
        }
    }
    Ok(())
}

