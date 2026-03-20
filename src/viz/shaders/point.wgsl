struct CameraUniforms {
    view_proj: mat4x4<f32>,
    eye_pos: vec3<f32>,
    _pad: f32,
};

@group(0) @binding(0) var<uniform> camera: CameraUniforms;

struct InstanceInput {
    @location(0) position: vec3<f32>,
    @location(1) size: f32,
    @location(2) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
};

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    instance: InstanceInput,
) -> VertexOutput {
    let offsets = array<vec2<f32>, 4>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 1.0,  1.0),
    );
    let corner = offsets[vertex_index % 4u];

    let forward = normalize(camera.eye_pos - instance.position);
    let world_up = vec3<f32>(0.0, 1.0, 0.0);
    var right = cross(world_up, forward);
    if length(right) < 0.001 {
        right = vec3<f32>(1.0, 0.0, 0.0);
    }
    right = normalize(right);
    let up = normalize(cross(forward, right));

    let world_pos = instance.position
        + right * corner.x * instance.size
        + up * corner.y * instance.size;

    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);
    out.color = instance.color;
    out.uv = corner * 0.5 + 0.5;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let dist = length(in.uv - vec2<f32>(0.5));
    if dist > 0.5 {
        discard;
    }
    let alpha = smoothstep(0.5, 0.35, dist) * in.color.a;
    return vec4<f32>(in.color.rgb, alpha);
}
