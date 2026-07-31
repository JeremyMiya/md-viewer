use std::collections::{HashMap, HashSet};

use egui::{FontData, FontDefinitions, FontFamily};
use egui_commonmark_extended::STRONG_FONT_FAMILY;
use fontdb::{Database, Family, Query, Source, Style, Weight, ID};

struct FontSpec {
    key: &'static str,
    families: &'static [&'static str],
    required_glyphs: &'static str,
    primary: bool,
    cjk_region: Option<CjkRegion>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CjkRegion {
    SimplifiedChinese,
    TraditionalChinese,
    Japanese,
    Korean,
}

struct StrongFontSpec {
    key: &'static str,
    matching_regular_key: &'static str,
    required_glyphs: &'static str,
    primary: bool,
}

const PRIMARY_SANS_FAMILIES: &[&str] = &[
    "Noto Sans",
    "DejaVu Sans",
    "Liberation Sans",
    "Ubuntu",
    "Cantarell",
    "Inter",
    "Segoe UI",
    "Arial",
    "Helvetica Neue",
    "Helvetica",
];

// Keep regional CJK faces separate. Pan-CJK fonts often contain all of these
// scripts, but their glyph variants are still region-specific.
const CJK_SC_FAMILIES: &[&str] = &[
    "Noto Sans CJK SC",
    "Noto Sans SC",
    "Source Han Sans SC",
    "Microsoft YaHei",
    "Microsoft YaHei UI",
    "DengXian",
    "SimSun",
    "NSimSun",
    "PingFang SC",
    "Droid Sans Fallback",
];

const CJK_TC_FAMILIES: &[&str] = &[
    "Noto Sans CJK TC",
    "Noto Sans TC",
    "Noto Sans CJK HK",
    "Noto Sans HK",
    "Source Han Sans TC",
    "Source Han Sans HC",
    "Microsoft JhengHei",
    "PingFang TC",
    "PingFang HK",
];

const CJK_JP_FAMILIES: &[&str] = &[
    "Noto Sans CJK JP",
    "Noto Sans JP",
    "Source Han Sans JP",
    "Yu Gothic",
    "Hiragino Sans",
];

const CJK_KR_FAMILIES: &[&str] = &[
    "Noto Sans CJK KR",
    "Noto Sans KR",
    "Source Han Sans KR",
    "Malgun Gothic",
    "Apple SD Gothic Neo",
];

const REGULAR_FONT_SPECS: &[FontSpec] = &[
    FontSpec {
        key: "SystemSans",
        families: PRIMARY_SANS_FAMILIES,
        required_glyphs: "Aa",
        primary: true,
        cjk_region: None,
    },
    FontSpec {
        key: "SystemCjkScSans",
        families: CJK_SC_FAMILIES,
        required_glyphs: "中文",
        primary: false,
        cjk_region: Some(CjkRegion::SimplifiedChinese),
    },
    FontSpec {
        key: "SystemCjkTcSans",
        families: CJK_TC_FAMILIES,
        required_glyphs: "繁體",
        primary: false,
        cjk_region: Some(CjkRegion::TraditionalChinese),
    },
    FontSpec {
        key: "SystemCjkJpSans",
        families: CJK_JP_FAMILIES,
        required_glyphs: "かなカナ",
        primary: false,
        cjk_region: Some(CjkRegion::Japanese),
    },
    FontSpec {
        key: "SystemCjkKrSans",
        families: CJK_KR_FAMILIES,
        required_glyphs: "한글",
        primary: false,
        cjk_region: Some(CjkRegion::Korean),
    },
    FontSpec {
        key: "SystemArabicSans",
        families: &["Noto Sans Arabic"],
        required_glyphs: "اب",
        primary: false,
        cjk_region: None,
    },
    FontSpec {
        key: "SystemHebrewSans",
        families: &["Noto Sans Hebrew"],
        required_glyphs: "אב",
        primary: false,
        cjk_region: None,
    },
    FontSpec {
        key: "SystemDevanagariSans",
        families: &["Noto Sans Devanagari"],
        required_glyphs: "अक",
        primary: false,
        cjk_region: None,
    },
    FontSpec {
        key: "SystemThaiSans",
        families: &["Noto Sans Thai"],
        required_glyphs: "กข",
        primary: false,
        cjk_region: None,
    },
    FontSpec {
        key: "SystemSymbols",
        families: &["Noto Sans Symbols"],
        required_glyphs: "→",
        primary: false,
        cjk_region: None,
    },
    FontSpec {
        key: "SystemSymbols2",
        families: &["Noto Sans Symbols 2", "Noto Sans Symbols2"],
        required_glyphs: "⚠",
        primary: false,
        cjk_region: None,
    },
    FontSpec {
        key: "SystemDejaVuSans",
        families: &["DejaVu Sans"],
        required_glyphs: "⚠",
        primary: false,
        cjk_region: None,
    },
];

