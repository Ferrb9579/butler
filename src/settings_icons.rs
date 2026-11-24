//! Reference glyphs from the T3 Code v0.1.11 AppImage (Lucide React 0.564.0).
//! The checked-in SVGs and Incular paths share one deterministic export.
//! See assets/icons/README.md for source, generator and license notices.
//! The provider marks are the filled SVG paths from the pinned T3 source.

use incular::prelude::{
    AffineTransform, Canvas, Color, CustomPaint, FillRule, Icon, LineCap, LineJoin, Offset, Path,
    Rect, Size, SizedBox, Stack, Stroke, Widget,
};
use std::sync::{Arc, OnceLock};

#[derive(Clone, Copy)]
enum Segment {
    M(f32, f32),
    L(f32, f32),
    C(f32, f32, f32, f32, f32, f32),
    Z,
}

struct Glyph {
    outline: &'static [Segment],
    interior: &'static [Segment],
    paths: OnceLock<(Arc<Path>, Option<Arc<Path>>)>,
}

impl Glyph {
    const fn new(outline: &'static [Segment], interior: &'static [Segment]) -> Self {
        Self {
            outline,
            interior,
            paths: OnceLock::new(),
        }
    }
}

fn make_path(commands: &[Segment]) -> Arc<Path> {
    let mut path = Path::builder();
    for &command in commands {
        match command {
            M(x, y) => path.move_to(Offset::new(x, y)),
            L(x, y) => path.line_to(Offset::new(x, y)),
            C(x, y, a, b, u, v) => {
                path.cubic_to(Offset::new(x, y), Offset::new(a, b), Offset::new(u, v))
            }
            Z => path.close(),
        };
    }
    Arc::new(path.build())
}

use Segment::{C, L, M, Z};

#[path = "acp_registry_mark_generated.rs"]
mod acp_registry_mark_generated;

struct Brand {
    view_box: Rect,
    rule: FillRule,
    path: Arc<Path>,
}

impl Brand {
    fn new(origin: Offset, size: Size, rule: FillRule, commands: &[Segment]) -> Self {
        Self {
            view_box: Rect::from_origin_size(origin, size),
            rule,
            path: make_path(commands),
        }
    }

    fn fit(&self, size: f32) -> AffineTransform {
        // SVG's xMidYMid meet keeps the aspect ratio and authored padding.
        let bounds = self.view_box;
        let scale = size / bounds.size.width.max(bounds.size.height);
        let offset = Offset::new(
            (size - bounds.size.width * scale) * 0.5 - bounds.origin.x * scale,
            (size - bounds.size.height * scale) * 0.5 - bounds.origin.y * scale,
        );
        AffineTransform::translation(offset).then(AffineTransform::scale(scale))
    }
}

/// Draws a provider's original Settings mark at its intrinsic SVG size.
///
/// Pass the driver name, not its editable display name. T3's 16px SVG sits
/// inside a centered 20px wrapper. Codex uses the supplied CSS current color
/// (foreground at 80% in Settings); Claude and Cursor have fixed brand fills.
pub fn provider_brand(driver: &str, size: f32, current_color: Color, dark: bool) -> Widget {
    let size = size.max(0.0);
    let Some((brand, fill)) = brand(driver, current_color, dark) else {
        return SizedBox::new().width(size).height(size).into();
    };
    paint_brand(brand, size, fill)
}

/// The published pure-black ACP Registry mark, for the light Welcome surface.
pub fn acp_registry_mark(size: f32) -> Widget {
    paint_brand(acp_brand(), size.max(0.0), Color::BLACK)
}

fn acp_brand() -> &'static Brand {
    static ACP: OnceLock<Brand> = OnceLock::new();
    ACP.get_or_init(|| {
        Brand::new(
            Offset::ZERO,
            Size::new(576.0, 220.0),
            FillRule::NonZero,
            acp_registry_mark_generated::PATH,
        )
    })
}

fn paint_brand(brand: &Brand, size: f32, fill: Color) -> Widget {
    let mut canvas = Canvas::default();
    canvas.save_transform(brand.fit(size));
    canvas.fill_path(brand.path.clone(), fill, brand.rule);
    canvas.restore();
    CustomPaint::new(Size::new(size, size), canvas.finish()).into()
}

