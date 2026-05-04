use bytes::Bytes;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use hang::container::{Frame, Timestamp, OrderedProducer};

const POSE_HZ : u64 = 90;
/// A single timestamp entry from the viewer.
#[derive(serde::Deserialize, Debug)]
struct RawTimestamp {
	label: String,
	/// Media timestamp in milliseconds.
	ts: f64,
}

/// A parsed timestamp entry with Duration.
#[derive(Debug)]
pub struct TimestampEntry {
	pub label: String,
	pub ts: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoseType {
    Head,
    Hand,
    Controller,
}

#[derive(Debug)]
pub enum Command {
    SetPose {
        pose_type: PoseType,
        viewer_id: String,
        position: [f32; 3],
        rotation: [f32; 4],
    },
    SetButtonState {
        button_name: String,
        pressed: bool,
    },
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Pose {
    pub pos: [f32; 3],
    pub rot: [f32; 4],
}

#[derive(Deserialize, Serialize, Debug)]
pub struct PoseFrame {
    pub ts: u64,
    head: Pose, 
    left_hand: Pose,
    right_hand: Pose, 
}

impl PoseType {
    const ALL: [PoseType; 3] = [Self::Head, Self::Hand, Self::Controller];
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "head" => Some(Self::Head),
            "hand" => Some(Self::Hand),
            "controller" => Some(Self::Controller),
            _ => None,
        }
    }
}

pub async fn run_pose_broadcast(
    viewer_id: &str,
    origin: moq_lite::OriginProducer, 
) -> anyhow::Result<()> {
    let mut broadcast = moq_lite::Broadcast::produce();
    let track = broadcast.create_track(moq_lite::Track {
name: "pose".to_string(),
priority: 10,
    })?;
    origin.publish_broadcast(format!("pose/{viewer_id}"), broadcast.consume());

    let group_window = Timestamp::from_millis(100)?;
    let mut producer = OrderedProducer::new(track).with_max_group_duration(group_window);
    let frame_dur = Duration::from_secs_f64(1.0 / POSE_HZ as f64);
    let start = std::time::Instant::now();
    let mut t = 0.0;
    loop {
        let elapsed_us = start.elapsed().as_micros() as u64;
        let ts = Timestamp::from_micros(elapsed_us)?;
        let frame = synthetic_pose_frame(ts.as_millis() as u64, t);
        let payload = Bytes::from(bincode::serialize(&frame)?);
        producer.write(Frame { timestamp: ts, payload: payload.into() })?;
        t += frame_dur.as_secs_f64();
        tokio::time::sleep(frame_dur).await;
    }

}


fn synthetic_pose_frame(ts_us: u64, t: f64) -> PoseFrame {
   let head_y = 1.7 + 0.05 * (t * 1.0).sin() as f32;
   let yaw = (t * 0.2) as f32;
   let head = Pose {
        pos: [0.0, head_y, 0.0],
        rot: [0.0, (yaw / 2.0).sin(), 0.0, (yaw / 2.0).cos()],
    };
    let a = (t * 0.5) as f32;
    let left_hand = Pose {
    pos: [-0.4 + 0.1 * a.cos(), 1.2, -0.3 + 0.1 * a.sin()],
    rot: [0.0, 0.0, 0.0, 1.0],
    };
    let right_hand = Pose {
        pos: [0.4 + 0.1 * a.cos(), 1.2, -0.3 + 0.1 * a.sin()],
        rot: [0.0, 0.0, 0.0, 1.0],
   };
    PoseFrame { ts: ts_us, head, left_hand, right_hand }
}

async fn handle_viewer_commands(
    viewer_id: &str,
    broadcast: moq_lite::BroadcastConsumer,
    cmd_tx: &tokio::sync::mpsc::Sender<Command>,
) -> anyhow::Result<()> {
    todo!()
}
