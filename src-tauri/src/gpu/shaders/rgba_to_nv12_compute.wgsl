// 入力RGBAテクスチャ (読み取り専用: 1920x1080)
@group(0) @binding(0) var input_rgba: texture_2d<f32>;

// 出力先テクスチャ群 (r32uint型: 480x1080 / 480x540)
@group(0) @binding(1) var output_y: texture_storage_2d<r32uint, write>;
@group(0) @binding(2) var output_uv: texture_storage_2d<r32uint, write>;

// 16x16のピクセルブロック単位で並列処理
@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let tx = id.x; // 0 〜 479
    let ty = id.y; // 0 〜 1079
    
    let rgba_dims = textureDimensions(input_rgba);
    
    // 安全のため、テクスチャの範囲外スレッドを弾く
    if (tx >= rgba_dims.x / 4u || ty >= rgba_dims.y) { return; }

    // 1スレッドで連続する4つのRGBAピクセルを処理
    let base_x = tx * 4u;
    
    let rgba0 = textureLoad(input_rgba, vec2<u32>(base_x + 0u, ty), 0);
    let rgba1 = textureLoad(input_rgba, vec2<u32>(base_x + 1u, ty), 0);
    let rgba2 = textureLoad(input_rgba, vec2<u32>(base_x + 2u, ty), 0);
    let rgba3 = textureLoad(input_rgba, vec2<u32>(base_x + 3u, ty), 0);

    // --- Y成分の計算 (BT.709) ---
    let y0 = u32(clamp(0.2126 * rgba0.r + 0.7152 * rgba0.g + 0.0722 * rgba0.b, 0.0, 1.0) * 255.0);
    let y1 = u32(clamp(0.2126 * rgba1.r + 0.7152 * rgba1.g + 0.0722 * rgba1.b, 0.0, 1.0) * 255.0);
    let y2 = u32(clamp(0.2126 * rgba2.r + 0.7152 * rgba2.g + 0.0722 * rgba2.b, 0.0, 1.0) * 255.0);
    let y3 = u32(clamp(0.2126 * rgba3.r + 0.7152 * rgba3.g + 0.0722 * rgba3.b, 0.0, 1.0) * 255.0);

    // 4バイトを1つのu32に結合してYテクスチャへ書き込み
    let packed_y = y0 | (y1 << 8u) | (y2 << 16u) | (y3 << 24u);
    textureStore(output_y, vec2<u32>(tx, ty), vec4<u32>(packed_y, 0u, 0u, 0u));

    // --- UV成分の計算 (縦方向が偶数行のときだけ、横2つのUVペアを処理) ---
    if (ty % 2u == 0u) {
        // base_x から始まる4ピクセルのうち、ピクセル0 と ピクセル2 を代表点として計算
        let u0 = u32(clamp(-0.1146 * rgba0.r - 0.3854 * rgba0.g + 0.5000 * rgba0.b + 0.5, 0.0, 1.0) * 255.0);
        let v0 = u32(clamp(0.5000 * rgba0.r - 0.4542 * rgba0.g - 0.0458 * rgba0.b + 0.5, 0.0, 1.0) * 255.0);
        
        let u1 = u32(clamp(-0.1146 * rgba2.r - 0.3854 * rgba2.g + 0.5000 * rgba2.b + 0.5, 0.0, 1.0) * 255.0);
        let v1 = u32(clamp(0.5000 * rgba2.r - 0.4542 * rgba2.g - 0.0458 * rgba2.b + 0.5, 0.0, 1.0) * 255.0);

        // 4バイト [U0, V0, U1, V1] を1つのu32にパッキングしてUVテクスチャへ書き込み
        let packed_uv = u0 | (v0 << 8u) | (u1 << 16u) | (v1 << 24u);
        textureStore(output_uv, vec2<u32>(tx, ty / 2u), vec4<u32>(packed_uv, 0u, 0u, 0u));
    }
}