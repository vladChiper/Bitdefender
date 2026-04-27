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
            }
            Command::StartMatch => {
                println!("The match has started!");
                
                 let args: StartMatchArgs = serde_json::from_value(message.args.clone()).context("Parsing StartMatchArgs")?;

                println!("Meci început! ID: {}", args.match_id);
    
            
                let my_heroes: Vec<&Hero> = args.state.heroes.iter()
                    .filter(|h| h.owner_id == args.your_player_id)
                    .collect();

            }
            Command::StartTurn => {
                // Deserializăm argumentele specifice pentru turnul curent
                let args: StartTurnArgs = serde_json::from_value(message.args.clone())
                    .context("Parsing StartTurnArgs")?;

                println!("Turnul: {}", args.turn);
                let mut rng = rand::thread_rng();

                // Identificăm eroii care ne aparțin folosind my_player_id salvat anterior
                let my_heroes: Vec<&Hero> = args.state.heroes.iter()
                    .filter(|h| h.owner_id == my_player_id)
                    .collect();

                for hero in my_heroes {
                    // Generăm o deplasare aleatorie pe axele X și Y (-1, 0, sau 1)
                    let dx = rng.gen_range(-1..=1);
                    let dy = rng.gen_range(-1..=1);

                    // Trimitem comanda MOVE folosind varianta din Enum
                    let move_command = WebSocketMessage {
                        command: Command::Move, // Corectat: folosim Enum-ul, nu String
                        args: serde_json::to_value(MoveArgs {
                            hero_id: hero.id,
                            x: hero.x + dx,
                            y: hero.y + dy,
                        })?,
                    };

                    send_command(&mut write, move_command).await?;
                    println!("Eroul {} se mișcă la coordonatele ({}, {})", hero.id, hero.x + dx, hero.y + dy);
                }
            }
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
