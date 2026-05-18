use anyhow::Context;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use std::collections::HashMap;

mod protocol;
mod utils;

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
    let mut map_width = 51;
    let mut map_height = 90;
    
    let mut world_grid: Vec<Vec<i32>> = Vec::new();

    // Memoria comună a echipei
    let mut last_known_enemies: HashMap<i32, (i32, i32)> = HashMap::new();
    let mut locked_target_id: Option<i32> = None;
    let mut initial_grouping_done = false;

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

                map_width = args.config.width;
                map_height = args.config.height;
                
                world_grid = vec![vec![0; map_height as usize]; map_width as usize];

                for wall in &args.state.walls {
                    for dx in -1i32..=1i32 {
                        for dy in -1i32..=1i32 {
                            let wx = wall.x + dx;
                            let wy = wall.y + dy;
                            if wx >= 0 && wx < map_width && wy >= 0 && wy < map_height {
                                world_grid[wx as usize][wy as usize] = 1;
                            }
                        }
                    }
                }
                
                last_known_enemies.clear();
                locked_target_id = None;
                initial_grouping_done = false; 
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

                // 2. VERIFICĂM CONTOPIREA EROILOR (la startul turnului)
                if !initial_grouping_done {
                    if my_heroes.len() >= 2 {
                        let h0 = my_heroes[0];
                        let h1 = my_heroes[1];
                        if h0.x == h1.x && h0.y == h1.y {
                            println!("🤝 Eroii s-au contopit perfect! Pornim tancul spre inamici.");
                            initial_grouping_done = true;
                        }
                    } else {
                        initial_grouping_done = true; 
                    }
                }

                // 3. ACTUALIZAREA ȚINTEI COMUNE (LOCK-ON)
                let visible_target = enemy_heroes.iter().min_by_key(|e| e.hp).copied();
                if let Some(target) = visible_target {
                    locked_target_id = Some(target.id);
                } else {
                    if let Some(id) = locked_target_id {
                        if let Some(&(ex, ey)) = last_known_enemies.get(&id) {
                            let reached = my_heroes.iter().any(|h| (h.x - ex).abs() <= 3 && (h.y - ey).abs() <= 3);
                            if reached {
                                locked_target_id = None;
                                last_known_enemies.remove(&id);
                            }
                        }
                    }
                }

                // 4. STABILIREA LIDERULUI ȘI A MUTĂRII LUI VIITOARE
                let leader_id = my_heroes.iter().map(|h| h.id).min().unwrap_or(-1);
                let leader_pos = my_heroes.iter().find(|h| h.id == leader_id).map(|h| (h.x, h.y)).unwrap_or((0,0));
                
                let mut leader_future_pos = leader_pos; 

                let mut squad_target_x = 25; 
                let mut squad_target_y = if my_player_id == 0 { map_height - 2 } else { 1 };

                if !initial_grouping_done {
                    squad_target_x = leader_pos.0;
                    squad_target_y = if my_player_id == 0 { 10 } else { map_height - 11 };
                } else if let Some(id) = locked_target_id {
                    if let Some(&(ex, ey)) = last_known_enemies.get(&id) {
                        squad_target_x = ex;
                        squad_target_y = ey;
                    }
                }

                // 5. SORTĂM EROII PENTRU A PROCESA LIDERUL PRIMUL
                let mut sorted_heroes = my_heroes.clone();
                sorted_heroes.sort_by_key(|h| h.id);

                for hero in sorted_heroes {
                    let mut action_sent = false;

                    // 5.1 FAZA DE TRAGERE
                    if hero.cooldown == 0 && !enemy_heroes.is_empty() {
                        let target_to_shoot = if let Some(id) = locked_target_id {
                            enemy_heroes.iter().find(|e| e.id == id).copied()
                        } else { None };

                        if let Some(target) = target_to_shoot {
                            if utils::has_line_of_sight(&world_grid, hero.x, hero.y, target.x, target.y) {
                                let shoot_command = WebSocketMessage {
                                    command: Command::Shoot,
                                    args: serde_json::to_value(ShootArgs {
                                        hero_id: hero.id,
                                        x: target.x,
                                        y: target.y,
                                        comment: Some("Focus Fire! 🎯".to_string()),
                                    })?,
                                };
                                send_command(&mut write, shoot_command).await?;
                                action_sent = true;
                                
                                if hero.id == leader_id {
                                    leader_future_pos = (hero.x, hero.y);
                                }
                            }
                        }

                        if !action_sent {
                            for target in &enemy_heroes {
                                if utils::has_line_of_sight(&world_grid, hero.x, hero.y, target.x, target.y) {
                                    let shoot_command = WebSocketMessage {
                                    command: Command::Shoot,
                                    args: serde_json::to_value(ShootArgs {
                                        hero_id: hero.id,
                                        x: target.x,
                                        y: target.y,
                                        comment: Some("Pow! 🔥".to_string()),
                                        })?,
                                    };  
                                    send_command(&mut write, shoot_command).await?;
                                    action_sent = true;
                                    
                                    if hero.id == leader_id {
                                        leader_future_pos = (hero.x, hero.y);
                                    }
                                    break; 
                                }
                            }
                        }
                    }

                    // 5.2 FAZA DE MIȘCARE
                    if !action_sent {
                        let mut next_pos = (hero.x, hero.y);

                        if hero.id == leader_id {
                            // DECIZIILE LIDERULUI
                            if hero.cooldown > 0 {
                                next_pos = utils::get_random_valid_move(&world_grid, hero.x, hero.y);
                            } else {
                                let mut target_x = squad_target_x;
                                let mut target_y = squad_target_y;
                                target_x = target_x.clamp(1, map_width - 2);
                                target_x = target_x - (target_x.rem_euclid(3)) + 1;
                                target_y = target_y - (target_y.rem_euclid(3)) + 1;

                                next_pos = utils::bfs_next_step(&world_grid, (hero.x, hero.y), (target_x, target_y));

                                if next_pos == (hero.x, hero.y) {
                                    // Dacă s-au grupat deja global, facem mișcări random când ajungem la destinația finală
                                    if initial_grouping_done {
                                        next_pos = utils::get_random_valid_move(&world_grid, hero.x, hero.y);
                                    }
                                    // CRITIC: Dacă NU s-au grupat, Liderul stă nemișcat la Y=10 și își așteaptă partenerul!
                                }
                            }
                            leader_future_pos = next_pos; 
                            
                        } else {
                            // FOLLOWER-UL
                            if hero.x == leader_pos.0 && hero.y == leader_pos.1 {
                                // Dacă este pe aceeași poziție, copiază orbește mutarea Liderului
                                next_pos = leader_future_pos;
                            } else {
                                // Dacă este despărțit, aleargă țintit după Lider
                                next_pos = utils::bfs_next_step(&world_grid, (hero.x, hero.y), leader_pos);
                                if next_pos == (hero.x, hero.y) && initial_grouping_done {
                                    // Doar dacă sunt grupați teoretic dar blocați de un perete, dăm random move ca fallback
                                    next_pos = utils::get_random_valid_move(&world_grid, hero.x, hero.y);
                                }
                                // CRITIC: Dacă nu s-au unit încă și a ajuns la Lider, stă pe loc pentru a activa contopirea în runda următoare!
                            }
                        }

                        let mut taunt_msg = None;
                        if hero.cooldown > 0 {
                            taunt_msg = Some("Dodge! 🤸".to_string());
                        } else if hero.id != leader_id && (hero.x == leader_pos.0 && hero.y == leader_pos.1) {
                            taunt_msg = Some("Synced! 👯".to_string());
                        } else if !initial_grouping_done {
                            taunt_msg = Some("Wait up! 🤝".to_string());
                        } else if locked_target_id.is_some() {
                            taunt_msg = Some("Pushing! 👀".to_string());
                        }

                        let move_command = WebSocketMessage {
                            command: Command::Move,
                            args: serde_json::to_value(MoveArgs {
                                hero_id: hero.id,
                                x: next_pos.0,
                                y: next_pos.1,
                                comment: taunt_msg, 
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
                
                match args.winner {
                    Some(ref name) if name == "vladChiper" => {
                        println!("🏆 Victorie!");
                    },
                    Some(ref name) => {
                        println!("💀 Lose. Câștigătorul este: {}", name);
                    },
                    None => {
                        if args.reason == "tie" {
                            println!("🤝 Egalitate!");
                        } else {
                            println!("Meciul s-a încheiat fără un câștigător clar.");
                        }
                    }
                }
                break; 
            },
        }
    }
    Ok(())
}