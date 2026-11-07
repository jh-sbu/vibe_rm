//! GPU textures and a path-keyed cache.

use std::collections::HashMap;
use std::sync::Arc;

use super::dds;

pub struct GpuTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    /// How it's viewed: a cube map (a cube DDS with all six faces), an
    /// array or a plain 2D texture.
    pub dimension: wgpu::TextureViewDimension,
}

pub fn upload_dds(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    d: &dds::Dds,
    label: &str,
) -> GpuTexture {
    let (bw, _) = d.format.block();
    // Block-compressed textures must have block-aligned base dimensions;
    // drop mips that would violate that by skipping leading levels if needed.
    let mut first = 0;
    while bw > 1
        && (d.mip_size(first).0 % bw != 0 || d.mip_size(first).1 % bw != 0)
        && first + 1 < d.mips
    {
        first += 1;
    }
    let (w, h) = d.mip_size(first);
    let mut mips = d.mips - first;
    // Mips smaller than a block are fine for wgpu, but each mip must exist.
    if bw > 1 {
        // wgpu requires every mip's *physical* size to be valid, which always holds.
        mips = mips.max(1);
    }
    let layers = d.layers;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: layers,
        },
        mip_level_count: mips,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: d.format.wgpu(),
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let (b, bytes) = d.format.block();
    let mut offset = 0usize;
    for layer in 0..layers {
        for level in 0..d.mips {
            let size = d.mip_bytes(level);
            if level >= first {
                let (mw, mh) = d.mip_size(level);
                let data = &d.data[offset..offset + size];
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &texture,
                        mip_level: level - first,
                        origin: wgpu::Origin3d {
                            x: 0,
                            y: 0,
                            z: layer,
                        },
                        aspect: wgpu::TextureAspect::All,
                    },
                    data,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(mw.div_ceil(b) * bytes),
                        rows_per_image: Some(mh.div_ceil(b)),
                    },
                    wgpu::Extent3d {
                        width: mw.div_ceil(b) * b,
                        height: mh.div_ceil(b) * b,
                        depth_or_array_layers: 1,
                    },
                );
            }
            offset += size;
        }
    }
    let dimension = if d.cube && layers == 6 {
        wgpu::TextureViewDimension::Cube
    } else if layers > 1 {
        wgpu::TextureViewDimension::D2Array
    } else {
        wgpu::TextureViewDimension::D2
    };
    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(dimension),
        ..Default::default()
    });
    GpuTexture {
        texture,
        view,
        dimension,
    }
}

pub fn solid(device: &wgpu::Device, queue: &wgpu::Queue, rgba: [u8; 4], label: &str) -> GpuTexture {
    let d = dds::Dds {
        width: 1,
        height: 1,
        mips: 1,
        layers: 1,
        cube: false,
        format: dds::Format::Rgba8,
        data: rgba.to_vec(),
    };
    upload_dds(device, queue, &d, label)
}

/// A 1x1 cube map of one colour on every face.
pub fn solid_cube(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    rgba: [u8; 4],
    label: &str,
) -> GpuTexture {
    let d = dds::Dds {
        width: 1,
        height: 1,
        mips: 1,
        layers: 6,
        cube: true,
        format: dds::Format::Rgba8,
        data: rgba.repeat(6),
    };
    upload_dds(device, queue, &d, label)
}

/// What the cache holds for a path.
enum Slot {
    /// Loaded, or `None`: missing or unreadable.
    Ready(Option<Arc<GpuTexture>>),
    /// Being read, parsed and uploaded in the background.
    Pending,
}

/// What [`TextureCache::peek`] finds, without waiting.
pub enum Peek {
    Unknown,
    Pending,
    Ready(Option<Arc<GpuTexture>>),
}

/// A background job's result: the parsed texture (`None`: missing or
/// unreadable), uploaded by the main thread.
pub(crate) type Parsed = (String, Option<dds::Dds>);

/// Textures by path. Paths can be pending while a background job reads and
/// parses them (see `Renderer::request_textures`); the main thread uploads
/// them, a few each frame ([`TextureCache::upload`]). [`TextureCache::get`]
/// waits for a pending one, so a caller that loads textures itself never
/// keeps a placeholder.
pub struct TextureCache {
    map: std::sync::Mutex<HashMap<String, Slot>>,
    done_tx: std::sync::mpsc::Sender<Parsed>,
    done_rx: std::sync::Mutex<std::sync::mpsc::Receiver<Parsed>>,
    /// Parsed, waiting to be uploaded.
    parsed: std::sync::Mutex<std::collections::VecDeque<Parsed>>,
    /// Where pending textures are uploaded (set with background loading).
    gpu: Option<(wgpu::Device, wgpu::Queue)>,
}

impl Default for TextureCache {
    fn default() -> Self {
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        TextureCache {
            map: Default::default(),
            done_tx,
            done_rx: std::sync::Mutex::new(done_rx),
            parsed: Default::default(),
            gpu: None,
        }
    }
}

