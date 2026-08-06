//! SVG loader fallback for resources whose URL does not end in `.svg`.
//!
//! egui_extras 0.33 documents MIME-based SVG detection, but its built-in
//! loader currently accepts only URIs ending in `.svg`. Dynamic image URLs
//! commonly omit that suffix while returning `image/svg+xml` correctly.

use std::{collections::HashMap, mem::size_of, sync::Arc};

use egui::{
    load::{BytesPoll, ImageLoadResult, ImageLoader, ImagePoll, LoadError, SizeHint},
    mutex::Mutex,
    ColorImage,
};
#[cfg(feature = "svg_text")]
use resvg::usvg::fontdb::{Database, Family};

struct MimeSvgLoader {
    cache: Mutex<HashMap<(String, SizeHint), Result<Arc<ColorImage>, String>>>,
    options: resvg::usvg::Options<'static>,
}

impl MimeSvgLoader {
    const ID: &'static str = egui::generate_loader_id!(MimeSvgLoader);
}

impl Default for MimeSvgLoader {
    fn default() -> Self {
        let options = resvg::usvg::Options::default();
        #[cfg(feature = "svg_text")]
        let options = {
            let mut options = options;
            options.fontdb_mut().load_system_fonts();
            repair_generic_font_families(options.fontdb_mut());
            options
        };
        Self {
            cache: Mutex::default(),
            options,
        }
    }
}

/// fontdb reads generic-family aliases from fontconfig configuration files,
/// but an alias may name a font that is not installed (for example FreeSans on
/// a minimal Linux desktop). Validate the aliases and, when necessary, ask the
/// system's fontconfig matcher for a concrete installed family. The final scan
/// keeps SVG text working on systems without the `fc-match` utility.
#[cfg(feature = "svg_text")]
fn repair_generic_font_families(database: &mut Database) {
    let serif = resolve_generic_family(
        database,
        database.family_name(&Family::Serif),
        "serif",
        "serif",
        false,
    );
    let sans_serif = resolve_generic_family(
        database,
        database.family_name(&Family::SansSerif),
        "sans-serif",
        "sans",
        false,
    );
    let monospace = resolve_generic_family(
        database,
        database.family_name(&Family::Monospace),
        "monospace",
        "mono",
        true,
    );

    if let Some(family) = serif {
        database.set_serif_family(family);
    }
    if let Some(family) = sans_serif {
        database.set_sans_serif_family(family);
    }
    if let Some(family) = monospace {
        database.set_monospace_family(family);
    }
}

#[cfg(feature = "svg_text")]
fn resolve_generic_family(
    database: &Database,
    configured: &str,
    fontconfig_pattern: &str,
    preferred_name_fragment: &str,
    monospaced: bool,
) -> Option<String> {
    canonical_family_name(database, configured)
        .or_else(|| {
            fontconfig_family(fontconfig_pattern)
                .and_then(|family| canonical_family_name(database, &family))
        })
        .or_else(|| fallback_family_name(database, preferred_name_fragment, monospaced))
}

#[cfg(feature = "svg_text")]
fn canonical_family_name(database: &Database, requested: &str) -> Option<String> {
    database.faces().find_map(|face| {
        face.families
            .iter()
            .find(|(family, _)| family.eq_ignore_ascii_case(requested))
            .map(|(family, _)| family.clone())
    })
}

#[cfg(feature = "svg_text")]
fn fallback_family_name(
    database: &Database,
    preferred_name_fragment: &str,
    monospaced: bool,
) -> Option<String> {
    let matching_width: Vec<_> = database
        .faces()
        .filter(|face| face.monospaced == monospaced)
        .collect();

    matching_width
        .iter()
        .flat_map(|face| &face.families)
        .find(|(family, _)| family.to_lowercase().contains(preferred_name_fragment))
        .or_else(|| matching_width.iter().flat_map(|face| &face.families).next())
        .map(|(family, _)| family.clone())
}

