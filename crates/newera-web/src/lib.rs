//! WebAssembly viewer: load a project, get the plan as SVG and 3D views as
//! PNG. Exposed through a tiny C ABI so the page needs no generated glue.
//!
//! Protocol: the page asks for a buffer with [`alloc`], writes bytes into
//! it, calls a function, then reads [`output_ptr`]/[`output_len`].

use std::sync::Mutex;

use newera_core::{Home, SharedDocument};

struct State {
    home: Option<Home>,
    output: Vec<u8>,
}

static STATE: Mutex<State> = Mutex::new(State {
    home: None,
    output: Vec::new(),
});

fn with_state<T>(f: impl FnOnce(&mut State) -> T) -> T {
    let mut guard = STATE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    f(&mut guard)
}

/// Loads a project from bytes; returns 0 on success. On failure the output
/// holds the error message.
pub fn load(bytes: &[u8]) -> i32 {
    match newera_core::project_from_bytes(bytes) {
        Ok((project, _files)) => {
            let document = SharedDocument::default();
            project.load_into(&mut document.write());
            let home = document.read().home().clone();
            with_state(|s| {
                s.output = serde_json::json!({
                    "name": home.name,
                    "walls": home.walls.len(),
                    "rooms": home.rooms.len(),
                    "furniture": home.furniture.len(),
                })
                .to_string()
                .into_bytes();
                s.home = Some(home);
            });
            0
        }
        Err(err) => {
            with_state(|s| s.output = err.to_string().into_bytes());
            1
        }
    }
}

/// The current storey's plan as SVG.
pub fn plan_svg() -> usize {
    with_state(|s| {
        let Some(home) = &s.home else { return 0 };
        let view = home.level_view(home.current_level());
        let scene = newera_draw::plan_scene(
            &view,
            &newera_draw::SceneOptions {
                show_background: false,
                ..newera_draw::SceneOptions::default()
            },
        );
        s.output = newera_draw::to_svg(&scene, &newera_draw::SvgOptions::default()).into_bytes();
        s.output.len()
    })
}

/// An aerial 3D view as PNG.
pub fn view_png(width: u32, height: u32, yaw: f32, pitch: f32) -> usize {
    with_state(|s| {
        let Some(home) = &s.home else { return 0 };
        let view = newera_render::View::aerial(home, yaw, pitch);
        let image = newera_render::render_home(
            home,
            &view,
            width.clamp(16, 1600),
            height.clamp(16, 1200),
            None,
        );
        let mut png = Vec::new();
        if image::DynamicImage::ImageRgba8(image)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .is_err()
        {
            return 0;
        }
        s.output = png;
        s.output.len()
    })
}

// --- C ABI for JavaScript -------------------------------------------------------

/// Allocates `len` bytes the page can write into; free with [`dealloc`].
#[unsafe(no_mangle)]
pub extern "C" fn alloc(len: usize) -> *mut u8 {
    let mut buffer = Vec::<u8>::with_capacity(len);
    let ptr = buffer.as_mut_ptr();
    std::mem::forget(buffer);
    ptr
}

/// Frees a buffer from [`alloc`].
///
/// # Safety
/// `ptr` must come from `alloc(len)` and not be used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dealloc(ptr: *mut u8, len: usize) {
    // SAFETY: the caller hands back a buffer created by `alloc(len)`.
    drop(unsafe { Vec::from_raw_parts(ptr, 0, len) });
}

/// Loads the project in `ptr[..len]`.
///
/// # Safety
/// `ptr` must point to `len` initialized bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn load_project(ptr: *const u8, len: usize) -> i32 {
    // SAFETY: the caller guarantees `len` readable bytes at `ptr`.
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    load(bytes)
}

#[unsafe(no_mangle)]
pub extern "C" fn render_plan_svg() -> usize {
    plan_svg()
}

#[unsafe(no_mangle)]
pub extern "C" fn render_view_png(width: u32, height: u32, yaw: f32, pitch: f32) -> usize {
    view_png(width, height, yaw, pitch)
}

#[unsafe(no_mangle)]
pub extern "C" fn output_ptr() -> *const u8 {
    with_state(|s| s.output.as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn output_len() -> usize {
    with_state(|s| s.output.len())
}

#[cfg(test)]
mod tests {
    use newera_core::{Document, Point2, Wall};

    use super::*;

    #[test]
    fn loads_a_project_and_renders_plan_and_view() {
        let mut doc = Document::default();
        let wall = Wall::new(
            doc.new_wall_id(),
            Point2::new(0.0, 0.0),
            Point2::new(300.0, 0.0),
        );
        doc.execute(newera_core::Command::insert(wall)).unwrap();
        let json = newera_core::to_project_json(&doc);
        assert_eq!(load(json.as_bytes()), 0);
        assert!(
            String::from_utf8_lossy(&with_state(|s| s.output.clone())).contains(r#""walls":1"#)
        );
        assert!(plan_svg() > 100);
        assert!(with_state(
            |s| s.output.starts_with(b"<svg") || s.output.starts_with(b"<?xml")
        ));
        assert!(view_png(64, 48, -60.0, 45.0) > 0);
        assert!(with_state(|s| s.output.starts_with(b"\x89PNG")));
        assert_eq!(load(b"not a project"), 1);
    }
}
