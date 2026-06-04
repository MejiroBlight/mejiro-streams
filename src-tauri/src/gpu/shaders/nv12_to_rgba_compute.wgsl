// 1本のフラットなNV12生データをストレージバッファ（4バイト単位）として受け取る
@group(0) @binding(0) var<storage, read> nv12_buffer: array<u32>;

// 書き込み先の一括RGBAテクスチャ
@group(0) @binding(1) var output_rgba_texture: texture_storage_2d<rgba8unorm, write>;

// 変換に必要なパラメータを一括で渡すためのユニフォームバッファ
@group(0) @binding(2) var<uniform> config: vec4<u32>; 

// 1つのスレッドグループを 16 × 16 ＝ 256スレッド のタイル状に配置
@compute @workgroup_size(16, 16)
fn cs_main(@builtin(global_invocation_id) id: vec3<u32>) {
    let dims = textureDimensions(output_rgba_texture);
    let x = id.x;
    let y = id.y;

    // 画面外（解像度の端の余り）のピクセルを担当するスレッドは即座に終了
    if (x >= dims.x || y >= dims.y) {
        return;
    }

    let width = dims.x;
    let height = dims.y;
    let stride = config.x; // 1行あたりのNV12バイト数（通常は width * 1.5）

    // ----------------------------------------------------
    // 1. Yプレーン（輝度）から該当ピクセルのデータを抽出
    // ----------------------------------------------------
    
    let y_byte_idx = y * stride + x; 
    let y_u32_idx = y_byte_idx / 4u;
    let y_shift = (y_byte_idx % 4u) * 8u;
    
    let y_raw = (nv12_buffer[y_u32_idx] >> y_shift) & 0xffu;
    let Y = f32(y_raw) / 255.0;

    // ----------------------------------------------------
    // 2. UVプレーン（色差）から該当ピクセルのデータを抽出
    // ----------------------------------------------------
    
    
    let uv_start = stride * height; 
    let uv_row = y / 2u;
    let uv_col = (x / 2u) * 2u;
    
    // 行の進み幅を stride に変更
    let u_byte_idx = uv_start + (uv_row * stride) + uv_col;
    let v_byte_idx = u_byte_idx + 1u;

    // U(青み)のビット抽出
    let u_raw = (nv12_buffer[u_byte_idx / 4u] >> ((u_byte_idx % 4u) * 8u)) & 0xffu;
    // V(赤み)のビット抽出
    let v_raw = (nv12_buffer[v_byte_idx / 4u] >> ((v_byte_idx % 4u) * 8u)) & 0xffu;

    // YUV色空間において、UとVは 0.5 (128) が色のないニュートラルな中心
    let U = f32(u_raw) / 255.0 - 0.5;
    let V = f32(v_raw) / 255.0 - 0.5;

    // ----------------------------------------------------
    // 3. BT.709 カラーマトリクスによる正確なカラー変換
    // ----------------------------------------------------
    let r = Y + 1.5748 * V;
    let g = Y - 0.1873 * U - 0.4681 * V;
    let b = Y + 1.8556 * U;

    // 4. 指定されたテクスチャ座標にRGBAの値を直接格納（サンプラーを一切介さないためブレない）
    textureStore(output_rgba_texture, vec2<i32>(i32(x), i32(y)), vec4<f32>(r, g, b, 1.0));
}