//! Fixed camera angle presets for consistent, predictable renders
//!
//! Provides a set of standard viewing angles that MCP clients can use
//! to get reproducible screenshots from any angle.

use glam::Vec3;
use soyuz_render::Camera;

/// Fixed camera viewing angles
///
/// These presets provide predictable, reproducible camera positions
/// that work well for inspecting 3D models from standard viewpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CameraAngle {
    /// View from front (+Z looking toward origin)
    Front,

    /// View from back (-Z looking toward origin)
    Back,

    /// View from left (-X looking toward origin)
    Left,

    /// View from right (+X looking toward origin)
    Right,

    /// View from above (+Y looking down)
    Top,

    /// View from below (-Y looking up)
    Bottom,

    /// Classic 3/4 isometric view (default)
    #[default]
    Isometric,
}

impl CameraAngle {
    /// Convert to a camera positioned to view the given bounds
    ///
    /// The camera is positioned at an appropriate distance to frame
    /// the entire bounding box with some padding.
    pub fn to_camera(&self, center: Vec3, size: f32) -> Camera {
        // Distance multiplier to ensure the object fits in view
        let distance = size.max(1.0) * 2.5;

        let position = match self {
            Self::Front => center + Vec3::new(0.0, 0.0, distance),
            Self::Back => center + Vec3::new(0.0, 0.0, -distance),
            Self::Left => center + Vec3::new(-distance, 0.0, 0.0),
            Self::Right => center + Vec3::new(distance, 0.0, 0.0),
            Self::Top => center + Vec3::new(0.0, distance, 0.001),
            Self::Bottom => center + Vec3::new(0.0, -distance, 0.001),
            Self::Isometric => {
                // Classic isometric: equal parts X, Y, Z offset
                let d = distance * 0.6;
                center + Vec3::new(d, d * 0.8, d)
            }
        };

        Camera::look_at(position, center)
    }

    /// Parse a camera angle from a string
    ///
    /// Accepts case-insensitive names: "front", "back", "left", "right",
    /// "top", "bottom", "isometric" (or "iso").
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "front" => Some(Self::Front),
            "back" => Some(Self::Back),
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            "top" => Some(Self::Top),
            "bottom" => Some(Self::Bottom),
            "isometric" | "iso" => Some(Self::Isometric),
            _ => None,
        }
    }

    /// Get all available angle names
    pub fn all_names() -> &'static [&'static str] {
        &["front", "back", "left", "right", "top", "bottom", "isometric"]
    }
}

impl std::str::FromStr for CameraAngle {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or_else(|| {
            format!(
                "Unknown camera angle '{}'. Valid options: {}",
                s,
                Self::all_names().join(", ")
            )
        })
    }
}

impl std::fmt::Display for CameraAngle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::Front => "front",
            Self::Back => "back",
            Self::Left => "left",
            Self::Right => "right",
            Self::Top => "top",
            Self::Bottom => "bottom",
            Self::Isometric => "isometric",
        };
        write!(f, "{name}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_angles() {
        assert_eq!(CameraAngle::parse("front"), Some(CameraAngle::Front));
        assert_eq!(CameraAngle::parse("FRONT"), Some(CameraAngle::Front));
        assert_eq!(CameraAngle::parse("iso"), Some(CameraAngle::Isometric));
        assert_eq!(CameraAngle::parse("invalid"), None);
    }

    #[test]
    fn test_to_camera() {
        let angle = CameraAngle::Isometric;
        let camera = angle.to_camera(Vec3::ZERO, 1.0);

        // Camera should be looking at origin
        assert_eq!(camera.target, Vec3::ZERO);

        // Camera should be positioned away from origin
        assert!(camera.position.length() > 1.0);
    }
}
