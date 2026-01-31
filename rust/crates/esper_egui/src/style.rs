//! Render style definitions.

use egui::Color32;

/// Visual style for rendering.
#[derive(Debug, Clone)]
pub struct RenderStyle {
    /// Background color.
    pub background: Color32,

    /// Entity colors by state.
    pub entity: EntityStyle,

    /// Edge/relationship colors.
    pub edge: EdgeStyle,

    /// Selection colors.
    pub selection: SelectionStyle,

    /// Text colors.
    pub text: TextStyle,

    /// Grid/debug colors.
    pub grid: GridStyle,
}

/// Entity visual style.
#[derive(Debug, Clone)]
pub struct EntityStyle {
    /// Default entity fill color.
    pub fill: Color32,
    /// Entity stroke color.
    pub stroke: Color32,
    /// Stroke width.
    pub stroke_width: f32,
    /// Hovered entity fill.
    pub fill_hovered: Color32,
    /// Selected entity fill.
    pub fill_selected: Color32,
    /// Focused entity fill.
    pub fill_focused: Color32,
    /// Muted/inactive entity fill.
    pub fill_muted: Color32,
}

/// Edge/relationship style.
#[derive(Debug, Clone)]
pub struct EdgeStyle {
    /// Default edge color.
    pub color: Color32,
    /// Edge width.
    pub width: f32,
    /// Highlighted edge color.
    pub color_highlight: Color32,
    /// Muted edge color.
    pub color_muted: Color32,
    /// Control edge color (ownership/control relationships).
    pub color_control: Color32,
    /// Arrow size for directed edges.
    pub arrow_size: f32,
}

/// Selection highlight style.
#[derive(Debug, Clone)]
pub struct SelectionStyle {
    /// Selection box stroke color.
    pub stroke: Color32,
    /// Selection box fill color (transparent).
    pub fill: Color32,
    /// Selection stroke width.
    pub stroke_width: f32,
    /// Focus ring color.
    pub focus_ring: Color32,
    /// Focus ring width.
    pub focus_ring_width: f32,
}

/// Text style.
#[derive(Debug, Clone)]
pub struct TextStyle {
    /// Primary text color.
    pub primary: Color32,
    /// Secondary/muted text color.
    pub secondary: Color32,
    /// Label text color.
    pub label: Color32,
    /// Error/warning text color.
    pub error: Color32,
}

/// Grid/debug overlay style.
#[derive(Debug, Clone)]
pub struct GridStyle {
    /// Grid line color.
    pub line: Color32,
    /// Grid line width.
    pub line_width: f32,
    /// Major grid line color.
    pub major_line: Color32,
    /// Bounds box color.
    pub bounds: Color32,
}

impl Default for RenderStyle {
    fn default() -> Self {
        Self::dark()
    }
}

impl RenderStyle {
    /// Dark theme (default).
    pub fn dark() -> Self {
        Self {
            background: Color32::from_rgb(24, 24, 32),
            entity: EntityStyle {
                fill: Color32::from_rgb(60, 80, 120),
                stroke: Color32::from_rgb(100, 140, 200),
                stroke_width: 1.5,
                fill_hovered: Color32::from_rgb(80, 110, 160),
                fill_selected: Color32::from_rgb(100, 150, 220),
                fill_focused: Color32::from_rgb(120, 180, 255),
                fill_muted: Color32::from_rgb(40, 50, 70),
            },
            edge: EdgeStyle {
                color: Color32::from_rgb(80, 90, 110),
                width: 1.0,
                color_highlight: Color32::from_rgb(150, 180, 220),
                color_muted: Color32::from_rgb(50, 55, 65),
                color_control: Color32::from_rgb(200, 150, 100),
                arrow_size: 8.0,
            },
            selection: SelectionStyle {
                stroke: Color32::from_rgb(100, 180, 255),
                fill: Color32::from_rgba_unmultiplied(100, 180, 255, 30),
                stroke_width: 2.0,
                focus_ring: Color32::from_rgb(255, 200, 100),
                focus_ring_width: 3.0,
            },
            text: TextStyle {
                primary: Color32::from_rgb(220, 220, 230),
                secondary: Color32::from_rgb(140, 140, 160),
                label: Color32::from_rgb(180, 180, 200),
                error: Color32::from_rgb(255, 100, 100),
            },
            grid: GridStyle {
                line: Color32::from_rgb(40, 45, 55),
                line_width: 0.5,
                major_line: Color32::from_rgb(55, 60, 75),
                bounds: Color32::from_rgb(100, 100, 120),
            },
        }
    }

