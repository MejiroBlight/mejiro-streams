struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

// 頂点シェーダー（これまでの共通フルスクリーン描画）
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 4>(
        vec2<f32>(-1.0,  1.0), vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0,  1.0), vec2<f32>( 1.0, -1.0)
    );
    var uvs = array<vec2<f32>, 4>(
        vec2<f32>(0.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0)
    );

    var out: VertexOutput;
    out.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    out.uv = uvs[vertex_index];
    return out;
}

// ----------------------------------------------------
// フラグメントシェーダー
// ----------------------------------------------------
@group(0) @binding(0) var texture_rgba: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

// MRT（複数同時出力）のための構造体
struct FragmentOutput {
    @location(0) output_y: vec4<f32>,  // 器は R8Unorm (x成分だけが使われる)
    @location(1) output_uv: vec4<f32>, // 器は Rg8Unorm (xy成分が使われる)
}

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    // 1. 現在のピクセルのRGBA色を取得
    let rgba = textureSample(texture_rgba, texture_sampler, in.uv);
    let r = rgba.r;
    let g = rgba.g;
    let b = rgba.b;

    // 2. RGB ➔ YUV 変換式 (BT.709の例)
    // Yは等倍なので、現在のピクセルの色からそのまま計算
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;

    // 3. UとVの計算
    // NV12のUVは「縦横半分（4ピクセルを1ピクセルに間引く）」にする必要があります。
    // wgpuのサンプラー（Linear設定）の力を借りると、縮小されたUVテクスチャの
    // 各ピクセルの中心からサンプリングした時点で、自動的に周囲4ピクセルの平均的なRGBAが得られます。
    let u = -0.1146 * r - 0.3854 * g + 0.5000 * b + 0.5;
    let v =  0.5000 * r - 0.4542 * g - 0.0458 * b + 0.5;

    // 4. それぞれの出力ターゲットへ書き出す
    var out: FragmentOutput;
    // R8Unormへ書き出すので、x（赤）チャンネルにYを入れる
    out.output_y = vec4<f32>(y, 0.0, 0.0, 1.0);
    // Rg8Unormへ書き出すので、xにU、yにVを入れる
    out.output_uv = vec4<f32>(u, v, 0.0, 1.0);

    return out;
}