use bytes::Bytes;
use tokio_tungstenite::{accept_async, tungstenite::Message};
use tokio::net::{TcpListener, TcpStream};
use futures_util::{SinkExt, StreamExt};
const TAG_VIDEO: u8 = 0x01;
const TAG_POSE_DOWN: u8 = 0x02;

pub async fn run_bridge(bind: &str, relay_url: &str) -> anyhow::Result<()> {
   let sub_origin = moq_lite::Origin::produce();
   let announcements = sub_origin.consume(); 
   let mut client_cfg = moq_native::ClientConfig::default();
   let client = client_cfg.init()?;
   let url = url::Url::parse(relay_url)?; 
   let session = client.with_consume(sub_origin).connect(url).await?;
   tokio::spawn( async move { let _ = session.closed().await; }) ;
   let listener = TcpListener::bind(bind) .await?;
   tracing::info!("Bridge listening on {bind}") ;
   loop {
        let (stream, addr) = listener.accept().await?;
        let announcements = announcements.clone();
        tokio::spawn(async move {
            tracing::info!("New connection from {addr}");
            if let Err(e) = handle_device(stream, announcements).await {
                tracing::error!("Error handling device {addr}: {e}");
            }

        });
    }
}
async fn handle_device(stream: TcpStream, mut announcements: moq_lite::OriginConsumer) -> anyhow::Result<()> {
    let ws = accept_async(stream).await?; 
    let (mut ws_tx, mut ws_rx) = ws.split();
    let mut video_broadcast = None;
    let mut pose_broadcast = None;
    while video_broadcast.is_none() || pose_broadcast.is_none() {
        let Some((path, broadcast)) = announcements.announced().await else {
            anyhow::bail!("Failed to get announcement");
        };
        match path.as_str() {
            "" => video_broadcast = broadcast,
            "pose/local" => pose_broadcast = broadcast,
            _ => {}
        }
    }

    let mut video_track = video_broadcast.unwrap().subscribe_track(&moq_lite::Track::new("video"))?;
    let mut pose_track = pose_broadcast.unwrap().subscribe_track(&moq_lite::Track::new("pose"))?;



 loop {
          tokio::select! {
              frame = video_track.read_frame() => {
                  match frame? {
                      Some(data) => { ws_tx.send(tagged(TAG_VIDEO, data)).await?; }
                      None => {
                          tracing::info!("video track closed");
                          break;
                      }
                  }
              }
              frame = pose_track.read_frame() => {
                  match frame? {
                      Some(data) => { ws_tx.send(tagged(TAG_POSE_DOWN, data)).await?; }
                      None => {
                          tracing::info!("pose track closed");
                          break;
                      }
                  }
              }
              msg = ws_rx.next() => {
                  match msg {
                      Some(Ok(Message::Close(_))) | None => break,
                      _ => {}
                  }
              }
          }
      }
      Ok(())
  }
fn tagged(tag: u8, data: Bytes) -> Message {
    let mut buf = Vec::with_capacity(1 + data.len());
    buf.push(tag);
    buf.extend_from_slice(&data);
    Message::Binary(buf.into())
}
