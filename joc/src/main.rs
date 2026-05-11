use anyhow::Context;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use std::collections::HashMap;

mod protocol;
mod utils; // Modulul nou creat

use crate::protocol::{Command, ErrorArgs, MoveArgs, ShootArgs, StartMatchArgs, StartTurnArgs};

#[derive(Debug, Serialize, Deserialize)]
pub struct WebSocketMessage {
    command: Command,
    args: serde_json::Value,
}

async fn send_command<S>(write: &mut S, msg: WebSocketMessage) -> anyhow::Result<()>
where
    S: SinkExt<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    let msg_deserialized = serde_json::to_string(&msg).context("serialize message")?;
    write
        .send(Message::Text(msg_deserialized.into()))
        .await
        .context("send message")?;
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let url = "wss://bitdefenders.cvjd.me/ws";
    let (ws, _) = connect_async(url).await.unwrap();
    let (mut write, mut read) = ws.split();

    println!("Conectat la server!");

    let mut my_player_id = 0;
    let mut world_grid: Vec<Vec<i32>> = Vec::new();
    let mut map_width = 51;
    let mut map_height = 90;
    
    // Gridul lumii (0 pentru liber, 1 pentru perete)
    let mut world_grid: Vec<Vec<i32>> = Vec::new();

   

    let mut last_known_enemies: HashMap<i32, (i32, i32)> = HashMap::new();

    while let Some(msg) = read.next().await {
        let msg = msg.unwrap();
        let text = match msg {
            Message::Text(text) => text,
            Message::Ping(payload) => {
                write.send(Message::Pong(payload)).await.unwrap();
                continue;
            }
            Message::Pong(_) | Message::Binary(_) | Message::Frame(_) => continue,
            Message::Close(frame) => {
                println!("Conexiune închisă: {frame:?}");
                break;
            }
        };

        let message: WebSocketMessage = serde_json::from_str(text.as_str()).unwrap();
        
        match message.command {
            Command::Hello => {
                send_command(
                    &mut write,
                    WebSocketMessage {
                        command: Command::Login,
                        args: serde_json::json!({"version": 1, "name": "vladChiper"}),
                    },
                ).await.expect("Failed to send login");
            }
            Command::Login => panic!("What are you doing here?"),
            Command::Error => {
                let err: ErrorArgs = serde_json::from_value(message.args).unwrap();
                println!("⚠️ Error {}: {}", err.code, err.message);
                if err.fatal { break; }
            }
            Command::Ready => {
                println!("Suntem gata! Începem Practice...");
                let setup_msg = WebSocketMessage {
                    command: Command::Practice,
                    args: serde_json::json!({"seed": null}),
                };
                send_command(&mut write, setup_msg).await.expect("Failed to start practice");
            }
            Command::Challenge => println!("You have been challenged!"),
            Command::Practice => println!("Modul de antrenament activat!"),
            Command::StartMatch => {
                let args: StartMatchArgs = serde_json::from_value(message.args.clone())
                    .context("Parsing StartMatchArgs")?;

                println!("🏁 Meci început! ID: {}", args.match_id);
                my_player_id = args.your_player_id;

                let width = args.config.width;
                let height = args.config.height;
                
                // Inițializăm harta
                world_grid = vec![vec![0; height as usize]; width as usize];

                // Preluăm pereții de pe hartă și marcăm blocul de 3x3 ca obstacol
                for wall in &args.state.walls {
                    for dx in -1i32..=1i32 {
                        for dy in -1i32..=1i32 {
                            let wx = wall.x + dx;
                            let wy = wall.y + dy;
                            if wx >= 0 && wx < width && wy >= 0 && wy < height {
                                world_grid[wx as usize][wy as usize] = 1;
                            }
                        }
                    }
                }
                println!("🗺️ Harta generată cu succes! Dimensiuni: {}x{}", width, height);

                // Resetăm memoria la început de meci
                last_known_enemies.clear();
            }
            Command::StartTurn => {
                let args: StartTurnArgs = serde_json::from_value(message.args.clone())
                    .context("Parsing StartTurnArgs")?;

                let my_heroes: Vec<&protocol::Hero> = args.state.heroes.iter()
                    .filter(|h| h.owner_id == my_player_id)
                    .collect();

                let enemy_heroes: Vec<&protocol::Hero> = args.state.heroes.iter()
                    .filter(|h| h.owner_id != my_player_id)
                    .collect();

                // ACTUALIZAREA MEMORIEI
                for enemy in &enemy_heroes {
                    last_known_enemies.insert(enemy.id, (enemy.x, enemy.y));
                }

                for hero in my_heroes {
                    let mut action_sent = false;

                    // 1. FAZA DE TRAGERE
                    if hero.cooldown == 0 && !enemy_heroes.is_empty() {
                        for target in &enemy_heroes {
                            if utils::has_line_of_sight(&world_grid, hero.x, hero.y, target.x, target.y) {
                                let shoot_command = WebSocketMessage {
                                    command: Command::Shoot,
                                    args: serde_json::to_value(ShootArgs {
                                        hero_id: hero.id,
                                        x: target.x,
                                        y: target.y,
                                    })?,
                                };
                                send_command(&mut write, shoot_command).await?;
                                println!("💥 Eroul {} trage spre ({}, {})!", hero.id, target.x, target.y);
                                action_sent = true;
                                break; 
                            }
                        }
                    }

                    // 2. FAZA DE MIȘCARE INTELIGENTĂ
                    if !action_sent {
                        let mut target_x = hero.x;
                        let mut target_y = hero.y;

                        if !last_known_enemies.is_empty() {
                            // Dacă avem inamici în memorie, luăm coordonatele primului din map
                           if let Some((_id, &(ex, ey))) = last_known_enemies.iter().next() {
                                target_x = ex;
                                target_y = ey;
                            }
                        } else {
                            // Dacă n-am văzut pe nimeni, mergem implicit spre "baza" adversă
                            // Dacă jucătorul tău e sus (id 0), vrei să mergi jos (harta are Y crescător spre jos)
                            target_y = if my_player_id == 0 { map_height - 2 } else { 1 };
                        }

                        // Asigurăm alinierea țintei la regula de centre (x % 3 == 1)
                        target_x = target_x - (target_x.rem_euclid(3)) + 1;
                        target_y = target_y - (target_y.rem_euclid(3)) + 1;

                        // Folosim BFS pentru a afla următorul pas logic
                        let mut next_pos = utils::bfs_next_step(&world_grid, (hero.x, hero.y), (target_x, target_y));

                        // Fallback: Dacă BFS dă fail (stă pe loc), facem o mișcare random ca să nu blocăm botul
                        if next_pos == (hero.x, hero.y) {
                            next_pos = utils::get_random_valid_move(&world_grid, hero.x, hero.y);
                        }

                        let move_command = WebSocketMessage {
                            command: Command::Move,
                            args: serde_json::to_value(MoveArgs {
                                hero_id: hero.id,
                                x: next_pos.0,
                                y: next_pos.1,
                            })?,
                        };
                        send_command(&mut write, move_command).await?;
                        println!("🏃 Eroul {} se îndreaptă spre ținta ({}, {})", hero.id, target_x, target_y);
                    }
                }
            }
            Command::Move => (),
            Command::Shoot => (),
        }
    }
    Ok(())
}