fn brand(driver: &str, current_color: Color, dark: bool) -> Option<(&'static Brand, Color)> {
    static CODEX: OnceLock<Brand> = OnceLock::new();
    static CLAUDE: OnceLock<Brand> = OnceLock::new();
    static CURSOR: OnceLock<Brand> = OnceLock::new();

    let (storage, view_offset, view_size, rule, commands, fill) = match driver {
        "codex" | "Codex" => (
            &CODEX,
            Offset::new(100.0, 100.0),
            Size::new(411.0, 411.0),
            FillRule::EvenOdd,
            CODEX_PATH,
            current_color,
        ),
        "claudeAgent" | "claude" | "Claude" => (
            &CLAUDE,
            Offset::ZERO,
            Size::new(256.0, 257.0),
            FillRule::NonZero,
            CLAUDE_PATH,
            Color::rgba(0xd9, 0x77, 0x57, 0xff),
        ),
        "cursor" | "Cursor" => (
            &CURSOR,
            Offset::ZERO,
            Size::new(466.73, 532.09),
            FillRule::NonZero,
            CURSOR_PATH,
            if dark {
                Color::rgba(0xed, 0xec, 0xec, 0xff)
            } else {
                Color::rgba(0x26, 0x25, 0x1e, 0xff)
            },
        ),
        _ => return None,
    };
    Some((
        storage.get_or_init(|| Brand::new(view_offset, view_size, rule, commands)),
        fill,
    ))
}

#[cfg(test)]
mod brand_tests {
    use super::*;

    #[test]
    fn acp_mark_keeps_the_original_wide_view_box_in_the_square_slot() {
        let acp = acp_brand();
        assert_eq!(acp.rule, FillRule::NonZero);
        let transform = acp.fit(16.0);
        let top_left = transform.transform_point(Offset::ZERO);
        let bottom_right = transform.transform_point(Offset::new(576.0, 220.0));
        let fitted_height = 16.0 * 220.0 / 576.0;
        assert!(top_left.x.abs() < 0.001);
        assert!((bottom_right.x - 16.0).abs() < 0.001);
        assert!((bottom_right.y - top_left.y - fitted_height).abs() < 0.001);
        assert!((top_left.y + bottom_right.y - 16.0).abs() < 0.001);
    }

    #[test]
    fn fits_source_viewboxes_and_preserves_logo_cutouts() {
        let (codex, _) = brand("Codex", Color::BLACK, false).unwrap();
        assert_eq!(codex.rule, FillRule::EvenOdd);
        assert!(!codex.path.contains(Offset::new(305.5, 305.5), codex.rule));
        let first = codex.fit(16.0).transform_point(Offset::new(100.0, 100.0));
        let last = codex.fit(16.0).transform_point(Offset::new(511.0, 511.0));
        assert!(first.x.abs() < 0.001 && first.y.abs() < 0.001);
        assert!((last.x - 16.0).abs() < 0.001 && (last.y - 16.0).abs() < 0.001);

        let (cursor, _) = brand("Cursor", Color::BLACK, false).unwrap();
        assert!(!cursor.path.contains(Offset::new(250.0, 200.0), cursor.rule));
        assert!(cursor.path.contains(Offset::new(60.0, 200.0), cursor.rule));
        let top_left = cursor.fit(16.0).transform_point(Offset::ZERO);
        assert!((top_left.x - 0.9827).abs() < 0.001 && top_left.y.abs() < 0.001);
    }

    #[test]
    fn uses_original_theme_and_brand_fills() {
        let current = Color::rgba(24, 24, 27, 204);
        assert_eq!(brand("codex", current, false).unwrap().1, current);
        assert_eq!(
            brand("claudeAgent", current, false).unwrap().1,
            Color::rgba(0xd9, 0x77, 0x57, 0xff)
        );
        assert_eq!(
            brand("cursor", current, false).unwrap().1,
            Color::rgba(0x26, 0x25, 0x1e, 0xff)
        );
        assert_eq!(
            brand("cursor", current, true).unwrap().1,
            Color::rgba(0xed, 0xec, 0xec, 0xff)
        );
    }
}

/// Resolve either a T3 settings row label or an original Lucide kebab-case name.
/// Unrecognized names retain the requested layout footprint without a fallback glyph.
pub fn icon(name: &str, size: f32, color: Color) -> Widget {
    // T3 index.css sets svg.lucide[stroke-width="2"] to 1.75, including its
    // createLucideIcon-based SnapShots glyph. The raw exports retain 2.
    rendered_icon(name, size, color, 24.0, 1.75)
}

/// The reference uses a custom 14px stroke for the normal Submit state.
pub fn compose_send(color: Color) -> Widget {
    rendered_icon("compose-send", 14.0, color, 14.0, 1.8)
}

