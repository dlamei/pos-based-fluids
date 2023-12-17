// Vertex shader

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) color: u32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
};

@vertex
fn vs_main(
    model: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = vec4<f32>(model.position, 0.0, 1.0);

    let r = (model.color >> 16u);
    let g = (model.color >> 8u ) & 0xffu;
    let b = (model.color       ) & 0xffu;
    let color = vec3(f32(r), f32(g), f32(b)) / 255.0;
    out.color = color;

    return out;
}

// Fragment shader

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color, 1.0);
}