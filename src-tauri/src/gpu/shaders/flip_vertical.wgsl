struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 4>(
        vec2<f32>(-1.0,  1.0), // 左上
        vec2<f32>(-1.0, -1.0), // 左下
        vec2<f32>( 1.0,  1.0), // 右上
        vec2<f32>( 1.0, -1.0)  // 右下
    );

    var uvs = array<vec2<f32>, 4>(
        vec2<f32>(0.0, 0.0), // 左上
        vec2<f32>(0.0, 1.0), // 左下
        vec2<f32>(1.0, 0.0), // 右上
        vec2<f32>(1.0, 1.0)  // 右下
    );

    var out: VertexOutput;
    out.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    
    // ★ここで上下反転！ Y座標（uv.y）を 1.0 から引くことで上下が逆になります
    out.uv = vec2<f32>(uvs[vertex_index].x, 1.0 - uvs[vertex_index].y);
    
    return out;
}

@group(0) @binding(0) var texture_rgba: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // 反転したUV座標を使って、元のRGBAテクスチャから色をそのままサンプリングする
    return textureSample(texture_rgba, texture_sampler, in.uv);
}