fn rendered_icon(name: &str, size: f32, color: Color, extent: f32, stroke_width: f32) -> Widget {
    let Some(glyph) = glyph(name) else {
        return SizedBox::new().width(size).height(size).into();
    };
    let (outline, interior) = glyph.paths.get_or_init(|| {
        (
            make_path(glyph.outline),
            (!glyph.interior.is_empty()).then(|| make_path(glyph.interior)),
        )
    });
    let view_box = Rect::from_origin_size(Offset::ZERO, Size::new(extent, extent));
    let outlined: Widget = Icon::new(outline.clone())
        .view_box(view_box)
        .size(size)
        .brush(color)
        .stroke(Stroke {
            width: stroke_width,
            cap: LineCap::Round,
            join: LineJoin::Round,
            ..Stroke::default()
        })
        .into();
    match interior {
        Some(interior) => Stack::new(vec![
            Icon::new(interior.clone())
                .view_box(view_box)
                .size(size)
                .brush(color)
                .into(),
            outlined,
        ])
        .into(),
        None => outlined,
    }
}

#[path = "reference_icons_generated.rs"]
mod generated;

fn glyph(name: &str) -> Option<&'static Glyph> {
    let slug = match name {
        "palette-link" => "link",
        "palette-folder-git" => "folder-git-2",
        "palette-folder-plus" => "folder-plus",
        "General" => "settings-2",
        "Appearance" => "palette",
        "Chat" => "message-square-text",
        "Diffs & pull requests" => "file-diff",
        "Keybindings" => "keyboard",
        "Voice" => "mic",
        "SnapShots" | "Snapshots" => "snap-shot",
        "Providers" => "bot",
        "Threads" => "messages-square",
        "Browser" => "globe",
        "Devices" => "smartphone",
        "Scheduled tasks" => "calendar-clock",
        "Projects" => "panels-top-left",
        "Source control & issues" | "Source control" => "git-branch",
        "Worktrees" => "folder-tree",
        "Environments" | "Connections" => "server",
        "Sharing & access" => "shield-check",
        "Secrets" => "key-round",
        "Maintenance" | "Diagnostics" => "wrench",
        "Labs" => "flask-conical",
        "About" => "info",
        "Archive" => "archive",
        "Search" => "search",
        "close" | "Close" => "x",
        _ => name,
    };
    generated::glyph(slug)
}

// T3 v0.1.11 apps/web/src/components/Icons.tsx:414–426 (OpenAI).
const CODEX_PATH: &[Segment] = &[
    M(252.794, 108.802),
    C(289.191, 99.0484, 326.265, 110.305, 351.148, 135.135),
    C(385.113, 126.072, 422.85, 134.862, 449.492, 161.505),
    C(476.136, 188.149, 484.925, 225.888, 475.862, 259.85),
    L(475.862, 259.854),
    C(500.696, 284.735, 511.95, 321.81, 502.198, 358.207),
    C(492.447, 394.602, 464.161, 421.084, 430.215, 430.217),
    C(421.083, 464.162, 394.603, 492.448, 358.206, 502.199),
    C(321.812, 511.951, 284.734, 500.693, 259.852, 475.864),
    C(225.887, 484.927, 188.15, 476.137, 161.507, 449.495),
    C(134.864, 422.851, 126.073, 385.111, 135.136, 351.149),
    C(110.304, 326.266, 99.0496, 289.192, 108.801, 252.795),
    C(118.552, 216.4, 146.84, 189.918, 180.784, 180.785),
    C(189.917, 146.841, 216.396, 118.553, 252.794, 108.802),
    Z,
    M(374.292, 407.145),
    C(374.292, 411.271, 372.092, 415.086, 368.517, 417.148),
    L(283.723, 466.102),
    C(302.487, 480.585, 327.555, 486.459, 352.217, 479.852),
    C(386.997, 470.532, 410.068, 439.312, 410.555, 405.006),
    L(410.555, 317.717),
    C(410.555, 315.08, 409.125, 312.621, 406.843, 311.303),
    L(374.292, 292.509),
    Z,
    M(251.868, 415.897),
    C(248.296, 417.959, 243.893, 417.959, 240.317, 415.897),
    L(155.526, 366.942),
    C(152.366, 390.436, 159.811, 415.08, 177.866, 433.136),
    L(177.863, 433.136),
    C(203.325, 458.594, 241.896, 462.962, 271.85, 446.232),
    L(347.449, 402.586),
    C(349.735, 401.268, 351.148, 398.8, 351.148, 396.163),
    L(351.148, 358.579),
    Z,
    M(368.602, 220.628),
    C(366.319, 219.309, 363.474, 219.318, 361.191, 220.637),
    L(328.641, 239.431),
    L(427.921, 296.749),
    C(431.496, 298.811, 433.697, 302.627, 433.697, 306.752),
    L(433.697, 404.661),
    C(455.622, 395.654, 473.244, 376.881, 479.851, 352.218),
    C(489.169, 317.442, 473.668, 281.85, 444.201, 264.274),
    Z,
    M(177.303, 206.34),
    C(155.377, 215.348, 137.756, 234.122, 131.148, 258.783),
    C(121.832, 293.561, 137.331, 329.153, 166.799, 346.727),
    L(242.398, 390.373),
    C(244.68, 391.692, 247.525, 391.684, 249.807, 390.366),
    L(282.357, 371.572),
    L(183.078, 314.253),
    C(179.504, 312.189, 177.303, 308.375, 177.303, 304.251),
    Z,
    M(259.849, 279.145),
    L(259.849, 331.858),
    L(305.5, 358.213),
    L(351.15, 331.858),
    L(351.15, 279.145),
    L(305.5, 252.789),
    Z,
    M(327.276, 144.9),
    C(308.512, 130.418, 283.445, 124.543, 258.782, 131.15),
    C(224.002, 140.471, 200.931, 171.691, 200.445, 205.995),
    L(200.445, 293.286),
    C(200.445, 295.923, 201.875, 298.381, 204.158, 299.7),
    L(236.707, 318.493),
    L(236.707, 203.856),
    C(236.707, 199.731, 238.909, 195.916, 242.483, 193.853),
    Z,
    M(433.137, 177.867),
    C(407.675, 152.407, 369.103, 148.038, 339.149, 164.769),
    L(263.55, 208.415),
    C(261.265, 209.734, 259.852, 212.202, 259.852, 214.838),
    L(259.852, 252.423),
    L(359.132, 195.105),
    C(362.703, 193.041, 367.108, 193.041, 370.682, 195.105),
    L(455.473, 244.06),
    C(458.635, 220.567, 451.189, 195.922, 433.135, 177.867),
    Z,
];

