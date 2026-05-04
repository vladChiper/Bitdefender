use anyhow::Context;
use futures_util::{SinkExt, StreamExt, stream::SplitSink};
use serde::{Deserialize, Serialize};
use std::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};
mod protocol;
use crate::protocol::{Hero, StartMatchArgs, MoveArgs, ShootArgs, StartTurnArgs, ErrorArgs};
use rand::Rng;
use crate::protocol::Command;
// use std::process::Command;

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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let url = "wss://bitdefenders.cvjd.me/ws";
    let (ws, _) = connect_async(url).await.unwrap();
    let (mut write, mut read) = ws.split();

    println!("connected");

    let my_player_id = 0;

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
                    seed: Some(12345),
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
                println!("The match has started!");
                
                 let args: StartMatchArgs = serde_json::from_value(message.args.clone()).context("Parsing StartMatchArgs")?;

                println!("Meci început! ID: {}", args.match_id);
    
            
                let my_heroes: Vec<&Hero> = args.state.heroes.iter()
                    .filter(|h| h.owner_id == args.your_player_id)
                    .collect();

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
                    // Aici putem pune logica de Pathfinding mai târziu. 
                    // Momentan păstrăm mișcarea direcțională aleatorie din codul tău.
                    let dx = rng.gen_range(-1..=1);
                    let dy = rng.gen_range(-1..=1);


                    // Evităm să stăm pe loc, forțăm o mișcare dacă dx și dy sunt ambele 0
                    let (final_dx, final_dy) = if dx == 0 && dy == 0 {
                        (1, 0) 
                    } else {
                        (dx, dy)
                    };

                    // Înmulțim direcția cu 10 ca ținta să iasă clar din footprint-ul de 3x3 al eroului
                    let target_x = hero.x + (final_dx * 10);
                    let target_y = hero.y + (final_dy * 10);

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
