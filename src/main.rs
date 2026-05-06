mod bridge;
mod config;
use bridge::{bridge::run_bridge };
use config::Config;
mod controls;
mod video;

use controls::run_pose_broadcast;
use video::run_video_broadcast;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    moq_native::Log::new(tracing::Level::DEBUG).init();

    let config = Config::load()?;
    let origin = moq_lite::Origin::produce();

 tokio::spawn(run_heartbeat_broadcast(origin.clone()));
    let relay_url = config.relay.url.clone();
    let viewer_id = config.pose.viewer_id.clone();
    let bind = config.bridge.bind.clone();

    tokio::select! {
        res = run_session(&relay_url, origin.consume()) => res,
        res = run_video_broadcast(origin.clone(), config.clone()) => res,
        res = run_pose_broadcast(&viewer_id, origin.clone()) => res,
        res = run_bridge(&bind, origin, config) => res,
    }
}

async fn run_session(relay_url: &str, origin: moq_lite::OriginConsumer) -> anyhow::Result<()> {
    let client = moq_native::ClientConfig::default().init()?;
    let url = url::Url::parse(relay_url)?;
    let session = client.with_publish(origin).connect(url).await?;
    session.closed().await.map_err(Into::into)
}
 async fn run_heartbeat_broadcast(origin: moq_lite::OriginProducer) -> anyhow::Result<()> {
      let mut broadcast = moq_lite::Broadcast::produce();
      let mut track = broadcast.create_track(moq_lite::Track::new("heartbeat"))?;
      origin.publish_broadcast("heartbeat", broadcast.consume());
      let mut interval = tokio::time::interval(std::time::Duration::from_secs(15));
      loop {
          interval.tick().await;
          let mut group = track.append_group()?;
          group.write_frame(bytes::Bytes::from_static(b"heartbeat"))?;
          group.finish()?;
      }
  }