// T3 v0.1.11 apps/web/src/components/Icons.tsx:429–437 (ClaudeAI).
const CLAUDE_PATH: &[Segment] = &[
    M(50.228, 170.321),
    L(100.585, 142.064),
    L(101.428, 139.601),
    L(100.585, 138.24),
    L(98.123, 138.24),
    L(89.697, 137.722),
    L(60.922, 136.944),
    L(35.97, 135.907),
    L(11.795, 134.611),
    L(5.703, 133.314),
    L(0.0, 125.796),
    L(0.583, 122.037),
    L(5.703, 118.603),
    L(13.027, 119.251),
    L(29.229, 120.352),
    L(53.533, 122.037),
    L(71.162, 123.074),
    L(97.28, 125.796),
    L(101.428, 125.796),
    L(102.011, 124.111),
    L(100.585, 123.074),
    L(99.484, 122.037),
    L(74.337, 104.992),
    L(47.117, 86.975),
    L(32.859, 76.605),
    L(25.146, 71.355),
    L(21.258, 66.43),
    L(19.573, 55.672),
    L(26.573, 47.959),
    L(35.97, 48.608),
    L(38.368, 49.256),
    L(47.895, 56.579),
    L(68.245, 72.329),
    L(94.817, 91.9),
    L(98.706, 95.14),
    L(100.261, 94.038),
    L(100.456, 93.261),
    L(98.706, 90.344),
    L(84.253, 64.226),
    L(68.828, 37.654),
    L(61.958, 26.636),
    L(60.144, 20.026),
    C(59.496, 17.303, 59.042, 15.035, 59.042, 12.248),
    L(67.014, 1.425),
    L(71.42, 0.0),
    L(82.05, 1.426),
    L(86.522, 5.314),
    L(93.132, 20.415),
    L(103.826, 44.201),
    L(120.417, 76.541),
    L(125.278, 86.133),
    L(127.87, 95.012),
    L(128.843, 97.734),
    L(130.528, 97.734),
    L(130.528, 96.178),
    L(131.888, 77.967),
    L(134.416, 55.607),
    L(136.879, 26.831),
    L(137.722, 18.731),
    L(141.74, 9.009),
    L(149.711, 3.759),
    L(155.933, 6.74),
    L(161.053, 14.064),
    L(160.34, 18.794),
    L(157.294, 38.562),
    L(151.332, 69.542),
    L(147.443, 90.281),
    L(149.711, 90.281),
    L(152.304, 87.688),
    L(162.803, 73.754),
    L(180.431, 51.718),
    L(188.209, 42.969),
    L(197.282, 33.312),
    L(203.115, 28.711),
    L(214.133, 28.711),
    L(222.233, 40.766),
    L(218.605, 53.209),
    L(207.263, 67.597),
    L(197.865, 79.781),
    L(184.385, 97.928),
    L(175.959, 112.446),
    L(176.737, 113.612),
    L(178.747, 113.418),
    L(209.207, 106.937),
    L(225.669, 103.955),
    L(245.306, 100.585),
    L(254.186, 104.733),
    L(255.157, 108.946),
    L(251.657, 117.566),
    L(230.659, 122.75),
    L(206.031, 127.676),
    L(169.349, 136.361),
    L(168.895, 136.685),
    L(169.414, 137.333),
    L(185.94, 138.888),
    L(193.005, 139.277),
    L(210.309, 139.277),
    L(242.519, 141.675),
    L(250.945, 147.249),
    L(256.0, 154.054),
    L(255.157, 159.238),
    L(242.195, 165.849),
    L(224.697, 161.701),
    L(183.867, 151.98),
    L(169.867, 148.48),
    L(167.923, 148.48),
    L(167.923, 149.647),
    L(179.589, 161.053),
    L(200.976, 180.367),
    L(227.743, 205.254),
    L(229.103, 211.411),
    L(225.669, 216.271),
    L(222.039, 215.753),
    L(198.513, 198.06),
    L(189.44, 190.088),
    L(168.895, 172.784),
    L(167.535, 172.784),
    L(167.535, 174.598),
    L(172.265, 181.533),
    L(197.282, 219.123),
    L(198.578, 230.659),
    L(196.764, 234.419),
    L(190.283, 236.687),
    L(183.153, 235.39),
    L(168.506, 214.846),
    L(153.406, 191.708),
    L(141.221, 170.969),
    L(139.731, 171.812),
    L(132.537, 249.26),
    L(129.167, 253.213),
    L(121.389, 256.194),
    L(114.909, 251.269),
    L(111.473, 243.297),
    L(114.908, 227.548),
    L(119.056, 207.004),
    L(122.426, 190.671),
    L(125.472, 170.386),
    L(127.287, 163.646),
    L(127.157, 163.192),
    L(125.667, 163.386),
    L(110.372, 184.385),
    L(87.105, 215.818),
    L(68.699, 235.52),
    L(64.292, 237.27),
    L(56.644, 233.316),
    L(57.357, 226.252),
    L(61.634, 219.966),
    L(87.104, 187.561),
    L(102.464, 167.469),
    L(112.381, 155.869),
    L(112.316, 154.183),
    L(111.733, 154.183),
    L(44.07, 198.125),
    L(32.015, 199.68),
    L(26.83, 194.82),
    L(27.478, 186.848),
    L(29.941, 184.255),
    L(50.291, 170.256),
    L(50.227, 170.321),
    Z,
];