const STRONG_FONT_SPECS: &[StrongFontSpec] = &[
    StrongFontSpec {
        key: "SystemSansBold",
        matching_regular_key: "SystemSans",
        required_glyphs: "Aa",
        primary: true,
    },
    StrongFontSpec {
        key: "SystemCjkScSansBold",
        matching_regular_key: "SystemCjkScSans",
        required_glyphs: "中文",
        primary: false,
    },
    StrongFontSpec {
        key: "SystemCjkTcSansBold",
        matching_regular_key: "SystemCjkTcSans",
        required_glyphs: "繁體",
        primary: false,
    },
    StrongFontSpec {
        key: "SystemCjkJpSansBold",
        matching_regular_key: "SystemCjkJpSans",
        required_glyphs: "かなカナ",
        primary: false,
    },
    StrongFontSpec {
        key: "SystemCjkKrSansBold",
        matching_regular_key: "SystemCjkKrSans",
        required_glyphs: "한글",
        primary: false,
    },
];

struct ResolvedFont {
    id: ID,
    data: FontData,
    family: String,
    source: String,
    weight: Weight,
}

struct InstalledRegularFonts {
    face_ids: HashSet<ID>,
    keys: Vec<String>,
    families_by_key: HashMap<String, String>,
    primary_key: Option<String>,
}

fn find_font_face_with(
    database: &Database,
    families: &[&str],
    weight: Weight,
    mut supports_required_glyphs: impl FnMut(ID) -> bool,
) -> Option<ID> {
    for family in families {
        let family = [Family::Name(family)];
        let Some(id) = database.query(&Query {
            families: &family,
            weight,
            style: Style::Normal,
            ..Query::default()
        }) else {
            continue;
        };
        if supports_required_glyphs(id) {
            return Some(id);
        }
    }

    None
}

fn font_has_glyphs(database: &Database, id: ID, required_glyphs: &str) -> bool {
    database
        .with_face_data(id, |bytes, face_index| {
            ttf_parser::Face::parse(bytes, face_index)
                .map(|face| {
                    required_glyphs
                        .chars()
                        .all(|character| face.glyph_index(character).is_some())
                })
                .unwrap_or(false)
        })
        .unwrap_or(false)
}

fn find_font_face(
    database: &Database,
    families: &[&str],
    weight: Weight,
    required_glyphs: &str,
) -> Option<ID> {
    find_font_face_with(database, families, weight, |id| {
        font_has_glyphs(database, id, required_glyphs)
    })
}

fn source_description(source: &Source) -> String {
    match source {
        Source::Binary(_) => "<memory>".to_owned(),
        Source::File(path) | Source::SharedFile(path, _) => path.display().to_string(),
    }
}

fn font_data_for_face(database: &Database, id: ID) -> Option<FontData> {
    database.with_face_data(id, |bytes, face_index| {
        let mut data = FontData::from_owned(bytes.to_vec());
        data.index = face_index;
        data
    })
}

fn resolve_font(
    database: &Database,
    families: &[&str],
    weight: Weight,
    required_glyphs: &str,
) -> Option<ResolvedFont> {
    let id = find_font_face(database, families, weight, required_glyphs)?;
    let face = database.face(id)?;
    let family = face
        .families
        .first()
        .map(|(name, _)| name.clone())
        .unwrap_or_else(|| face.post_script_name.clone());
    let source = source_description(&face.source);
    let data = font_data_for_face(database, id)?;

    Some(ResolvedFont {
        id,
        data,
        family,
        source,
        weight: face.weight,
    })
}

