use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
    pub eye_pos: [f32; 3],
    pub _pad: f32,
}

pub struct OrbitCamera {
    pub center: Vec3,
    pub distance: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub fov_y: f32,
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
}

impl OrbitCamera {
    pub fn new(aspect: f32) -> Self {
        Self {
            center: Vec3::new(0.275, 0.275, 0.275),
            distance: 0.8,
            yaw: std::f32::consts::FRAC_PI_4,
            pitch: 0.4,
            fov_y: 45.0_f32.to_radians(),
            aspect,
            near: 0.001,
            far: 10.0,
        }
    }

    pub fn eye_position(&self) -> Vec3 {
        let cos_pitch = self.pitch.cos();
        let eye = Vec3::new(
            self.center.x + self.distance * cos_pitch * self.yaw.sin(),
            self.center.y + self.distance * self.pitch.sin(),
            self.center.z + self.distance * cos_pitch * self.yaw.cos(),
        );
        eye
    }

    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.eye_position(), self.center, Vec3::Y)
    }

    pub fn proj_matrix(&self) -> Mat4 {
        Mat4::perspective_rh(self.fov_y, self.aspect, self.near, self.far)
    }

    pub fn uniform(&self) -> CameraUniform {
        let vp = self.proj_matrix() * self.view_matrix();
        let eye = self.eye_position();
        CameraUniform {
            view_proj: vp.to_cols_array_2d(),
            eye_pos: eye.to_array(),
            _pad: 0.0,
        }
    }

    pub fn rotate(&mut self, delta_yaw: f32, delta_pitch: f32) {
        self.yaw += delta_yaw;
        self.pitch = (self.pitch + delta_pitch).clamp(-1.5, 1.5);
    }

    pub fn zoom(&mut self, delta: f32) {
        self.distance = (self.distance * (1.0 - delta * 0.1)).clamp(0.05, 5.0);
    }

    pub fn pan(&mut self, dx: f32, dy: f32) {
        let right = Vec3::new(self.yaw.cos(), 0.0, -self.yaw.sin());
        let up = Vec3::Y;
        let scale = self.distance * 0.002;
        self.center += right * dx * scale + up * dy * scale;
    }
}
