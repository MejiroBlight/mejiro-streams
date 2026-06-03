// Composite shader: sample input texture and output to render target

@group(0) @binding(0)
var input_texture: texture_2d<f32>;

@group(0) @binding(1)
var input_sampler: sampler;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
};

/// Generates a fullscreen quad (triangle strip, 4 vertices) without a vertex buffer.
/// Vertex order: top-left, top-right, bottom-left, bottom-right
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let xi = vertex_index & 1u;        // 0, 1, 0, 1
    let yi = vertex_index >> 1u;       // 0, 0, 1, 1

    let x = f32(i32(xi) * 2 - 1);     // -1, 1, -1, 1
    let y = f32(1 - i32(yi) * 2);     //  1, 1, -1, -1

    var out: VertexOutput;
    out.clip_position = vec4<f32>(x, y, 0.0, 1.0);
    out.tex_coords = vec2<f32>(f32(xi), f32(yi));
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(input_texture, input_sampler, in.tex_coords);
}
