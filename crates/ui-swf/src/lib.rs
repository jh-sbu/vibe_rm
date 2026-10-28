//! Skyrim's Interface menus (Scaleform `.swf` movies, ActionScript 2) through
//! the vendored Ruffle (`vendor/ruffle`, with Scaleform changes).
//!
//! `UiSwf` holds what all menus share: Ruffle's GPU state on the engine's
//! device, the Interface files, the fonts (`fontconfig.txt`) and the
//! translations. `UiSwf::open` starts a `Menu`, which draws into a texture of
//! its own; `Compositor` draws those over the frame. The game's side of a menu
//! (what to call in it, what its calls mean) lives with the engine.
//!
//! What each menu needed, and how this was worked out, is in
//! `spike/swf/README.md`.

mod compositor;
mod files;
mod fonts;
pub mod input;
mod menu;
mod navigator;

use std::sync::Arc;

use ruffle_render_wgpu::descriptors::Descriptors;

pub use compositor::Compositor;
pub use files::InterfaceFiles;
pub use menu::{GameCall, Menu, Movie, Responder};
pub use ruffle_core::events::{MouseButton, MouseWheelDelta, PlayerEvent, TextControlCode};
pub use ruffle_core::external::Value;

pub(crate) type HostImages = Arc<std::sync::Mutex<std::collections::HashMap<String, Vec<u8>>>>;

pub struct UiSwf {
    descriptors: Arc<Descriptors>,
    images: HostImages,
    files: Arc<InterfaceFiles>,
    fonts: fonts::Fonts,
    translations: Vec<(String, String)>,
    pub compositor: Compositor,
}

impl UiSwf {
    /// Ruffle on the engine's GPU device, drawing over frames in `format`, with
    /// the Interface files from `vfs` and the translations of `language`
    /// (`english`).
    pub fn new(
        instance: wgpu::Instance,
        adapter: wgpu::Adapter,
        device: wgpu::Device,
        queue: wgpu::Queue,
        format: wgpu::TextureFormat,
        vfs: &vfs::Vfs,
        language: &str,
    ) -> anyhow::Result<Self> {
        let compositor = Compositor::new(&device, format);
        let descriptors = Arc::new(Descriptors::new(instance, adapter, device, queue));
        let files = InterfaceFiles::load(vfs);
        let fonts = fonts::Fonts::load(&files)?;
        let translations = fonts::translations(&files, language);
        Ok(UiSwf {
            descriptors,
            images: Default::default(),
            files: Arc::new(files),
            fonts,
            translations,
            compositor,
        })
    }

    /// Register a host image, shared by all menus on this UI.
    pub fn set_image(
        &self,
        name: &str,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            width > 0 && height > 0 && width as usize * height as usize * 4 == rgba.len(),
            "invalid host image"
        );
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, width, height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.write_header()?.write_image_data(rgba)?;
        }
        self.images.lock().unwrap().insert(name.into(), bytes);
        Ok(())
    }

    /// Starts a menu movie (`hudmenu.swf`) the size of the frame.
    pub fn open(&self, name: &str, size: (u32, u32)) -> anyhow::Result<Menu> {
        Menu::open(self, name, size)
    }

    /// Draws the menus' textures, in order, over `target`.
    pub fn composite(&self, target: &wgpu::TextureView, menus: &[&Menu]) {
        let textures: Vec<wgpu::Texture> = menus.iter().map(|m| m.texture()).collect();
        self.compositor.draw(
            &self.descriptors.device,
            &self.descriptors.queue,
            target,
            &textures,
        );
    }
}
