//! Keeps the C ABI and ownership of Metal resources behind a safe interface.
#![cfg(all(target_os = "macos", target_arch = "aarch64"))]

use std::{ffi::c_void, ptr::NonNull};

pub const TEXTURE_SIZE: usize = 256;
const TEXTURE_BYTES: usize = TEXTURE_SIZE * TEXTURE_SIZE * 4;
const VERTEX_BYTES: usize = 52;
pub type Pixel = [f32; 12];
pub type Lamp = [f32; 16];

// Must match the Params layout in photo.metal and bridge.m.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Params {
    pub eye: [f32; 4],
    pub forward: [f32; 4],
    pub right: [f32; 4],
    pub up: [f32; 4],
    pub sky: [f32; 4],
    pub sun_direction: [f32; 4],
    pub sun_color: [f32; 4],
    pub width: u32,
    pub height: u32,
    pub first_row: u32,
    pub rows: u32,
    pub samples: u32,
    pub bounces: u32,
    pub opaque_triangles: u32,
    pub light_count: u32,
    pub squeeze: f32,
    pub coarse: u32,
    pub padding: [u32; 2],
}

#[derive(Debug)]
pub struct Scene {
    handle: NonNull<c_void>,
    opaque_triangles: u32,
    light_count: u32,
}

unsafe extern "C" {
    fn newera_metal_create(
        shader: *const u8,
        shader_len: usize,
        vertices: *const u8,
        vertex_bytes: usize,
        indices: *const u32,
        index_count: usize,
        textures: *const u8,
        layers: usize,
        lights: *const Lamp,
        light_count: usize,
    ) -> *mut c_void;
    fn newera_metal_render(scene: *mut c_void, params: *const Params, pixels: *mut Pixel) -> bool;
    fn newera_metal_destroy(scene: *mut c_void);
}

impl Scene {
    pub fn new(
        shader: &str,
        vertices: &[u8],
        opaque: &[u32],
        transparent: &[u32],
        textures: &[u8],
        lights: &[Lamp],
    ) -> Option<Self> {
        if vertices.is_empty()
            || !vertices.len().is_multiple_of(VERTEX_BYTES)
            || !opaque.len().is_multiple_of(3)
            || !transparent.len().is_multiple_of(3)
            || !textures.len().is_multiple_of(TEXTURE_BYTES)
        {
            return None;
        }
        let opaque_triangles = u32::try_from(opaque.len() / 3).ok()?;
        let light_count = u32::try_from(lights.len()).ok()?;
        let indices = [opaque, transparent].concat();
        if indices.is_empty()
            || u32::try_from(indices.len() / 3).is_err()
            || indices
                .iter()
                .any(|&i| i as usize >= vertices.len() / VERTEX_BYTES)
        {
            return None;
        }
        // The bridge copies the slices before returning. It owns only its own allocations.
        let handle = unsafe {
            newera_metal_create(
                shader.as_ptr(),
                shader.len(),
                vertices.as_ptr(),
                vertices.len(),
                indices.as_ptr(),
                indices.len(),
                textures.as_ptr(),
                textures.len() / TEXTURE_BYTES,
                lights.as_ptr(),
                lights.len(),
            )
        };
        Some(Self {
            handle: NonNull::new(handle)?,
            opaque_triangles,
            light_count,
        })
    }

    pub fn render(&mut self, mut params: Params, pixels: &mut [Pixel]) -> bool {
        if params.width == 0
            || params.rows == 0
            || params.samples == 0
            || params.first_row.saturating_add(params.rows) > params.height
            || (params.width as usize).checked_mul(params.rows as usize) != Some(pixels.len())
        {
            return false;
        }
        params.opaque_triangles = self.opaque_triangles;
        params.light_count = self.light_count;
        // The bridge waits for completion and copies exactly width * rows pixels.
        unsafe { newera_metal_render(self.handle.as_ptr(), &params, pixels.as_mut_ptr()) }
    }
}

impl Drop for Scene {
    fn drop(&mut self) {
        // The owned bridge handle is destroyed exactly once.
        unsafe { newera_metal_destroy(self.handle.as_ptr()) }
    }
}

#[derive(Debug)]
pub struct Video {
    handle: NonNull<c_void>,
    index_count: usize,
    clear_count: usize,
    vertex_count: usize,
    bytes: usize,
}

unsafe extern "C" {
    fn newera_video_create(
        source: *const u8,
        source_len: usize,
        vertices: *const u8,
        vertex_bytes: usize,
        indices: *const u32,
        index_count: usize,
        clear_count: usize,
        textures: *const u8,
        layers: usize,
        width: usize,
        height: usize,
    ) -> *mut c_void;
    fn newera_video_render(
        handle: *mut c_void,
        view: *const f32,
        sky: *const f32,
        clear: *const u32,
        index_count: usize,
        clear_count: usize,
        pixels: *mut u8,
    ) -> bool;
    fn newera_video_destroy(handle: *mut c_void);
}

impl Video {
    pub fn new(
        shader: &str,
        vertices: &[u8],
        opaque: &[u32],
        clear_count: usize,
        textures: &[u8],
        size: (u32, u32),
    ) -> Option<Self> {
        let (width, height) = (size.0 as usize, size.1 as usize);
        let bytes = width.checked_mul(height)?.checked_mul(4)?;
        let vertex_count = vertices.len() / VERTEX_BYTES;
        if bytes == 0
            || width > 8192
            || height > 8192
            || bytes > 2_073_600 * 4
            || !vertices.len().is_multiple_of(VERTEX_BYTES)
            || opaque.is_empty()
            || !opaque.len().is_multiple_of(3)
            || !clear_count.is_multiple_of(3)
            || !textures.len().is_multiple_of(TEXTURE_BYTES)
            || opaque.iter().any(|&i| i as usize >= vertex_count)
        {
            return None;
        }
        // The bridge copies the provided data during this call.
        let handle = unsafe {
            newera_video_create(
                shader.as_ptr(),
                shader.len(),
                vertices.as_ptr(),
                vertices.len(),
                opaque.as_ptr(),
                opaque.len(),
                clear_count,
                textures.as_ptr(),
                textures.len() / TEXTURE_BYTES,
                width,
                height,
            )
        };
        Some(Self {
            handle: NonNull::new(handle)?,
            index_count: opaque.len(),
            clear_count,
            vertex_count,
            bytes,
        })
    }

    pub fn frame(
        &mut self,
        view: &[f32; 16],
        sky: &[f32; 3],
        clear: &[u32],
        pixels: &mut [u8],
    ) -> bool {
        if pixels.len() != self.bytes
            || clear.len() != self.clear_count
            || clear.iter().any(|&i| i as usize >= self.vertex_count)
        {
            return false;
        }
        // The bridge waits for GPU completion and copies exactly self.bytes bytes.
        unsafe {
            newera_video_render(
                self.handle.as_ptr(),
                view.as_ptr(),
                sky.as_ptr(),
                clear.as_ptr(),
                self.index_count,
                self.clear_count,
                pixels.as_mut_ptr(),
            )
        }
    }
}

impl Drop for Video {
    fn drop(&mut self) {
        // The owned bridge handle is destroyed exactly once.
        unsafe { newera_video_destroy(self.handle.as_ptr()) }
    }
}
