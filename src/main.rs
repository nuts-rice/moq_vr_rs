

mod controls; 
mod video;
use video::run_video_broadcast;
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    moq_native::Log::new(tracing::Level::DEBUG).init();

    let origin = moq_lite::Origin::produce();

    tokio::select! {
        res = run_session(origin.consume()) => res,
        res = run_heartbeat_broadcast(origin.clone()) => res,
        res = run_video_broadcast(origin) => res,
    }
    
}


async fn run_session(origin: moq_lite::OriginConsumer) -> anyhow::Result<()> {

    let client = moq_native::ClientConfig::default().init()?;

    let url = url::Url::parse("http://localhost:4443/anon").unwrap();

    let session = client.with_publish(origin).connect(url).await?;

    session.closed().await.map_err(Into::into)
}


async fn run_heartbeat_broadcast(origin: moq_lite::OriginProducer) -> anyhow::Result<()> {

    let mut broadcast = moq_lite::Broadcast::produce();

    let mut track = broadcast.create_track(moq_lite::Track::new("heartbeat"))?;

    origin.publish_broadcast("", broadcast.consume());

    let mut group = track.append_group()?;

    group.write_frame(bytes::Bytes::from_static(b"heartbeat"))?;

    group.finish()?;

    Ok(())

}