fn cjk_region_priority(locale: Option<&str>) -> [CjkRegion; 4] {
    let locale = locale.unwrap_or_default().to_ascii_lowercase();
    let locale = locale.replace('-', "_");
    let subtags: Vec<_> = locale
        .split(['_', '.'])
        .filter(|subtag| !subtag.is_empty())
        .collect();
    let is_chinese = subtags.first() == Some(&"zh");
    let is_traditional_chinese = is_chinese
        && if subtags.contains(&"hant") {
            true
        } else if subtags.contains(&"hans") {
            false
        } else {
            subtags
                .iter()
                .any(|subtag| matches!(*subtag, "tw" | "hk" | "mo"))
        };

    if is_traditional_chinese {
        [
            CjkRegion::TraditionalChinese,
            CjkRegion::SimplifiedChinese,
            CjkRegion::Japanese,
            CjkRegion::Korean,
        ]
    } else if subtags.first() == Some(&"ja") {
        [
            CjkRegion::Japanese,
            CjkRegion::SimplifiedChinese,
            CjkRegion::TraditionalChinese,
            CjkRegion::Korean,
        ]
    } else if subtags.first() == Some(&"ko") {
        [
            CjkRegion::Korean,
            CjkRegion::SimplifiedChinese,
            CjkRegion::TraditionalChinese,
            CjkRegion::Japanese,
        ]
    } else {
        [
            CjkRegion::SimplifiedChinese,
            CjkRegion::TraditionalChinese,
            CjkRegion::Japanese,
            CjkRegion::Korean,
        ]
    }
}

fn current_locale() -> Option<String> {
    sys_locale::get_locale()
}

fn ordered_regular_specs(locale: Option<&str>) -> Vec<&'static FontSpec> {
    let mut ordered = Vec::with_capacity(REGULAR_FONT_SPECS.len());
    ordered.extend(
        REGULAR_FONT_SPECS
            .iter()
            .filter(|spec| spec.primary && spec.cjk_region.is_none()),
    );
    for region in cjk_region_priority(locale) {
        ordered.extend(
            REGULAR_FONT_SPECS
                .iter()
                .filter(|spec| spec.cjk_region == Some(region)),
        );
    }
    ordered.extend(
        REGULAR_FONT_SPECS
            .iter()
            .filter(|spec| !spec.primary && spec.cjk_region.is_none()),
    );
    ordered
}

fn install_regular_fonts(
    database: &Database,
    fonts: &mut FontDefinitions,
    locale: Option<&str>,
) -> InstalledRegularFonts {
    let mut loaded_faces = HashSet::new();
    let mut loaded_keys = Vec::new();
    let mut families_by_key = HashMap::new();
    let mut primary_key = None;
    let mut cjk_faces = Vec::new();

    for spec in ordered_regular_specs(locale) {
        if spec.cjk_region.is_some()
            && cjk_faces
                .iter()
                .any(|id| font_has_glyphs(database, *id, spec.required_glyphs))
        {
            continue;
        }

        let Some(resolved) = resolve_font(
            database,
            spec.families,
            Weight::NORMAL,
            spec.required_glyphs,
        ) else {
            continue;
        };
        if !loaded_faces.insert(resolved.id) {
            continue;
        }
        if spec.cjk_region.is_some() {
            cjk_faces.push(resolved.id);
        }

        log::info!(
            "Loaded system font fallback {} ({}) from {} (face index {})",
            spec.key,
            resolved.family,
            resolved.source,
            resolved.data.index
        );
        fonts
            .font_data
            .insert(spec.key.to_owned(), resolved.data.into());
        families_by_key.insert(spec.key.to_owned(), resolved.family);

        if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
            if spec.primary {
                family.insert(0, spec.key.to_owned());
                primary_key = Some(spec.key.to_owned());
            } else {
                family.push(spec.key.to_owned());
            }
        }
        if let Some(family) = fonts.families.get_mut(&FontFamily::Monospace) {
            family.push(spec.key.to_owned());
        }
        loaded_keys.push(spec.key.to_owned());
    }

    InstalledRegularFonts {
        face_ids: loaded_faces,
        keys: loaded_keys,
        families_by_key,
        primary_key,
    }
}

fn push_unique(
    destination: &mut Vec<String>,
    seen: &mut HashSet<String>,
    font_names: impl IntoIterator<Item = String>,
) {
    for font_name in font_names {
        if seen.insert(font_name.clone()) {
            destination.push(font_name);
        }
    }
}