    /// Light theme.
    pub fn light() -> Self {
        Self {
            background: Color32::from_rgb(245, 245, 250),
            entity: EntityStyle {
                fill: Color32::from_rgb(200, 210, 230),
                stroke: Color32::from_rgb(100, 120, 160),
                stroke_width: 1.5,
                fill_hovered: Color32::from_rgb(180, 195, 220),
                fill_selected: Color32::from_rgb(150, 180, 220),
                fill_focused: Color32::from_rgb(120, 160, 220),
                fill_muted: Color32::from_rgb(220, 225, 235),
            },
            edge: EdgeStyle {
                color: Color32::from_rgb(160, 170, 190),
                width: 1.0,
                color_highlight: Color32::from_rgb(80, 120, 180),
                color_muted: Color32::from_rgb(200, 205, 215),
                color_control: Color32::from_rgb(180, 130, 80),
                arrow_size: 8.0,
            },
            selection: SelectionStyle {
                stroke: Color32::from_rgb(60, 140, 220),
                fill: Color32::from_rgba_unmultiplied(60, 140, 220, 30),
                stroke_width: 2.0,
                focus_ring: Color32::from_rgb(220, 160, 60),
                focus_ring_width: 3.0,
            },
            text: TextStyle {
                primary: Color32::from_rgb(30, 30, 40),
                secondary: Color32::from_rgb(100, 100, 120),
                label: Color32::from_rgb(60, 60, 80),
                error: Color32::from_rgb(200, 60, 60),
            },
            grid: GridStyle {
                line: Color32::from_rgb(220, 225, 235),
                line_width: 0.5,
                major_line: Color32::from_rgb(200, 205, 220),
                bounds: Color32::from_rgb(150, 160, 180),
            },
        }
    }

    /// High contrast theme for accessibility.
    pub fn high_contrast() -> Self {
        Self {
            background: Color32::BLACK,
            entity: EntityStyle {
                fill: Color32::from_rgb(0, 60, 120),
                stroke: Color32::WHITE,
                stroke_width: 2.0,
                fill_hovered: Color32::from_rgb(0, 100, 180),
                fill_selected: Color32::from_rgb(0, 150, 255),
                fill_focused: Color32::YELLOW,
                fill_muted: Color32::from_rgb(40, 40, 40),
            },
            edge: EdgeStyle {
                color: Color32::from_rgb(128, 128, 128),
                width: 1.5,
                color_highlight: Color32::WHITE,
                color_muted: Color32::from_rgb(60, 60, 60),
                color_control: Color32::from_rgb(255, 200, 0),
                arrow_size: 10.0,
            },
            selection: SelectionStyle {
                stroke: Color32::YELLOW,
                fill: Color32::from_rgba_unmultiplied(255, 255, 0, 40),
                stroke_width: 3.0,
                focus_ring: Color32::from_rgb(0, 255, 255),
                focus_ring_width: 4.0,
            },
            text: TextStyle {
                primary: Color32::WHITE,
                secondary: Color32::from_rgb(180, 180, 180),
                label: Color32::WHITE,
                error: Color32::from_rgb(255, 100, 100),
            },
            grid: GridStyle {
                line: Color32::from_rgb(60, 60, 60),
                line_width: 1.0,
                major_line: Color32::from_rgb(100, 100, 100),
                bounds: Color32::YELLOW,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_themes() {
        let dark = RenderStyle::dark();
        let light = RenderStyle::light();
        let hc = RenderStyle::high_contrast();

        // Dark background should be darker than light
        assert!(dark.background.r() < light.background.r());

        // High contrast should have black background
        assert_eq!(hc.background, Color32::BLACK);
    }

    #[test]
    fn style_default_is_dark() {
        let default = RenderStyle::default();
        let dark = RenderStyle::dark();
        assert_eq!(default.background, dark.background);
    }
}
