use anyhow::Context;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use std::time::Duration;
use std::collections::HashMap;

mod protocol;
mod utils;
mod simulator; // Terenul de joacă mental (Regulile jocului)
mod mcts;      // Creierul care ghicește viitorul (UCB1)

use crate::protocol::{Command, ErrorArgs, MoveArgs, ShootArgs, StartMatchArgs, StartTurnArgs};
use crate::simulator::{Action, SimState};
use crate::mcts::Mcts;

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

    println!("🔗 Conectat la server!");

    let mut my_player_id = 0;
    let mut map_width = 51;
    let mut map_height = 90;
    
    // Harta fixă a lumii (Rămâne ca memorie globală pentru a o da rapid simulatorului)
    let mut world_grid: Vec<Vec<i32>> = Vec::new();

    // Reține ID-ul inamicului și ultima poziție (x, y) în care a fost văzut
    let mut last_known_enemies: std::collections::HashMap<i32, (i32, i32)> = std::collections::HashMap::new();

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

                let setup_msg: WebSocketMessage = WebSocketMessage {
                    command: Command::Practice,
                    args: serde_json::json!({
                        "seed": null,
                        "ranked": false,
                        "my_id": 1,
                        "name": "robertcd29",
                    }),
                };

                send_command(&mut write, setup_msg).await.expect("Failed to start practice");
            }
            Command::Challenge => println!("You have been challenged!"),
            Command::Practice => println!("Modul de antrenament activat!"),
            Command::StartMatch => {
                let args: StartMatchArgs = serde_json::from_value(message.args.clone())
                    .context("Parsing StartMatchArgs")?;

                for hero in &args.state.heroes {
                    if hero.owner_id != my_player_id {
                        last_known_enemies.insert(hero.id, (hero.x, hero.y));
                    }
                }

                println!("🏁 Meci început! ID: {}", args.match_id);
                my_player_id = args.your_player_id;

                map_width = args.config.width;
                map_height = args.config.height;
                
                // Setăm harta de coliziuni (1 singură dată la început de meci)
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
            }
            Command::StartTurn => {
                let args: StartTurnArgs = serde_json::from_value(message.args.clone())
                    .context("Parsing StartTurnArgs")?;

                let sim_state = SimState::from_state(&args.state, &world_grid, my_player_id, map_width, map_height);
                let mut mcts = Mcts::new(sim_state);
                
                let best_actions = mcts.search(Duration::from_millis(150));

                for (hero_id, action) in best_actions {
                    match action {
                        Action::Move { x, y } => {
                            let move_command = WebSocketMessage {
                                command: Command::Move,
                                args: serde_json::json!(MoveArgs {
                                    hero_id,
                                    x,
                                    y,
                                    comment: Some("TACTICAL PUSH! 🚀".to_string()), 
                                }),
                            };
                            send_command(&mut write, move_command).await.unwrap();
                        },
                        Action::Shoot { x, y } => {
                            let shoot_command = WebSocketMessage {
                                command: Command::Shoot,
                                args: serde_json::json!(ShootArgs {
                                    hero_id,
                                    x,
                                    y,
                                    comment: Some("MCTS Sniper Elite! 🎯".to_string()),
                                }),
                            };
                            send_command(&mut write, shoot_command).await.unwrap();
                        },
                        Action::Wait => {
                            if let Some(hero) = args.state.heroes.iter().find(|h| h.id == hero_id) {
                                let move_command = WebSocketMessage {
                                    command: Command::Move,
                                    args: serde_json::json!(MoveArgs {
                                        hero_id,
                                        x: hero.x,
                                        y: hero.y,
                                        comment: Some("Holding the line! 🛡️".to_string()),
                                    }),
                                };
                                send_command(&mut write, move_command).await.unwrap();
                            }
                        }
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
                    Some(ref name) if name == "vladChiper" => println!("🏆 Victorie supremă MCTS!"),
                    Some(ref name) => println!("💀 Am pierdut. Inamicul a fost mai bun: {}", name),
                    None => println!("🤝 Egalitate!"),
                }
                break; 
            },
        }
    }
    Ok(())
}