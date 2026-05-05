use crate::bridge::config::Config;
use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::{accept_async, tungstenite::Message};

const TAG_VIDEO: u8 = 0x01;
const TAG_POSE_DOWN: u8 = 0x02;
const TAG_POSE_UP: u8 = 0x03;

pub async fn run_bridge(
    bind: &str,
    origin: moq_lite::OriginProducer,
    config: Config,
) -> anyhow::Result<()> {
    // Subscribe: pull video + pose/local from relay
    let sub_origin = moq_lite::Origin::produce();
    let announcements = sub_origin.consume();
    let mut client_cfg = moq_native::ClientConfig::default();
    let client = client_cfg.init()?;
    let url = url::Url::parse(&config.relay.url)?;
    let session = client.with_consume(sub_origin).connect(url).await?;
    tokio::spawn(async move {
        let _ = session.closed().await;
    });

    // Publish: pose-up from Quest → relay as pose/remote
    let mut pose_up_broadcast = moq_lite::Broadcast::produce();
    let pose_up_track = pose_up_broadcast.create_track(moq_lite::Track::new("pose"))?;
    origin.publish_broadcast("pose/remote", pose_up_broadcast.consume());

    let (pose_tx, mut pose_rx) = mpsc::channel::<Bytes>(64);
    tokio::spawn(async move {
        let mut track = pose_up_track;
        while let Some(data) = pose_rx.recv().await {
            let Ok(mut group) = track.append_group() else {
                break;
            };
            let _ = group.write_frame(data);
            let _ = group.finish();
        }
    });

    let listener = TcpListener::bind(bind).await?;
    tracing::info!("bridge listening on {bind}");
    loop {
        let (stream, addr) = listener.accept().await?;
        let announcements = announcements.clone();
        let pose_tx = pose_tx.clone();
        tokio::spawn(async move {
            tracing::info!("new connection from {addr}");
            if let Err(e) = handle_device(stream, announcements, pose_tx).await {
                tracing::warn!("bridge {addr}: {e}");
            }
        });
    }
}

async fn handle_device(
    stream: TcpStream,
    mut announcements: moq_lite::OriginConsumer,
    pose_tx: mpsc::Sender<Bytes>,
) -> anyhow::Result<()> {
    let ws = accept_async(stream).await?;
    let (mut ws_tx, mut ws_rx) = ws.split();

    let mut video_broadcast = None;
    let mut pose_broadcast = None;
    while video_broadcast.is_none() || pose_broadcast.is_none() {
        let Some((path, broadcast)) = announcements.announced().await else {
            anyhow::bail!("relay disconnected before broadcasts arrived");
        };
        match path.as_str() {
            "" => video_broadcast = broadcast,
            "pose/local" => pose_broadcast = broadcast,
            _ => {}
        }
    }

    let mut video_track = video_broadcast
        .unwrap()
        .subscribe_track(&moq_lite::Track::new("video"))?;
    let mut pose_track = pose_broadcast
        .unwrap()
        .subscribe_track(&moq_lite::Track::new("pose"))?;

    loop {
        tokio::select! {
            frame = video_track.read_frame() => {
                match frame? {
                    Some(data) => { ws_tx.send(tagged(TAG_VIDEO, data)).await?; }
                    None => break,
                }
            }
            frame = pose_track.read_frame() => {
                match frame? {
                    Some(data) => { ws_tx.send(tagged(TAG_POSE_DOWN, data)).await?; }
                    None => break,
                }
            }
            msg = ws_rx.next() => {
                match msg {
                    Some(Ok(Message::Binary(data))) if data.first() == Some(&TAG_POSE_UP) => {
                        let payload = Bytes::copy_from_slice(&data[1..]);
                        let _ = pose_tx.try_send(payload);
                    }
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
