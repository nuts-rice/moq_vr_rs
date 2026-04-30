use std::collections::BTreeMap;
use bytes::Bytes;
use openh264::{encoder::{BitRate, Complexity, Encoder, EncoderConfig, FrameRate, FrameType, IntraFramePeriod}, formats::YUVBuffer, OpenH264API, Timestamp as EncTimestamp};
use hang::container::{Frame, OrderedProducer};
const WIDTH: usize = 1280;
const HEIGHT: usize = 720;
const FPS: usize = 30;

/*
pub struct VideoEncoder {
    tx: tokio::sync::mpsc::Sender<EncoderMsg>,
    pub track: moq_lite::TrackProducer,
}

struct EncoderMsg {
    rgba: Bytes,
    ts: hang::container::Timestamp,
}
*/

pub async fn run_video_broadcast(origin: moq_lite::OriginProducer) -> anyhow::Result<()> {
    let config = EncoderConfig::new()
        .bitrate(BitRate::from_bps(5_000_000))
        .max_frame_rate(FrameRate::from_hz(FPS as f32))
        .complexity(Complexity::Low)
        .intra_frame_period(IntraFramePeriod::from_num_frames(FPS as u32));

let mut broadcast = moq_lite::Broadcast::produce();
    let track = setup_track(&mut broadcast)?;
    origin.publish_broadcast("", broadcast.consume());
    let mut producer = OrderedProducer::new(track);
    let mut encoder = Encoder::with_api_config(OpenH264API::from_source(), config)?;
    let frame_dur = std::time::Duration::from_secs(1) / FPS as u32; 
    let start = std::time::Instant::now();
    let mut frame_idx = 0;
    loop {
        let yuv = synthetic_frame(WIDTH, HEIGHT, frame_idx);
        let elapsed_ms = start.elapsed().as_millis() as u64;
        let bitstream = encoder.encode_at(&yuv, EncTimestamp::from_millis(elapsed_ms))?;
        let is_keyframe = matches!(bitstream.frame_type(), FrameType::IDR | FrameType::I);
        if is_keyframe {
            producer.keyframe()?;
        }
        let ts = hang::container::Timestamp::from_millis(elapsed_ms)? ;
        let payload = Bytes::from(bitstream.to_vec());
        producer.write (Frame {timestamp: ts, payload: payload.into()})?;
        frame_idx += 1;
        tokio::time::sleep(frame_dur).await;

    }    
}

fn setup_track(broadcast: &mut moq_lite::BroadcastProducer) -> anyhow::Result<moq_lite::TrackProducer> {
let video_track = moq_lite::Track {
        name: "video".to_string(),
        priority: 1,
    };
    let mut renditions = BTreeMap::new();
    renditions.insert(
        video_track.name.clone(),
        hang::catalog::VideoConfig {
            codec: hang::catalog::H264 {
                profile: 0x4D,
                constraints: 0x00,
                level: 0x1F,
                inline: true
                
            }.into() ,
            description: None,
             coded_width: Some(WIDTH as u32),
             coded_height: Some(HEIGHT as u32),
             framerate: Some(FPS as f64),
            display_ratio_width: None,
            display_ratio_height: None,
            bitrate: Some(5_000_000),
            optimize_for_latency: Some(true),
            container: hang::catalog::Container::Legacy,
            jitter: None,
        }
    );

    let catalog = hang::catalog::Catalog {
        video: hang::catalog::Video {
            renditions, 
            display: None,
            rotation: None, 
            flip: None,            
        },
        ..Default::default()
    };
    let mut catalog_track = broadcast.create_track(hang::catalog::Catalog::default_track())?;
    let mut group = catalog_track.append_group()?;
    group.write_frame(Bytes::from(catalog.to_string()?))?;

    group.finish()?;

    Ok(broadcast.create_track(video_track)?)
    
    

}


fn synthetic_frame(width: usize, height: usize, idx: u64) -> YUVBuffer {
    let luma = (idx % 256) as u8;
    let mut data = vec![128u8; (3 * width * height) / 2];
    data[..width * height ].fill(luma);
    YUVBuffer::from_vec(data, width, height)
}
