//! Shared camera controller for interactive preview windows

// Input state tracks multiple mouse buttons independently
#![allow(clippy::struct_excessive_bools)]

use crate::camera::Camera;
use glam::Vec3;
use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, Modifiers, MouseButton, MouseScrollDelta},
};

/// Input state for camera control
#[derive(Debug, Default)]
pub struct InputState {
    pub mouse_left: bool,
    pub mouse_right: bool,
    pub mouse_middle: bool,
    pub last_mouse_pos: Option<PhysicalPosition<f64>>,
    pub shift_held: bool,
}

/// Camera controller shared by all preview window types
pub struct CameraController {
    pub camera: Camera,
    pub input: InputState,
}

impl CameraController {
    pub fn new() -> Self {
        Self {
            camera: Camera::default(),
            input: InputState::default(),
        }
    }

    pub fn handle_mouse_motion(&mut self, position: PhysicalPosition<f64>) {
        if let Some(last_pos) = self.input.last_mouse_pos {
            let dx = (position.x - last_pos.x) as f32 * 0.005;
            let dy = (position.y - last_pos.y) as f32 * 0.005;

            if self.input.mouse_right || (self.input.mouse_left && self.input.shift_held) {
                self.camera.pan(-dx * 2.0, dy * 2.0);
            } else if self.input.mouse_left {
                self.camera.orbit(dx, dy);
            } else if self.input.mouse_middle {
                self.camera.zoom(dy * 5.0);
            }
        }
        self.input.last_mouse_pos = Some(position);
    }

    pub fn handle_scroll(&mut self, delta: MouseScrollDelta) {
        let scroll = match delta {
            MouseScrollDelta::LineDelta(_, y) => y,
            MouseScrollDelta::PixelDelta(pos) => pos.y as f32 * 0.01,
        };
        self.camera.zoom(scroll * 0.5);
    }

    pub fn handle_mouse_button(&mut self, state: ElementState, button: MouseButton) {
        let pressed = state == ElementState::Pressed;
        match button {
            MouseButton::Left => self.input.mouse_left = pressed,
            MouseButton::Right => self.input.mouse_right = pressed,
            MouseButton::Middle => self.input.mouse_middle = pressed,
            _ => {}
        }
    }

    pub fn handle_modifiers(&mut self, modifiers: &Modifiers) {
        self.input.shift_held = modifiers.state().shift_key();
    }

    pub fn reset_camera(&mut self) {
        let aspect = self.camera.aspect;
        self.camera = Camera::default();
        self.camera.aspect = aspect;
    }

    pub fn focus_origin(&mut self) {
        self.camera.target = Vec3::ZERO;
    }
}

impl Default for CameraController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shift_left_drag_pans_instead_of_orbiting() {
        let mut controller = CameraController::new();
        let initial_target = controller.camera.target;
        let initial_distance = controller.camera.distance();

        controller.input.mouse_left = true;
        controller.input.shift_held = true;
        controller.input.last_mouse_pos = Some(PhysicalPosition::new(100.0, 100.0));

        controller.handle_mouse_motion(PhysicalPosition::new(120.0, 110.0));

        assert_ne!(controller.camera.target, initial_target);
        assert!((controller.camera.distance() - initial_distance).abs() < 0.001);
    }

    #[test]
    fn left_drag_orbits_without_panning_target() {
        let mut controller = CameraController::new();
        let initial_target = controller.camera.target;
        let initial_position = controller.camera.position;

        controller.input.mouse_left = true;
        controller.input.last_mouse_pos = Some(PhysicalPosition::new(100.0, 100.0));

        controller.handle_mouse_motion(PhysicalPosition::new(120.0, 110.0));

        assert_eq!(controller.camera.target, initial_target);
        assert_ne!(controller.camera.position, initial_position);
    }
}