#[cfg(feature = "svg_text")]
fn fontconfig_family(pattern: &str) -> Option<String> {
    #[cfg(all(
        unix,
        not(any(target_os = "macos", target_os = "ios", target_os = "android"))
    ))]
    {
        use std::process::{Command, Stdio};

        let output = Command::new("fc-match")
            .arg("--format=%{family}\\n")
            .arg(pattern)
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .ok()?;
        if output.status.success() {
            return parse_fontconfig_family(&output.stdout);
        }
    }

    let _ = pattern;
    None
}

#[cfg(feature = "svg_text")]
fn parse_fontconfig_family(stdout: &[u8]) -> Option<String> {
    String::from_utf8_lossy(stdout)
        .lines()
        .next()?
        .split(',')
        .map(str::trim)
        .find(|family| !family.is_empty())
        .map(str::to_owned)
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

        let key = (uri.to_owned(), size_hint);
        if let Some(result) = self.cache.lock().get(&key).cloned() {
            return result
                .map(|image| ImagePoll::Ready { image })
                .map_err(LoadError::Loading);
        }

        match ctx.try_load_bytes(uri)? {
            BytesPoll::Pending { size } => Ok(ImagePoll::Pending { size }),
            BytesPoll::Ready { bytes, mime, .. } if mime.as_deref().is_some_and(is_svg_mime) => {
                let result =
                    egui_extras::image::load_svg_bytes_with_size(&bytes, size_hint, &self.options)
                        .map(Arc::new);
                self.cache.lock().insert(key, result.clone());
                result
                    .map(|image| ImagePoll::Ready { image })
                    .map_err(LoadError::Loading)
            }
            BytesPoll::Ready { .. } => Err(LoadError::NotSupported),
        }
    }

    fn forget(&self, uri: &str) {
        self.cache
            .lock()
            .retain(|(cached_uri, _), _| cached_uri != uri);
    }

    fn forget_all(&self) {
        self.cache.lock().clear();
    }

    fn byte_size(&self) -> usize {
        self.cache
            .lock()
            .values()
            .map(|result| match result {
                Ok(image) => image.pixels.len() * size_of::<egui::Color32>(),
                Err(error) => error.len(),
            })
            .sum()
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
    use std::sync::Arc;

    use egui::{
        load::{ImageLoader, SizeHint},
        Color32, ColorImage,
    };

    #[cfg(feature = "svg_text")]
    use super::parse_fontconfig_family;
    use super::{is_svg_mime, MimeSvgLoader};

    #[test]
    fn recognizes_svg_mime_with_optional_parameters() {
        assert!(is_svg_mime("image/svg+xml"));
        assert!(is_svg_mime("image/svg+xml;charset=utf-8"));
        assert!(is_svg_mime("IMAGE/SVG+XML; charset=UTF-8"));
        assert!(!is_svg_mime("image/png"));
    }

    #[cfg(feature = "svg_text")]
    #[test]
    fn parses_first_concrete_fontconfig_family() {
        assert_eq!(
            parse_fontconfig_family(b"DejaVu Sans,DejaVu Sans Condensed\n"),
            Some("DejaVu Sans".to_string())
        );
        assert_eq!(parse_fontconfig_family(b"\n"), None);
        assert_eq!(parse_fontconfig_family(b""), None);
    }

    #[test]
    fn forget_removes_every_cached_size_for_uri() {
        let loader = MimeSvgLoader {
            cache: Default::default(),
            options: resvg::usvg::Options::default(),
        };
        let image = Arc::new(ColorImage::filled([1, 1], Color32::WHITE));

        loader.cache.lock().insert(
            ("https://example.com/badge".to_owned(), SizeHint::Width(10)),
            Ok(image.clone()),
        );
        loader.cache.lock().insert(
            ("https://example.com/badge".to_owned(), SizeHint::Width(20)),
            Ok(image.clone()),
        );
        loader.cache.lock().insert(
            ("https://example.com/other".to_owned(), SizeHint::Width(10)),
            Ok(image),
        );

        loader.forget("https://example.com/badge");

        let cache = loader.cache.lock();
        assert_eq!(cache.len(), 1);
        assert!(cache
            .keys()
            .all(|(uri, _)| uri == "https://example.com/other"));
    }
}
