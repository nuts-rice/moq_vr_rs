use std::time::Duration;
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

async fn handle_viewer_commands(
    viewer_id: &str,
    broadcast: moq_lite::BroadcastConsumer,
    cmd_tx: &tokio::sync::mpsc::Sender<Command>,
) -> anyhow::Result<()> {
    todo!()
}
