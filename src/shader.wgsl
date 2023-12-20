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
    @location(3) velocity: vec2<f32>,
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
    let pos = instance.position + model.position * 0.5;

    out.local_pos = model.position;
    out.position = camera.transform * vec4<f32>(pos, 0.0, 1.0);
    out.color = vec3<f32>(instance.velocity, 1.0);

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let center = in.local_pos;
    let dist = center.x * center.x + center.y * center.y;
    let outer_alpha = smoothstep(0.0, 0.01, 1.0 - dist);
    let inner_alpha = smoothstep(0.01, 0.0, 0.90 - dist);
    return vec4(in.color, outer_alpha * inner_alpha);
}