// T3 v0.1.11 packages/provider-cursor/src/client.ts:21–27 (Cursor).
const CURSOR_PATH: &[Segment] = &[
    M(457.43, 125.94),
    L(244.42, 2.96),
    C(237.58, -0.99, 229.14, -0.99, 222.3, 2.96),
    L(9.3, 125.94),
    C(3.55, 129.26, 0.0, 135.4, 0.0, 142.05),
    L(0.0, 390.04),
    C(0.0, 396.69, 3.55, 402.83, 9.3, 406.15),
    L(222.31, 529.13),
    C(229.15, 533.08, 237.59, 533.08, 244.43, 529.13),
    L(457.44, 406.15),
    C(463.19, 402.83, 466.74, 396.69, 466.74, 390.04),
    L(466.74, 142.05),
    C(466.74, 135.4, 463.19, 129.26, 457.44, 125.94),
    L(457.43, 125.94),
    Z,
    M(444.05, 151.99),
    L(238.42, 508.15),
    C(237.03, 510.55, 233.36, 509.57, 233.36, 506.79),
    L(233.36, 273.58),
    C(233.36, 268.92, 230.87, 264.61, 226.83, 262.27),
    L(24.87, 145.67),
    C(22.47, 144.28, 23.45, 140.61, 26.23, 140.61),
    L(437.49, 140.61),
    C(443.33, 140.61, 446.98, 146.94, 444.06, 152.0),
    L(444.05, 152.0),
    Z,
];
