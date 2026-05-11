use bytes::Bytes;
use hang::container::{Frame, OrderedProducer, Timestamp};
use openxr as xr;
use serde::{Deserialize, Serialize};
use std::time::Duration;

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

pub async fn run_pose_broadcast(
    viewer_id: &str,
    origin: moq_lite::OriginProducer,
    hz: u32,
) -> anyhow::Result<()> {
    let mut broadcast = moq_lite::Broadcast::produce();
    let track = broadcast.create_track(moq_lite::Track {
        name: "pose".to_string(),
        priority: 10,
    })?;
    origin.publish_broadcast(format!("pose/{viewer_id}"), broadcast.consume());

    let group_window = Timestamp::from_millis(100)?;
    let mut producer = OrderedProducer::new(track).with_max_group_duration(group_window);
    let frame_dur = Duration::from_secs_f64(1.0 / hz as f64);
    let start = std::time::Instant::now();
    let mut t = 0.0;
    tokio::task::spawn_blocking(move || pose_loop(producer, frame_dur)).await??;
    Ok(())
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
    PoseFrame {
        ts: ts_us,
        head,
        left_hand,
        right_hand,
    }
}

fn pose_loop(mut producer: OrderedProducer, frame_dur: Duration) -> anyhow::Result<()> {
    let entry = xr::Entry::linked();
    let extensions = entry.enumerate_extensions()?;
    let headless = extensions.mnd_headless;
    let mut enabled_exts = xr::ExtensionSet::default();
    enabled_exts.mnd_headless = headless;
    enabled_exts.ext_hand_tracking = extensions.ext_hand_tracking;
    let instance = entry.create_instance(
        &xr::ApplicationInfo {
            application_name: "Pose Broadcast",
            ..Default::default()
        },
        &enabled_exts,
        &[],
    )?;
    let system = instance.system(xr::FormFactor::HEAD_MOUNTED_DISPLAY)?;
    let (session, mut frame_wait, mut frame_stream) = if headless {
        unsafe {
            instance.create_session::<xr::Headless>(system, &xr::headless::SessionCreateInfo {})?
        }
    } else {
        anyhow::bail!("Non-headless mode is not implemented yet");
    };
    let (mut event_storage, mut session_state) =
        (xr::EventDataBuffer::new(), xr::SessionState::IDLE);
    session.begin(xr::ViewConfigurationType::PRIMARY_STEREO)?;
    let action_set = instance.create_action_set("pose", "Pose", 0)?;
    let left_grip = action_set.create_action::<xr::Posef>("left_grip", "Left Grip", &[])?;
    let right_grip = action_set.create_action::<xr::Posef>("right_grip", "Right Grip", &[])?;
    instance.suggest_interaction_profile_bindings(
        instance.string_to_path("/interaction_profiles/khr/simple_controller")?,
        &[
            xr::Binding::new(
                &left_grip,
                instance.string_to_path("/user/hand/left/input/grip/pose")?,
            ),
            xr::Binding::new(
                &right_grip,
                instance.string_to_path("/user/hand/right/input/grip/pose")?,
            ),
        ],
    )?;
    session.attach_action_sets(&[&action_set])?;
    let right_space = right_grip
        .create_space(&session, xr::Path::NULL, xr::Posef::IDENTITY)
        .unwrap();
    let left_space = left_grip
        .create_space(&session, xr::Path::NULL, xr::Posef::IDENTITY)
        .unwrap();
    let stage = session
        .create_reference_space(xr::ReferenceSpaceType::STAGE, xr::Posef::IDENTITY)
        .unwrap();
    let view_space = session
        .create_reference_space(xr::ReferenceSpaceType::VIEW, xr::Posef::IDENTITY)
        .unwrap();
    let start = std::time::Instant::now();
    loop {
        while let Some(event) = instance.poll_event(&mut event_storage)? {
            match event {
                xr::Event::SessionStateChanged(e) => {
                    session_state = e.state();
                    println!("Session state changed: {:?}", session_state);
                }
                _ => {}
            }
        }
        if session_state != xr::SessionState::FOCUSED {
            std::thread::sleep(Duration::from_millis(100));
            continue;
        }
        session.sync_actions(&[(&action_set).into()])?;
        let now = xr::Time::from_nanos(start.elapsed().as_nanos() as i64);
        let head_loc = stage.locate(&view_space, now)?;
        let left_loc = left_space.locate(&stage, now)?;
        let right_loc = right_space.locate(&stage, now)?;
        let elapsed_us = start.elapsed().as_micros() as u64;
        let ts = Timestamp::from_micros(elapsed_us)?;
        let frame = PoseFrame {
            ts: elapsed_us,
            head: xr_pose_to_pose(&head_loc.pose),
            left_hand: xr_pose_to_pose(&left_loc.pose),
            right_hand: xr_pose_to_pose(&right_loc.pose),
        };

        producer.write(Frame {
            timestamp: ts,
            payload: Bytes::from(bincode::serialize(&frame)?).into(),
        })?;
        std::thread::sleep(frame_dur);
    }
}

fn xr_pose_to_pose(p: &xr::Posef) -> Pose {
    Pose {
        pos: [p.position.x, p.position.y, p.position.z],
        rot: [
            p.orientation.x,
            p.orientation.y,
            p.orientation.z,
            p.orientation.w,
        ],
    }
}
