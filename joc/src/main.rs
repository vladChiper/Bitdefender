use anyhow::Context;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use std::collections::HashMap;
use std::collections::VecDeque;

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
                println!("Suntem gata! Începem...");

                //Ranked mode
                // let setup_msg = WebSocketMessage {
                //     command: Command::Challenge,
                //     args: serde_json::json!({"seed": null,
                //     "ranked": true
                //     }),
                // };

                //Practice mode

                let setup_msg = WebSocketMessage {
                    command: Command::Practice,
                    args: serde_json::json!({
                        "seed": null
                    }),
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

                // 1. ACTUALIZAREA MEMORIEI
                for enemy in &enemy_heroes {
                    last_known_enemies.insert(enemy.id, (enemy.x, enemy.y));
                }

                // 2. ALEGEREA ȚINTEI PRINCIPALE (FOCUS FIRE)
                // Găsim inamicul vizibil cu cel mai mic HP
                let primary_target = enemy_heroes.iter().min_by_key(|e| e.hp).copied();

                for hero in my_heroes {
                    let mut action_sent = false;

                    // 3. FAZA DE TRAGERE (Focus Fire)
                    if hero.cooldown == 0 && !enemy_heroes.is_empty() {
                        // Încercăm prima dată să tragem în ținta principală
                        if let Some(target) = primary_target {
                            if utils::has_line_of_sight(&world_grid, hero.x, hero.y, target.x, target.y) {
                                let shoot_command = WebSocketMessage {
                                    command: Command::Shoot,
                                    args: serde_json::to_value(ShootArgs {
                                        hero_id: hero.id,
                                        x: target.x,
                                        y: target.y,
                                        comment: Some("POW!!".to_string()), // <-- Mesajul tău aici
                                    })?,
                                };
                                send_command(&mut write, shoot_command).await?;
                                println!("🎯 Eroul {} dă FOCUS FIRE pe inamicul {} la ({}, {})!", hero.id, target.id, target.x, target.y);
                                action_sent = true;
                            }
                        }

                        // Dacă e blocat peretele spre ținta principală, tragem în orice alt inamic vizibil
                        if !action_sent {
                            for target in &enemy_heroes {
                                if utils::has_line_of_sight(&world_grid, hero.x, hero.y, target.x, target.y) {
                                    let shoot_command = WebSocketMessage {
                                    command: Command::Shoot,
                                    args: serde_json::to_value(ShootArgs {
                                        hero_id: hero.id,
                                        x: target.x,
                                        y: target.y,
                                        comment: Some("Shoot".to_string()), // <-- Mesajul tău aici
                                        })?,
                                    };  
                                    send_command(&mut write, shoot_command).await?;
                                    println!("💥 Eroul {} trage spre inamicul {} la ({}, {})!", hero.id, target.id, target.x, target.y);
                                    action_sent = true;
                                    break; 
                                }
                            }
                        }
                    }

                    // 4. FAZA DE MIȘCARE INTELIGENTĂ
                    if !action_sent {
                        let mut next_pos = (hero.x, hero.y);

                        // VERIFICĂM COOLDOWN-UL PENTRU DODGE
                        if hero.cooldown > 0 {
                            next_pos = utils::get_random_valid_move(&world_grid, hero.x, hero.y);
                            println!("🤸 Eroul {} face dodge tactic spre ({}, {})! (cooldown: {})", 
                                hero.id, next_pos.0, next_pos.1, hero.cooldown);
                        } else {
                            // CĂUTARE / FLANCARE
                            let mut target_x = hero.x;
                            let mut target_y = hero.y;

                            if let Some(target) = primary_target {
                                // Dacă îl vedem acum, mergem direct pe el
                                target_x = target.x;
                                target_y = target.y;
                            } else if !last_known_enemies.is_empty() {
                                // Dacă l-am văzut în trecut, mergem acolo
                                if let Some((_id, &(ex, ey))) = last_known_enemies.iter().next() {
                                    target_x = ex;
                                    target_y = ey;
                                }
                            } else {
                                // Implicit: avansăm spre baza adversă
                                target_y = if my_player_id == 0 { map_height - 2 } else { 1 };
                            }

                            // FLANCARE: Adăugăm un offset pe X pentru ca eroii să nu se suprapună (pierd vision range)
                            let offset = if hero.id % 2 == 0 { -6 } else { 6 };
                            target_x = (target_x + offset).clamp(1, map_width - 2);

                            // Asigurăm alinierea țintei la regula de centre (x % 3 == 1)
                            target_x = target_x - (target_x.rem_euclid(3)) + 1;
                            target_y = target_y - (target_y.rem_euclid(3)) + 1;

                            next_pos = utils::bfs_next_step(&world_grid, (hero.x, hero.y), (target_x, target_y));

                            if next_pos == (hero.x, hero.y) {
                                next_pos = utils::get_random_valid_move(&world_grid, hero.x, hero.y);
                            }
                            
                            println!("🏃 Eroul {} avansează tactic (flancare) spre ({}, {}) -> face pasul la ({}, {})", 
                                hero.id, target_x, target_y, next_pos.0, next_pos.1);
                        }

                        let mut taunt_msg = None;
                        if hero.cooldown > 0 {
                            taunt_msg = Some("FUG!".to_string());
                        } else if !last_known_enemies.is_empty() {
                            taunt_msg = Some("go home".to_string());
                        }

                        let move_command = WebSocketMessage {
                            command: Command::Move,
                            args: serde_json::to_value(MoveArgs {
                                hero_id: hero.id,
                                x: next_pos.0,
                                y: next_pos.1,
                                comment: taunt_msg, // <-- Atașează mesajul dinamic
                            })?,
                        };

                        send_command(&mut write, move_command).await?;
                    }
                }
            }
            Command::Move => (),
            Command::Shoot => (),
            Command::EndMatch => {
                let args: protocol::EndMatchArgs = serde_json::from_value(message.args.clone())
                    .context("Parsing EndMatchArgs")?;

                println!("🏁 Meciul s-a terminat! Motiv: {}", args.reason);
                
                match args.winner {
                    Some(ref name) if name == "vladChiper" => {
                        println!("Victorie!");
                    },
                    Some(ref name) => {
                        println!("Lose. Câștigătorul este: {}", name);
                    },
                    None => {
                        if args.reason == "tie" {
                            println!("Egalitate!");
                        } else {
                            println!("Meciul s-a încheiat fără un câștigător clar.");
                        }
                    }
                }
                
                // Pt finalizarea programului.
                break; 
            },
        }
    }
    Ok(())
}