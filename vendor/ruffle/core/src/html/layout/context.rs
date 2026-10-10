use crate::character::Character;
use crate::context::{RenderContext, UpdateContext};
use crate::font::{DefaultFont, Font, FontType};
use crate::string::WStr;
use gc_arena::Mutation;
use ruffle_common::tag_utils::SwfMovie;
use ruffle_render::bitmap::BitmapHandle;
use std::sync::Arc;

/// Provides access to the font resolution operations needed to lay out text,
/// common to both `UpdateContext` and `RenderContext`.
pub trait LayoutContext<'gc> {
    /// Retrieve the current GC context.
    fn gc(&self) -> &'gc Mutation<'gc>;

    /// Find an embedded font by its name and parameters.
    fn get_embedded_font_by_name(
        &self,
        name: &str,
        font_type: FontType,
        is_bold: bool,
        is_italic: bool,
        movie: Option<Arc<SwfMovie>>,
    ) -> Option<Font<'gc>>;

    /// Returns the default font implementations behind a built-in name.
    fn default_font(&mut self, name: DefaultFont, is_bold: bool, is_italic: bool)
    -> Vec<Font<'gc>>;

    /// Returns the device font(s) best matching the given name and parameters.
    fn get_or_sort_device_fonts(
        &mut self,
        name: &str,
        is_bold: bool,
        is_italic: bool,
    ) -> Vec<Font<'gc>>;

    /// A bitmap the movie exports under `name` (an HTML `<img src>` naming a
    /// library symbol, as Scaleform allows), with its width and height.
    fn library_bitmap(
        &mut self,
        name: &WStr,
        movie: &Arc<SwfMovie>,
    ) -> Option<(BitmapHandle, u32, u32)>;
}

impl<'gc> LayoutContext<'gc> for UpdateContext<'gc> {
    #[inline(always)]
    fn gc(&self) -> &'gc Mutation<'gc> {
        self.gc_context
    }

    fn get_embedded_font_by_name(
        &self,
        name: &str,
        font_type: FontType,
        is_bold: bool,
        is_italic: bool,
        movie: Option<Arc<SwfMovie>>,
    ) -> Option<Font<'gc>> {
        self.library
            .get_embedded_font_by_name(name, font_type, is_bold, is_italic, movie)
    }

    fn default_font(
        &mut self,
        name: DefaultFont,
        is_bold: bool,
        is_italic: bool,
    ) -> Vec<Font<'gc>> {
        self.library.default_font(
            name,
            is_bold,
            is_italic,
            self.ui,
            self.renderer,
            self.gc_context,
        )
    }

    fn get_or_sort_device_fonts(
        &mut self,
        name: &str,
        is_bold: bool,
        is_italic: bool,
    ) -> Vec<Font<'gc>> {
        self.library.get_or_sort_device_fonts(
            name,
            is_bold,
            is_italic,
            self.ui,
            self.renderer,
            self.gc_context,
        )
    }

    fn library_bitmap(
        &mut self,
        name: &WStr,
        movie: &Arc<SwfMovie>,
    ) -> Option<(BitmapHandle, u32, u32)> {
        let (_, Character::Bitmap(bitmap)) = self
            .library
            .library_for_movie(movie.clone())?
            .character_by_export_name(name)?
        else {
            return None;
        };
        let size = bitmap.compressed().size();
        let handle = bitmap.bitmap_handle(self.renderer).ok()?;
        Some((handle, size.width, size.height))
    }
}

impl<'gc> LayoutContext<'gc> for RenderContext<'_, 'gc> {
    #[inline(always)]
    fn gc(&self) -> &'gc Mutation<'gc> {
        self.gc_context
    }

    fn get_embedded_font_by_name(
        &self,
        name: &str,
        font_type: FontType,
        is_bold: bool,
        is_italic: bool,
        movie: Option<Arc<SwfMovie>>,
    ) -> Option<Font<'gc>> {
        self.library
            .get_embedded_font_by_name(name, font_type, is_bold, is_italic, movie)
    }

    fn default_font(
        &mut self,
        name: DefaultFont,
        is_bold: bool,
        is_italic: bool,
    ) -> Vec<Font<'gc>> {
        self.library.default_font(
            name,
            is_bold,
            is_italic,
            self.ui,
            self.renderer,
            self.gc_context,
        )
    }

    fn get_or_sort_device_fonts(
        &mut self,
        name: &str,
        is_bold: bool,
        is_italic: bool,
    ) -> Vec<Font<'gc>> {
        self.library.get_or_sort_device_fonts(
            name,
            is_bold,
            is_italic,
            self.ui,
            self.renderer,
            self.gc_context,
        )
    }

    fn library_bitmap(
        &mut self,
        name: &WStr,
        movie: &Arc<SwfMovie>,
    ) -> Option<(BitmapHandle, u32, u32)> {
        let (_, Character::Bitmap(bitmap)) = self
            .library
            .library_for_movie(movie.clone())?
            .character_by_export_name(name)?
        else {
            return None;
        };
        let size = bitmap.compressed().size();
        let handle = bitmap.bitmap_handle(self.renderer).ok()?;
        Some((handle, size.width, size.height))
    }
}
