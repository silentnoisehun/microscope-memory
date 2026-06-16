// Microscope Memory — GPU compute shader for 4D L2 soft search
//
// Buffer layout:
//   positions: array<vec4<f32>>  — (x, y, z, zoom) per block, tightly packed
//   query:     Query uniform     — (x, y, z, qz, zoom_weight)
//   distances: array<f32>        — output distance² per block

struct Query {
    x: f32,
    y: f32,
    z: f32,
    qz: f32,
    zw: f32,
}

@group(0) @binding(0)
var<storage, read> positions: array<vec4<f32>>;

@group(0) @binding(1)
var<uniform> query: Query;

@group(0) @binding(2)
var<storage, read_write> distances: array<f32>;

@compute @workgroup_size(64)
fn l2_4d(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= arrayLength(&positions)) {
        return;
    }

    let p = positions[i];
    let dx = p.x - query.x;
    let dy = p.y - query.y;
    let dz = p.z - query.z;
    let dw = (p.w - query.qz) * query.zw;

    distances[i] = dx * dx + dy * dy + dz * dz + dw * dw;
}
