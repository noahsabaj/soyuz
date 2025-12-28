//! Camera controls for the renderer

use glam::{Mat4, Vec3};

/// A simple orbital camera that orbits around a target point
#[derive(Debug, Clone)]
pub struct Camera {
    /// Camera position in world space
    pub position: Vec3,
    /// Point the camera is looking at
    pub target: Vec3,
    /// Up vector (usually Y-up)
    pub up: Vec3,
    /// Field of view in radians
    pub fov: f32,
    /// Aspect ratio (width / height)
    pub aspect: f32,
    /// Near clipping plane
    pub near: f32,
    /// Far clipping plane
    pub far: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            position: Vec3::new(2.0, 1.5, 2.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
            fov: 45.0_f32.to_radians(),
            aspect: 16.0 / 9.0,
            near: 0.01,
            far: 100.0,
        }
    }
}

impl Camera {
    /// Create a new camera with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a camera looking at a target from a position
    pub fn look_at(position: Vec3, target: Vec3) -> Self {
        Self {
            position,
            target,
            ..Default::default()
        }
    }

    /// Get the view matrix (world to camera transform)
    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.position, self.target, self.up)
    }

    /// Get the projection matrix
    pub fn projection_matrix(&self) -> Mat4 {
        Mat4::perspective_rh(self.fov, self.aspect, self.near, self.far)
    }

    /// Get the combined view-projection matrix
    pub fn view_projection_matrix(&self) -> Mat4 {
        self.projection_matrix() * self.view_matrix()
    }

    /// Get the forward direction (normalized)
    pub fn forward(&self) -> Vec3 {
        (self.target - self.position).normalize()
    }

    /// Get the right direction (normalized)
    pub fn right(&self) -> Vec3 {
        self.forward().cross(self.up).normalize()
    }

    /// Get the actual up direction (may differ from self.up due to camera orientation)
    pub fn actual_up(&self) -> Vec3 {
        self.right().cross(self.forward())
    }

    /// Get distance from camera to target
    pub fn distance(&self) -> f32 {
        (self.position - self.target).length()
    }

    /// Orbit around the target point
    ///
    /// - `delta_x`: Horizontal rotation (positive = rotate right)
    /// - `delta_y`: Vertical rotation (positive = rotate up, drag down to see top)
    pub fn orbit(&mut self, delta_x: f32, delta_y: f32) {
        let radius = self.distance();

        // Get spherical coordinates
        let offset = self.position - self.target;
        let mut theta = offset.x.atan2(offset.z);
        let mut phi = (offset.y / radius).clamp(-0.999, 0.999).acos();

        // Apply rotation (inverted Y for natural drag: drag down = look at top)
        theta -= delta_x;
        phi = (phi - delta_y).clamp(0.01, std::f32::consts::PI - 0.01);

        // Convert back to cartesian
        self.position = self.target
            + Vec3::new(
                radius * phi.sin() * theta.sin(),
                radius * phi.cos(),
                radius * phi.sin() * theta.cos(),
            );
    }

    /// Zoom in/out (move camera closer/farther from target)
    ///
    /// - `delta`: Positive = zoom in, negative = zoom out
    pub fn zoom(&mut self, delta: f32) {
        let dir = (self.position - self.target).normalize();
        let distance = self.distance();
        let new_distance = (distance - delta).clamp(0.1, 100.0);
        self.position = self.target + dir * new_distance;
    }

    /// Pan the camera (move both position and target)
    ///
    /// - `delta_x`: Horizontal pan (positive = move right)
    /// - `delta_y`: Vertical pan (positive = move up)
    pub fn pan(&mut self, delta_x: f32, delta_y: f32) {
        let right = self.right();
        let up = self.actual_up();

        let offset = right * delta_x + up * delta_y;
        self.position += offset;
        self.target += offset;
    }

    /// Set the camera to look at a specific point from a specific position
    pub fn set_look_at(&mut self, position: Vec3, target: Vec3) {
        self.position = position;
        self.target = target;
    }

    /// Reset to default position
    pub fn reset(&mut self) {
        let aspect = self.aspect; // Save aspect ratio
        *self = Self::default();
        self.aspect = aspect; // Restore aspect ratio
    }

    /// Frame a bounding box (adjust camera to see the entire object)
    pub fn frame_bounds(&mut self, min: Vec3, max: Vec3, padding: f32) {
        let center = (min + max) * 0.5;
        let size = (max - min).max_element();
        let distance = (size * 0.5 * (1.0 + padding)) / (self.fov * 0.5).tan();

        self.target = center;
        self.position = center + Vec3::new(distance * 0.7, distance * 0.5, distance * 0.7);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_camera_orbit() {
        let mut camera = Camera::default();
        let initial_distance = camera.distance();

        camera.orbit(0.1, 0.0);

        // Distance should remain the same
        assert!((camera.distance() - initial_distance).abs() < 0.001);
    }

    #[test]
    fn test_camera_zoom() {
        let mut camera = Camera::default();
        let initial_distance = camera.distance();

        camera.zoom(0.5);

        assert!(camera.distance() < initial_distance);
    }

    #[test]
    fn test_camera_pan() {
        let mut camera = Camera::default();
        let initial_target = camera.target;

        camera.pan(1.0, 0.0);

        assert_ne!(camera.target, initial_target);
    }
}
