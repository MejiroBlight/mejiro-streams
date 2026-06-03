// 頂点シェーダーからフラグメントシェーダーへ渡すデータの定義
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

// ----------------------------------------------------
// 頂点シェーダー (Vertex Shader)
// ----------------------------------------------------
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // 頂点バッファを使わず、インデックス(0, 1, 2, 3)から
    // 画面全体を覆う四角形（2つの三角形）の座標を自動生成する
    var positions = array<vec2<f32>, 4>(
        vec2<f32>(-1.0,  1.0), // 左上
        vec2<f32>(-1.0, -1.0), // 左下
        vec2<f32>( 1.0,  1.0), // 右上
        vec2<f32>( 1.0, -1.0)  // 右下
    );

    // 座標に対応するテクスチャのUV座標 (Y軸の向きに注意)
    var uvs = array<vec2<f32>, 4>(
        vec2<f32>(0.0, 0.0), // 左上
        vec2<f32>(0.0, 1.0), // 左下
        vec2<f32>(1.0, 0.0), // 右上
        vec2<f32>(1.0, 1.0)  // 右下
    );

    var out: VertexOutput;
    out.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    out.uv = uvs[vertex_index];
    return out;
}

// ----------------------------------------------------
// フラグメントシェーダー (Fragment Shader)
// ----------------------------------------------------
@group(0) @binding(0) var texture_y: texture_2d<f32>;
@group(0) @binding(1) var texture_uv: texture_2d<f32>;
@group(0) @binding(2) var texture_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Y（輝度）を取得
    let y = textureSample(texture_y, texture_sampler, in.uv).r;
    
    // UV（色差）を取得
    let uv = textureSample(texture_uv, texture_sampler, in.uv).rg;
    let u = uv.x - 0.5;
    let v = uv.y - 0.5;

    // BT.709 変換式
    let r = y + 1.5748 * v;
    let g = y - 0.1873 * u - 0.4681 * v;
    let b = y + 1.8556 * u;

    return vec4<f32>(r, g, b, 1.0);
}