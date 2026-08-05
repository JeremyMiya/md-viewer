//! SVG loader fallback for resources whose URL does not end in `.svg`.
//!
//! egui_extras 0.33 documents MIME-based SVG detection, but its built-in
//! loader currently accepts only URIs ending in `.svg`. Dynamic image URLs
//! commonly omit that suffix while returning `image/svg+xml` correctly.

use std::sync::Arc;

use egui::load::{BytesPoll, ImageLoadResult, ImageLoader, ImagePoll, LoadError, SizeHint};

struct MimeSvgLoader {
    options: resvg::usvg::Options<'static>,
}

impl MimeSvgLoader {
    const ID: &'static str = egui::generate_loader_id!(MimeSvgLoader);
}

impl Default for MimeSvgLoader {
    fn default() -> Self {
        let mut options = resvg::usvg::Options::default();
        #[cfg(feature = "svg_text")]
        options.fontdb_mut().load_system_fonts();
        Self { options }
    }
}

impl ImageLoader for MimeSvgLoader {
    fn id(&self) -> &str {
        Self::ID
    }

    fn load(&self, ctx: &egui::Context, uri: &str, size_hint: SizeHint) -> ImageLoadResult {
        // Let egui_extras' built-in loader handle ordinary `.svg` URIs. This
        // fallback is deliberately limited to the MIME-detection gap.
        if uri.ends_with(".svg") {
            return Err(LoadError::NotSupported);
        }

        match ctx.try_load_bytes(uri)? {
            BytesPoll::Pending { size } => Ok(ImagePoll::Pending { size }),
            BytesPoll::Ready { bytes, mime, .. } if mime.as_deref().is_some_and(is_svg_mime) => {
                egui_extras::image::load_svg_bytes_with_size(&bytes, size_hint, &self.options)
                    .map(Arc::new)
                    .map(|image| ImagePoll::Ready { image })
                    .map_err(LoadError::Loading)
            }
            BytesPoll::Ready { .. } => Err(LoadError::NotSupported),
        }
    }

    fn forget(&self, _uri: &str) {}

    fn forget_all(&self) {}

    fn byte_size(&self) -> usize {
        // Decoded textures are cached by egui's DefaultTextureLoader.
        0
    }
}

fn is_svg_mime(mime: &str) -> bool {
    mime.split(';')
        .next()
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("image/svg+xml"))
}

pub(crate) fn install(ctx: &egui::Context) {
    if !ctx.is_loader_installed(MimeSvgLoader::ID) {
        // Image loaders are tried newest-first, so install this after the
        // standard egui_extras loaders initialized by prepare_show().
        ctx.add_image_loader(Arc::new(MimeSvgLoader::default()));
    }
}

#[cfg(test)]
mod tests {
    use super::is_svg_mime;

    #[test]
    fn recognizes_svg_mime_with_optional_parameters() {
        assert!(is_svg_mime("image/svg+xml"));
        assert!(is_svg_mime("image/svg+xml;charset=utf-8"));
        assert!(is_svg_mime("IMAGE/SVG+XML; charset=UTF-8"));
        assert!(!is_svg_mime("image/png"));
    }
}