fn build_strong_family(
    primary_regular: Option<&str>,
    primary_bold: Option<&str>,
    default_proportional: &[String],
    script_bold: &[String],
    proportional: &[String],
) -> Vec<String> {
    let mut family = Vec::new();
    let mut seen = HashSet::new();

    push_unique(
        &mut family,
        &mut seen,
        primary_bold.into_iter().map(str::to_owned),
    );
    push_unique(
        &mut family,
        &mut seen,
        primary_regular.into_iter().map(str::to_owned),
    );
    // Keep egui's bundled Latin and emoji fonts ahead of script-specific bold
    // faces. A CJK font may also contain Latin glyphs, but should not replace
    // the UI's normal Latin face merely because no paired system sans is available.
    push_unique(&mut family, &mut seen, default_proportional.iter().cloned());
    push_unique(&mut family, &mut seen, script_bold.iter().cloned());
    push_unique(&mut family, &mut seen, proportional.iter().cloned());

    family
}

fn is_true_bold(weight: Weight) -> bool {
    weight >= Weight::BOLD
}

fn ordered_script_bold_fonts(
    regular_keys: &[String],
    bold_by_regular_key: &HashMap<String, String>,
) -> Vec<String> {
    regular_keys
        .iter()
        .filter_map(|key| bold_by_regular_key.get(key).cloned())
        .collect()
}

fn install_strong_font_family(
    database: &Database,
    fonts: &mut FontDefinitions,
    regular_fonts: &InstalledRegularFonts,
    default_proportional: &[String],
) {
    let mut loaded_faces = regular_fonts.face_ids.clone();
    let mut primary_bold = None;
    let mut script_bold_by_regular_key = HashMap::new();

    for spec in STRONG_FONT_SPECS {
        let Some(regular_family) = regular_fonts.families_by_key.get(spec.matching_regular_key)
        else {
            continue;
        };
        let matching_family = [regular_family.as_str()];

        let Some(resolved) = resolve_font(
            database,
            &matching_family,
            Weight::BOLD,
            spec.required_glyphs,
        ) else {
            continue;
        };
        if !is_true_bold(resolved.weight) {
            log::debug!(
                "Ignoring {} because the closest installed face has weight {}",
                spec.key,
                resolved.weight.0
            );
            continue;
        }
        if !loaded_faces.insert(resolved.id) {
            continue;
        }

        log::info!(
            "Loaded Markdown strong font {} ({}) from {} (face index {})",
            spec.key,
            resolved.family,
            resolved.source,
            resolved.data.index
        );
        fonts
            .font_data
            .insert(spec.key.to_owned(), resolved.data.into());
        if spec.primary {
            primary_bold = Some(spec.key.to_owned());
        } else {
            script_bold_by_regular_key
                .insert(spec.matching_regular_key.to_owned(), spec.key.to_owned());
        }
    }
    let script_bold = ordered_script_bold_fonts(&regular_fonts.keys, &script_bold_by_regular_key);

    let proportional = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let strong_family = build_strong_family(
        regular_fonts.primary_key.as_deref(),
        primary_bold.as_deref(),
        default_proportional,
        &script_bold,
        &proportional,
    );

    if primary_bold.is_none() {
        if let Some(primary_family) = regular_fonts
            .primary_key
            .as_ref()
            .and_then(|key| regular_fonts.families_by_key.get(key))
        {
            log::warn!(
                "No true bold face found for primary system family {primary_family}; \
                 Markdown strong Latin text will use a regular fallback."
            );
        } else {
            log::debug!(
                "No paired system sans family found; using egui defaults for Markdown strong text."
            );
        }
    }

    // The renderer selects this named family for strong spans. Register it even
    // without a bold face so systems with few fonts still degrade without panic.
    fonts
        .families
        .insert(FontFamily::Name(STRONG_FONT_FAMILY.into()), strong_family);
}

