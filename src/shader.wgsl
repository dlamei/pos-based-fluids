struct CameraUniform {
    transform: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct VertexInput {
    @location(0) position: vec2<f32>,
};

struct InstanceInput {
    @location(2) position: vec2<f32>,
    @location(3) scale: f32,
    @location(4) color: u32,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(1) local_pos: vec2<f32>,
    @location(0) color: vec3<f32>,
};

@vertex
fn vs_main(
    model: VertexInput,
    instance: InstanceInput,
) -> VertexOutput {
    var out: VertexOutput;
    let pos = instance.position + model.position * instance.scale;

    let r = (instance.color >> 16u);
    let g = (instance.color >> 8u ) & 0xffu;
    let b = (instance.color       ) & 0xffu;

    out.local_pos = model.position;
    out.position = camera.transform * vec4<f32>(pos, 0.0, 1.0);
    out.color = vec3(f32(r), f32(g), f32(b)) / 255.0;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let center = in.local_pos;
    let dist = center.x * center.x + center.y * center.y;
    let alpha = smoothstep(0.0, 0.01, 1.0 - dist);
    return vec4(in.color, alpha);
}