use glam::{Mat4, Vec3, Vec4};
use crate::viz::scene::PointInstance;

/// Ray-sphere intersection for click-to-select.
/// Returns the index into `instances` (and thus `block_indices`) of the closest hit.
pub fn pick_point(
    mouse_x: f32,
    mouse_y: f32,
    screen_width: f32,
    screen_height: f32,
    view_proj: &Mat4,
    instances: &[PointInstance],
) -> Option<usize> {
    // Normalized device coordinates [-1, 1]
    let ndc_x = (2.0 * mouse_x / screen_width) - 1.0;
    let ndc_y = 1.0 - (2.0 * mouse_y / screen_height);

    let inv_vp = view_proj.inverse();

    // Unproject near and far
    let near_point = inv_vp * Vec4::new(ndc_x, ndc_y, 0.0, 1.0);
    let far_point = inv_vp * Vec4::new(ndc_x, ndc_y, 1.0, 1.0);

    let near = Vec3::new(near_point.x, near_point.y, near_point.z) / near_point.w;
    let far = Vec3::new(far_point.x, far_point.y, far_point.z) / far_point.w;

    let ray_dir = (far - near).normalize();
    let ray_origin = near;

    let mut best_t = f32::MAX;
    let mut best_idx: Option<usize> = None;

    for (i, inst) in instances.iter().enumerate() {
        let center = Vec3::from_array(inst.position);
        let radius = inst.size * 1.5; // slightly larger hit area

        // Ray-sphere intersection
        let oc = ray_origin - center;
        let b = oc.dot(ray_dir);
        let c = oc.dot(oc) - radius * radius;
        let discriminant = b * b - c;

        if discriminant >= 0.0 {
            let t = -b - discriminant.sqrt();
            if t > 0.0 && t < best_t {
                best_t = t;
                best_idx = Some(i);
            }
        }
    }

    best_idx
}
