//! Points of view: stored cameras, and the keyframe path a video follows.

use std::path::PathBuf;

use newera_core::Command;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{ErrorData, tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;

use super::NewEraMcp;
use super::reply::{core, invalid, ok, write_action};
use crate::compact;

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct CamerasParams {
    /// `view`, `aerial`, `store` or `delete`.
    pub(crate) action: Option<String>,
    /// Stored view index.
    pub(crate) i: Option<usize>,
    pub(crate) name: Option<String>,
    pub(crate) x: Option<f64>,
    pub(crate) y: Option<f64>,
    /// Eye height cm.
    pub(crate) z: Option<f64>,
    pub(crate) yaw: Option<f64>,
    /// Degrees down.
    pub(crate) pitch: Option<f64>,
    /// Horizontal field of view, degrees.
    pub(crate) fov: Option<f64>,
    /// For `store`: point `[x,y,z]` cm to look at (sets yaw and pitch).
    pub(crate) look_at: Option<[f64; 3]>,
}
#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct VideoParams {
    /// `add`, `delete`, `clear`, `orbit`, `set` or `render`.
    action: Option<String>,
    /// Keyframe index (`delete`, or insert position for `add`).
    i: Option<usize>,
    /// Stored view index to add as keyframe.
    cam: Option<usize>,
    x: Option<f64>,
    y: Option<f64>,
    z: Option<f64>,
    yaw: Option<f64>,
    pitch: Option<f64>,
    fov: Option<f64>,
    /// Keyframes for `orbit`.
    n: Option<usize>,
    fps: Option<u32>,
    /// Camera speed m/s.
    speed: Option<f64>,
    /// Output `.avi` for `render`.
    path: Option<String>,
    w: Option<u32>,
    h: Option<u32>,
}
#[tool_router(router = cameras_router, vis = "pub(crate)")]
impl NewEraMcp {
    #[tool(
        name = "cameras",
        description = "Stored points of view: {active: visitor|aerial, rows [i,name,x,y,z,yaw,pitch,fov]}, cm and degrees. Change them with edit_cameras."
    )]
    pub(crate) fn list_cameras(&self) -> Result<String, ErrorData> {
        self.cameras(Parameters(CamerasParams::default()))
    }
    #[tool(
        description = "Change the points of view. view {i} shows stored view i in the 3D window; aerial returns to the orbit view; store {name?,x?,y?,z?,yaw?,pitch?,fov?,look_at?:[x,y,z]} saves one (missing values from the visitor; yaw 0 looks toward +y/plan bottom, 90 toward -x; pitch positive looks down); delete {i}. cm and degrees. The list is the cameras tool."
    )]
    pub(crate) fn edit_cameras(
        &self,
        Parameters(p): Parameters<CamerasParams>,
    ) -> Result<String, ErrorData> {
        write_action(
            p.action.as_deref(),
            &["view", "aerial", "store", "delete"],
            "cameras",
        )?;
        self.cameras(Parameters(p))
    }
    /// Points of view: the list, and every change to it.
    pub(crate) fn cameras(
        &self,
        Parameters(p): Parameters<CamerasParams>,
    ) -> Result<String, ErrorData> {
        let mut doc = self.document.write();
        let mut cameras = doc.home().cameras.clone();
        let index = |len: usize| -> Result<usize, ErrorData> {
            p.i.filter(|i| *i < len)
                .ok_or_else(|| invalid(format!("`i` must be below {len}")))
        };
        match p.action.as_deref().unwrap_or("list") {
            "list" => {
                let rows: Vec<serde_json::Value> = cameras
                    .stored
                    .iter()
                    .enumerate()
                    .map(|(i, c)| {
                        serde_json::json!([
                            i,
                            c.name,
                            compact::num(c.x),
                            compact::num(c.y),
                            compact::num(c.z),
                            compact::num(c.yaw),
                            compact::num(c.pitch),
                            compact::num(c.fov)
                        ])
                    })
                    .collect();
                return Ok(serde_json::json!({"active": if cameras.observer_active { "visitor" } else { "aerial" }, "rows": rows}).to_string());
            }
            "view" => {
                let i = index(cameras.stored.len())?;
                cameras.observer = cameras.stored[i].clone();
                cameras.observer_active = true;
            }
            "aerial" => cameras.observer_active = false,
            "store" => {
                let base = cameras.observer.clone();
                let camera = newera_core::Camera {
                    name: p
                        .name
                        .clone()
                        .or_else(|| Some(format!("Ponto de vista {}", cameras.stored.len() + 1))),
                    x: p.x.unwrap_or(base.x),
                    y: p.y.unwrap_or(base.y),
                    z: p.z.unwrap_or(base.z),
                    yaw: p.yaw.unwrap_or(base.yaw),
                    pitch: p.pitch.unwrap_or(base.pitch),
                    fov: p.fov.unwrap_or(base.fov),
                    ..base
                };
                let mut camera = camera;
                if let Some([tx, ty, tz]) = p.look_at {
                    let (dx, dy, dz) = (tx - camera.x, ty - camera.y, tz - camera.z);
                    // `Camera::direction` is (-sin yaw, cos yaw) on the plan.
                    camera.yaw = (-dx).atan2(dy).to_degrees();
                    camera.pitch = (-dz).atan2(dx.hypot(dy)).to_degrees();
                }
                cameras.stored.push(camera);
            }
            "delete" => {
                let i = index(cameras.stored.len())?;
                cameras.stored.remove(i);
            }
            other => return Err(invalid(format!("unknown action `{other}`"))),
        }
        doc.execute(Command::SetCameras { cameras }).map_err(core)?;
        Ok(ok(&doc, &[]))
    }
    #[tool(
        name = "video",
        description = "The video camera path: {fps,speed,secs,rows [i,x,y,z,yaw,pitch,fov]}, cm, degrees, m/s. Change it and render it with edit_video."
    )]
    pub(crate) fn list_video(&self) -> Result<String, ErrorData> {
        self.video(Parameters(VideoParams::default()))
    }
    #[tool(
        description = "Change the video camera path, or render it. add {cam? | x?,y?,z?,yaw?,pitch?,fov?, i?} appends a keyframe (missing values from the visitor); delete {i}; clear; orbit {z?,n?} replaces the path with an aerial tour; set {fps?,speed?}; render {path .avi, w?,h?} writes a Motion-JPEG video. cm, degrees, m/s. The path itself is the video tool."
    )]
    pub(crate) fn edit_video(
        &self,
        Parameters(p): Parameters<VideoParams>,
    ) -> Result<String, ErrorData> {
        write_action(
            p.action.as_deref(),
            &["add", "delete", "clear", "orbit", "set", "render"],
            "video",
        )?;
        self.video(Parameters(p))
    }
    /// The video camera path: the list, and every change to it.
    pub(crate) fn video(
        &self,
        Parameters(p): Parameters<VideoParams>,
    ) -> Result<String, ErrorData> {
        let mut doc = self.document.write();
        let mut environment = doc.home().environment.clone();
        let path = &mut environment.camera_path;
        match p.action.as_deref().unwrap_or("list") {
            "list" => {
                let rows: Vec<serde_json::Value> = path
                    .iter()
                    .enumerate()
                    .map(|(i, c)| {
                        serde_json::json!([
                            i,
                            compact::num(c.x),
                            compact::num(c.y),
                            compact::num(c.z),
                            compact::num(c.yaw),
                            compact::num(c.pitch),
                            compact::num(c.fov)
                        ])
                    })
                    .collect();
                let video = &environment.video;
                #[allow(clippy::cast_precision_loss)]
                let secs = if path.len() < 2 {
                    0.0
                } else {
                    newera_render::video::frame_count(path, video.frame_rate, video.speed) as f64
                        / f64::from(video.frame_rate.max(1))
                };
                return Ok(serde_json::json!({
                    "fps": video.frame_rate,
                    "speed": compact::num(video.speed),
                    "secs": secs,
                    "rows": rows,
                })
                .to_string());
            }
            "add" => {
                let cameras = &doc.home().cameras;
                let base = match p.cam {
                    Some(i) => cameras
                        .stored
                        .get(i)
                        .cloned()
                        .ok_or_else(|| invalid(format!("no stored camera {i}")))?,
                    None => cameras.observer.clone(),
                };
                let camera = newera_core::Camera {
                    name: None,
                    x: p.x.unwrap_or(base.x),
                    y: p.y.unwrap_or(base.y),
                    z: p.z.unwrap_or(base.z),
                    yaw: p.yaw.unwrap_or(base.yaw),
                    pitch: p.pitch.unwrap_or(base.pitch),
                    fov: p.fov.unwrap_or(base.fov),
                    ..base
                };
                let at = p.i.unwrap_or(path.len()).min(path.len());
                path.insert(at, camera);
            }
            "delete" => {
                let i =
                    p.i.filter(|i| *i < path.len())
                        .ok_or_else(|| invalid(format!("`i` must be below {}", path.len())))?;
                path.remove(i);
            }
            "clear" => path.clear(),
            "orbit" => {
                *path = newera_render::video::orbit_path(
                    doc.home(),
                    p.z.unwrap_or(800.0),
                    p.n.unwrap_or(8),
                );
            }
            "set" => {
                if let Some(fps) = p.fps {
                    environment.video.frame_rate = fps.clamp(1, 60);
                }
                if let Some(speed) = p.speed {
                    environment.video.speed = speed.clamp(0.05, 50.0);
                }
            }
            "render" => {
                let file = PathBuf::from(
                    p.path
                        .as_deref()
                        .ok_or_else(|| invalid("`path` is required"))?,
                );
                let home = doc.home().clone();
                let assets = doc.asset_dir();
                drop(doc);
                let (w, h) = (
                    p.w.unwrap_or(640).clamp(64, 1920),
                    p.h.unwrap_or(360).clamp(64, 1080),
                );
                let video = &home.environment.video;
                let info = newera_render::video::render_video(
                    &home,
                    &home.environment.camera_path,
                    video.frame_rate,
                    video.speed,
                    (w, h),
                    assets.as_deref(),
                    &file,
                    |_, _| {},
                )
                .map_err(invalid)?;
                return Ok(serde_json::json!({
                    "frames": info.frames,
                    "secs": info.seconds,
                    "bytes": info.bytes,
                })
                .to_string());
            }
            other => return Err(invalid(format!("unknown action `{other}`"))),
        }
        doc.execute(Command::SetEnvironment { environment })
            .map_err(core)?;
        Ok(ok(&doc, &[]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::CreateParams;
    use crate::tools::server;

    #[test]
    fn video_list_duration_matches_the_encoded_frames() {
        let s = server();
        let listed_seconds = || {
            let list: serde_json::Value =
                serde_json::from_str(&s.video(Parameters(VideoParams::default())).unwrap())
                    .unwrap();
            list["secs"].as_f64().unwrap()
        };
        assert!(listed_seconds().abs() < f64::EPSILON);
        s.video(Parameters(VideoParams {
            action: Some("add".into()),
            ..VideoParams::default()
        }))
        .unwrap();
        assert!(listed_seconds().abs() < f64::EPSILON);
        for (distance, fps, expected) in [(0.0, 1, 2.0), (0.0, 25, 0.2), (26.0, 25, 0.28)] {
            let keys = vec![
                newera_core::Camera::default(),
                newera_core::Camera {
                    x: newera_core::Camera::default().x + distance,
                    ..newera_core::Camera::default()
                },
            ];
            {
                let mut doc = s.document.write();
                let mut environment = doc.home().environment.clone();
                environment.camera_path = keys.clone();
                environment.video.frame_rate = fps;
                environment.video.speed = 1.0;
                doc.execute(Command::SetEnvironment { environment })
                    .unwrap();
            }
            let secs = listed_seconds();
            assert!((secs - expected).abs() < 1e-9);
            let (bytes, info) = newera_render::video::render_video_bytes(
                s.document.read().home(),
                &keys,
                fps,
                1.0,
                (16, 16),
                None,
                |_, _| {},
            )
            .unwrap();
            let header = bytes.windows(4).position(|w| w == b"avih").unwrap() + 8;
            let frames = u32::from_le_bytes(bytes[header + 16..header + 20].try_into().unwrap());
            assert!((secs - f64::from(frames) / f64::from(fps)).abs() < 1e-9);
            assert!((secs - info.seconds).abs() < 1e-9);
        }
    }

    #[test]
    fn video_path_keyframes_and_render() {
        let s = server();
        let params: CreateParams = serde_json::from_str(
            r#"{"walls":[{"pts":[[0,0],[400,0],[400,300],[0,300]],"closed":true}]}"#,
        )
        .unwrap();
        s.create(Parameters(params)).unwrap();
        let act = |action: &str| VideoParams {
            action: Some(action.into()),
            ..VideoParams::default()
        };
        assert!(s.video(Parameters(act("render"))).is_err());
        s.video(Parameters(act("orbit"))).unwrap();
        s.video(Parameters(VideoParams {
            fps: Some(4),
            speed: Some(20.0),
            ..act("set")
        }))
        .unwrap();
        s.video(Parameters(VideoParams {
            x: Some(200.0),
            y: Some(150.0),
            i: Some(0),
            ..act("add")
        }))
        .unwrap();
        s.video(Parameters(VideoParams {
            i: Some(0),
            ..act("delete")
        }))
        .unwrap();
        let list: serde_json::Value =
            serde_json::from_str(&s.video(Parameters(VideoParams::default())).unwrap()).unwrap();
        assert_eq!(list["rows"].as_array().unwrap().len(), 9);
        assert_eq!(list["fps"], 4);
        let dir = std::env::temp_dir().join(format!("newera-mcp-video-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("tour.avi");
        let reply: serde_json::Value = serde_json::from_str(
            &s.video(Parameters(VideoParams {
                path: Some(file.display().to_string()),
                w: Some(64),
                h: Some(64),
                ..act("render")
            }))
            .unwrap(),
        )
        .unwrap();
        assert!(reply["frames"].as_u64().unwrap() >= 2, "{reply}");
        assert!(std::fs::read(&file).unwrap().starts_with(b"RIFF"));
        s.video(Parameters(act("clear"))).unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn cameras_can_look_at_a_point() {
        let s = server();
        s.cameras(Parameters(CamerasParams {
            action: Some("store".into()),
            x: Some(0.0),
            y: Some(0.0),
            z: Some(170.0),
            look_at: Some([100.0, 100.0, 70.0]),
            ..CamerasParams::default()
        }))
        .unwrap();
        let doc = s.document.read();
        let camera = &doc.home().cameras.stored[0];
        let (dx, dy) = camera.direction();
        let k = std::f64::consts::FRAC_1_SQRT_2;
        assert!((dx - k).abs() < 1e-9 && (dy - k).abs() < 1e-9, "{dx} {dy}");
        // 100 cm down over 141 cm: about 35° below the horizon.
        assert!((camera.pitch - 35.26).abs() < 0.1, "{}", camera.pitch);
    }
}