impl TextureCache {
    /// The texture at `path`: `None` if it was never requested, `Some(None)`
    /// if it's missing. Waits for one still loading, and uploads it.
    pub fn get(&self, path: &str) -> Option<Option<Arc<GpuTexture>>> {
        loop {
            match self.peek(path) {
                Peek::Unknown => return None,
                Peek::Ready(t) => return Some(t),
                Peek::Pending => {
                    self.receive(false);
                    if !self.upload_one(path) {
                        self.receive(true);
                    }
                }
            }
        }
    }
    /// The texture at `path` as it stands, never waiting.
    pub fn peek(&self, path: &str) -> Peek {
        match self.map.lock().unwrap().get(path) {
            None => Peek::Unknown,
            Some(Slot::Pending) => Peek::Pending,
            Some(Slot::Ready(t)) => Peek::Ready(t.clone()),
        }
    }
    /// Whether `path` was requested (loaded, missing or pending).
    pub fn contains(&self, path: &str) -> bool {
        self.map.lock().unwrap().contains_key(path)
    }
    pub fn insert(&mut self, path: String, t: Option<Arc<GpuTexture>>) {
        self.map.get_mut().unwrap().insert(path, Slot::Ready(t));
    }
    pub fn len(&self) -> usize {
        self.map.lock().unwrap().len()
    }
    /// Upload pending textures with this device and queue.
    pub(crate) fn set_gpu(&mut self, device: wgpu::Device, queue: wgpu::Queue) {
        self.gpu = Some((device, queue));
    }
    /// Mark `path` pending; false if it's already known.
    pub(crate) fn mark_pending(&mut self, path: &str) -> bool {
        let map = self.map.get_mut().unwrap();
        if map.contains_key(path) {
            return false;
        }
        map.insert(path.to_owned(), Slot::Pending);
        true
    }
    /// Where background jobs report a parsed texture.
    pub(crate) fn sender(&self) -> std::sync::mpsc::Sender<Parsed> {
        self.done_tx.clone()
    }
    /// Whether any texture is still loading.
    pub fn any_pending(&self) -> bool {
        self.map
            .lock()
            .unwrap()
            .values()
            .any(|s| matches!(s, Slot::Pending))
    }
    /// Take in parsed textures: those already done, or (`wait`) at least
    /// one, waiting for it.
    pub(crate) fn receive(&self, wait: bool) {
        let rx = self.done_rx.lock().unwrap();
        let mut parsed = self.parsed.lock().unwrap();
        if wait && let Ok(done) = rx.recv() {
            parsed.push_back(done);
        }
        parsed.extend(rx.try_iter());
    }
    /// Upload parsed textures in arrival order until `budget` has passed
    /// (`None`: all). True if any is left.
    pub(crate) fn upload(&self, budget: Option<std::time::Duration>) -> bool {
        let start = std::time::Instant::now();
        loop {
            if budget.is_some_and(|b| start.elapsed() >= b) {
                return !self.parsed.lock().unwrap().is_empty();
            }
            let Some((path, d)) = self.parsed.lock().unwrap().pop_front() else {
                return false;
            };
            self.finish(path, d);
        }
    }
    /// Upload `path` now if it has been parsed; false if it hasn't yet.
    fn upload_one(&self, path: &str) -> bool {
        let mut parsed = self.parsed.lock().unwrap();
        let Some(i) = parsed.iter().position(|(p, _)| p == path) else {
            return false;
        };
        let (path, d) = parsed.remove(i).unwrap();
        drop(parsed);
        self.finish(path, d);
        true
    }
    fn finish(&self, path: String, d: Option<dds::Dds>) {
        let t = d
            .zip(self.gpu.as_ref())
            .map(|(d, (device, queue))| Arc::new(upload_dds(device, queue, &d, &path)));
        self.map.lock().unwrap().insert(path, Slot::Ready(t));
    }
    /// Wait until no texture is loading, uploading them all.
    pub fn wait_all(&self) {
        loop {
            self.receive(false);
            self.upload(None);
            if !self.any_pending() {
                return;
            }
            self.receive(true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_textures_are_known_and_waited_for() {
        let mut cache = TextureCache::default();
        assert!(cache.mark_pending("a.dds"));
        assert!(!cache.mark_pending("a.dds"));
        assert!(cache.contains("a.dds"));
        assert!(cache.any_pending());
        assert!(matches!(cache.peek("a.dds"), Peek::Pending));
        cache.receive(false);
        assert!(!cache.upload(None));
        let done = cache.sender();
        std::thread::spawn(move || done.send(("a.dds".into(), None)).unwrap());
        // Waits for the job, then finds it missing.
        assert!(matches!(cache.get("a.dds"), Some(None)));
        assert!(!cache.any_pending());
        assert!(cache.get("b.dds").is_none());
        cache.wait_all();
    }
}