/// Load installed fonts by family metadata instead of distro-specific paths.
///
/// `fontdb` scans the platform's configured font directories and also reports
/// the face index for font collections, which is copied into `egui::FontData`.
pub(crate) fn setup_fonts(ctx: &egui::Context) {
    let mut database = Database::new();
    database.load_system_fonts();
    log::info!("Discovered {} installed font faces", database.len());

    let mut fonts = FontDefinitions::default();
    let default_proportional = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let locale = current_locale();
    let regular_fonts = install_regular_fonts(&database, &mut fonts, locale.as_deref());

    if regular_fonts.keys.is_empty() {
        log::warn!("No suitable system font fallbacks found; using egui defaults.");
    } else {
        log::info!(
            "Loaded {} system font fallbacks: {}",
            regular_fonts.keys.len(),
            regular_fonts.keys.join(", ")
        );
    }

    install_strong_font_family(&database, &mut fonts, &regular_fonts, &default_proportional);
    ctx.set_fonts(fonts);
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use fontdb::{FaceInfo, Language, Stretch};

    use super::*;

    fn push_face(database: &mut Database, family: &str, weight: Weight, index: u32) -> ID {
        database.push_face_info(FaceInfo {
            id: ID::dummy(),
            source: Source::Binary(Arc::new(Vec::<u8>::new())),
            index,
            families: vec![(family.to_owned(), Language::English_UnitedStates)],
            post_script_name: family.replace(' ', "-"),
            style: Style::Normal,
            weight,
            stretch: Stretch::Normal,
            monospaced: false,
        })
    }

    #[test]
    fn font_family_candidates_are_prioritized() {
        let mut database = Database::new();
        let fallback = push_face(&mut database, "Fallback Sans", Weight::NORMAL, 0);
        let preferred = push_face(&mut database, "Preferred Sans", Weight::NORMAL, 0);

        assert_eq!(
            find_font_face_with(
                &database,
                &["Preferred Sans", "Fallback Sans"],
                Weight::NORMAL,
                |_| true
            ),
            Some(preferred)
        );
        assert_ne!(preferred, fallback);
    }

    #[test]
    fn bold_query_selects_the_bold_face() {
        let mut database = Database::new();
        push_face(&mut database, "Example Sans", Weight::NORMAL, 0);
        let bold = push_face(&mut database, "Example Sans", Weight::BOLD, 0);

        assert_eq!(
            find_font_face_with(&database, &["Example Sans"], Weight::BOLD, |_| true),
            Some(bold)
        );
    }

    #[test]
    fn font_without_required_glyphs_is_skipped() {
        let mut database = Database::new();
        let missing = push_face(&mut database, "Missing Glyphs", Weight::NORMAL, 0);
        let supported = push_face(&mut database, "Supported Glyphs", Weight::NORMAL, 0);

        assert_eq!(
            find_font_face_with(
                &database,
                &["Missing Glyphs", "Supported Glyphs"],
                Weight::NORMAL,
                |id| id != missing
            ),
            Some(supported)
        );
    }

    #[test]
    fn resolved_font_preserves_collection_face_index() {
        let mut database = Database::new();
        let id = push_face(&mut database, "Collection Sans", Weight::NORMAL, 7);
        let data = font_data_for_face(&database, id).expect("font data");

        assert_eq!(data.index, 7);
    }

    #[test]
    fn empty_database_keeps_egui_fallbacks_for_strong_text() {
        let database = Database::new();
        let mut fonts = FontDefinitions::default();
        let default_proportional = fonts.families[&FontFamily::Proportional].clone();
        let regular_fonts = install_regular_fonts(&database, &mut fonts, Some("en-US"));

        assert!(regular_fonts.keys.is_empty());
        install_strong_font_family(&database, &mut fonts, &regular_fonts, &default_proportional);

        assert_eq!(
            fonts.families[&FontFamily::Name(STRONG_FONT_FAMILY.into())],
            default_proportional
        );
    }

    #[test]
    fn only_weight_700_or_higher_counts_as_bold() {
        assert!(!is_true_bold(Weight::MEDIUM));
        assert!(!is_true_bold(Weight::SEMIBOLD));
        assert!(is_true_bold(Weight::BOLD));
        assert!(is_true_bold(Weight::BLACK));
    }

    #[test]
    fn default_latin_precedes_script_bold_without_primary_system_sans() {
        let strong = build_strong_family(
            None,
            None,
            &["Ubuntu-Light".to_owned(), "NotoEmoji-Regular".to_owned()],
            &["SystemCjkScSansBold".to_owned()],
            &["Ubuntu-Light".to_owned(), "SystemCjkScSans".to_owned()],
        );

        assert_eq!(
            strong,
            [
                "Ubuntu-Light",
                "NotoEmoji-Regular",
                "SystemCjkScSansBold",
                "SystemCjkScSans"
            ]
        );
    }

    #[test]
    fn matching_primary_regular_and_bold_stay_together() {
        let strong = build_strong_family(
            Some("SystemSans"),
            Some("SystemSansBold"),
            &["Ubuntu-Light".to_owned()],
            &["SystemCjkScSansBold".to_owned()],
            &[
                "SystemSans".to_owned(),
                "Ubuntu-Light".to_owned(),
                "SystemCjkScSans".to_owned(),
            ],
        );

        assert_eq!(
            strong,
            [
                "SystemSansBold",
                "SystemSans",
                "Ubuntu-Light",
                "SystemCjkScSansBold",
                "SystemCjkScSans"
            ]
        );
    }

    #[test]
    fn cjk_specs_keep_region_specific_glyph_variants_separate() {
        let cjk_specs: Vec<_> = REGULAR_FONT_SPECS
            .iter()
            .filter(|spec| spec.key.starts_with("SystemCjk"))
            .map(|spec| (spec.key, spec.required_glyphs))
            .collect();

        assert_eq!(
            cjk_specs,
            [
                ("SystemCjkScSans", "中文"),
                ("SystemCjkTcSans", "繁體"),
                ("SystemCjkJpSans", "かなカナ"),
                ("SystemCjkKrSans", "한글")
            ]
        );
    }

    #[test]
    fn cjk_priority_follows_the_system_locale() {
        assert_eq!(
            cjk_region_priority(Some("zh_TW.UTF-8"))[0],
            CjkRegion::TraditionalChinese
        );
        assert_eq!(
            cjk_region_priority(Some("zh-Hant"))[0],
            CjkRegion::TraditionalChinese
        );
        assert_eq!(
            cjk_region_priority(Some("zh-Hant-HK"))[0],
            CjkRegion::TraditionalChinese
        );
        assert_eq!(
            cjk_region_priority(Some("zh-Hans-CN"))[0],
            CjkRegion::SimplifiedChinese
        );
        assert_eq!(
            cjk_region_priority(Some("zh-Hans-TW"))[0],
            CjkRegion::SimplifiedChinese
        );
        assert_eq!(
            cjk_region_priority(Some("ja_JP.UTF-8"))[0],
            CjkRegion::Japanese
        );
        assert_eq!(
            cjk_region_priority(Some("ko_KR.UTF-8"))[0],
            CjkRegion::Korean
        );
        assert_eq!(
            cjk_region_priority(Some("C.UTF-8"))[0],
            CjkRegion::SimplifiedChinese
        );
    }

    #[test]
    fn primary_system_sans_precedes_locale_specific_cjk() {
        let ordered = ordered_regular_specs(Some("ja_JP.UTF-8"));
        assert_eq!(ordered[0].key, "SystemSans");
        assert_eq!(ordered[1].key, "SystemCjkJpSans");
    }

    #[test]
    fn script_bold_order_follows_loaded_regular_order() {
        let regular_keys = vec![
            "SystemSans".to_owned(),
            "SystemCjkJpSans".to_owned(),
            "SystemCjkTcSans".to_owned(),
        ];
        let bold_by_regular_key = HashMap::from([
            (
                "SystemCjkTcSans".to_owned(),
                "SystemCjkTcSansBold".to_owned(),
            ),
            (
                "SystemCjkJpSans".to_owned(),
                "SystemCjkJpSansBold".to_owned(),
            ),
        ]);

        assert_eq!(
            ordered_script_bold_fonts(&regular_keys, &bold_by_regular_key),
            ["SystemCjkJpSansBold", "SystemCjkTcSansBold"]
        );
    }

    #[test]
    #[ignore = "requires an installed CJK font; run explicitly for local verification"]
    fn installed_fonts_cover_cjk_in_regular_and_strong_text() {
        let context = egui::Context::default();
        setup_fonts(&context);
        context.begin_pass(Default::default());

        let regular = egui::FontId::proportional(16.0);
        let strong = egui::FontId::new(16.0, FontFamily::Name(STRONG_FONT_FAMILY.into()));
        let cjk_samples = "中文繁體かなカナ한글";
        let regular_has_cjk = context.fonts_mut(|fonts| fonts.has_glyphs(&regular, cjk_samples));
        let strong_has_cjk = context.fonts_mut(|fonts| fonts.has_glyphs(&strong, cjk_samples));

        let _ = context.end_pass();
        assert!(regular_has_cjk, "regular font chain is missing CJK glyphs");
        assert!(strong_has_cjk, "strong font chain is missing CJK glyphs");
    }
}
