<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onDestroy, onMount } from "svelte";
  import type { VideoInfo} from "../lib/bindings";
  import {commands} from "../lib/bindings";
  import {open} from "@tauri-apps/plugin-dialog";
    import { Channel } from "@tauri-apps/api/core";

  // ---------------------------------------------------------------------------
  // State
  // ---------------------------------------------------------------------------

  let videoInfo = $state<VideoInfo | null>(null);
  let currentMs = $state(0);
  let previewSrc = $state("");
  let isLoading = $state(false);
  let errorMsg = $state("");
  let filename = $state("");

  //const ctx = canvas.getContext('2d')!;

  // ---------------------------------------------------------------------------
  // Helpers
  // ---------------------------------------------------------------------------
  function msToTimecode(ms: number): string {
    const h = Math.floor(ms / 3_600_000);
    const m = Math.floor((ms % 3_600_000) / 60_000);
    const s = Math.floor((ms % 60_000) / 1_000);
    const f = Math.floor((ms % 1_000) / (1000 / 30)); // approx 30fps frame number
    return `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}.${String(f).padStart(2, "0")}`;
  }

  async function loadVideo(path: string) {
    isLoading = true;
    errorMsg = "";
    try {
      const info = await commands.loadVideoPath(path);
      if (info.status !== "ok") {
        throw new Error(info.error);
      }
      videoInfo = info.data;
      currentMs = 0;
      filename = path.split(/[\\/]/).pop() ?? path;
      await fetchFrame(0);
    } catch (e) {
      errorMsg = String(e);
    } finally {
      isLoading = false;
    }
  }

  async function fetchFrame(ms: number) {
    try {
      const url = await commands.seekFrame(ms);
      if (url.status !== "ok") {
        throw new Error(url.error);
      }
      // Append a random cache-bust query param in case the browser caches identical URLs
      previewSrc = url.data + "&_cb=" + Date.now();
    } catch (e) {
      errorMsg = String(e);
    }
  }

  // Debounced seek – avoids flooding the Rust side while dragging
  let seekTimer: ReturnType<typeof setTimeout> | null = null;
  function onSeek(e: Event) {
    const ms = Number((e.target as HTMLInputElement).value);
    currentMs = ms;
    if (seekTimer) clearTimeout(seekTimer);
    seekTimer = setTimeout(() => fetchFrame(ms), 80);
  }

  // ---------------------------------------------------------------------------
  // Drag-and-drop (Tauri window-level)
  // ---------------------------------------------------------------------------
  onMount(() => {
    /* const appWindow = getCurrentWindow();
    appWindow.onDragDropEvent(async (event) => {
      if (event.payload.type === "drop") {
        const paths: string[] = event.payload.paths;
        if (paths.length > 0) {
          await loadVideo(paths[0]);
        }
      }
    }).then(unlisten => {
      onDestroy(() => {
        unlisten();
      });
    }); */

    startNv12Stream();
  });

  // ---------------------------------------------------------------------------
  // Open-file button
  // ---------------------------------------------------------------------------
  async function openFile() {
    try {
      const selected = await open({
        multiple: false,
        filters: [
          { name: "Video Files", extensions: ["mp4", "mkv", "avi", "mov", "webm"] },
          { name: "All Files", extensions: ["*"] }
        ]
      });
      if (typeof selected === "string") {
        await loadVideo(selected);
      }
    } catch (e) {
      // User probably cancelled the dialog, so we can ignore errors
    }
  }


  // 1. Canvas と描画コンテキストの取得
  

  async function startNv12Stream() {
    const canvas = document.getElementById('video-canvas') as HTMLCanvasElement;
    console.log(canvas);

    // 動画の解像度（Rust側の wgpu 出力サイズと完全に一致させてください）
    const width = 1920;
    const height = 1080;
    
    // Canvasのサイズをあらかじめ動画に合わせておく
    canvas.width = width;
    canvas.height = height;

    // 2. Tauri v2 の IPC チャンネルを作成
    const onFrameChannel = new Channel<number[]>();

    const ctx = canvas.getContext('2d')!;
    
    // 3. 毎フレームのバイナリが届いたときの処理
    onFrameChannel.onmessage = (nv12Binary: number[]) => {
      try {
        // 1920 を 256 の倍数に切り上げる計算（Rustの aligned_stride と同じにする）
      // 1920 なら 2048、1280 なら 1280 になります。
      const alignedWidth = (width + 255) & ~255; 

      const frame = new VideoFrame(new Uint8Array(nv12Binary), {
        format: 'NV12',
        
        // ★ 最重要修正：データの実体幅（パディング込みの幅）を教える
        codedWidth: alignedWidth,  // ➔ 2048
        codedHeight: height,       // ➔ 1080

        // ★ 追加：ただし、実際に画面に見せる（切り出す）サイズは本来の動画サイズだよ、と指定する
        // これを入れることで、右側の128バイト分の黒いゴミ余白が自動的にカットされて描画されます。
        displayWidth: width,  // ➔ 1920
        displayHeight: height,// ➔ 1080

        timestamp: performance.now() * 1000,
        colorSpace: {
          matrix: 'bt709',
          primaries: 'bt709',
          transfer: 'bt709',
          fullRange: false,
        }
      });

        // 5. Canvas に描画
        // 2D Canvas の drawImage は、VideoFrame をそのまま画像ソースとして受け付けます。
        // ブラウザ内部の GPU が、超高速に NV12 ➔ RGBA への色変換を行ってくれます。
        ctx.drawImage(frame, 0, 0, width, height);

        // 6. ★最重要★ メモリ解放
        // VideoFrame は GPU のリソースと直結しているため、描画直後に必ず close() を呼びます。
        // これを忘れると数秒でブラウザ（WebView）がメモリ不足でクラッシュします。
        frame.close();
        console.log('フレームを描画しました');
      } catch (error) {
        console.error('VideoFrame の生成または描画に失敗しました:', error);
      }
    };

    // 7. バックエンドのストリーミング開始コマンドを呼び出し、チャンネルを渡す
    try {
      await commands.startFrameServer(onFrameChannel);
      console.log('NV12 ストリーミングを開始しました');
    } catch (err) {
      console.error('バックエンドの起動に失敗:', err);
    }
  }


</script>

<!-- ========================================================================
     Markup
     ======================================================================== -->
<div class="app">

  <!-- Toolbar -->
  <header class="toolbar">
    <span class="app-title">mejiro streams</span>
    <button class="btn-open" onclick={openFile} disabled={isLoading}>
      {isLoading ? "Loading…" : "Open Video"}
    </button>
    {#if filename}
      <span class="filename">{filename}</span>
    {/if}
    {#if videoInfo}
      <span class="meta">{videoInfo.width}×{videoInfo.height}</span>
    {/if}
  </header>

  <!-- Preview area -->
  <main
    class="preview-area"
    class:drop-zone={!videoInfo}
    ondragover={(e) => e.preventDefault()}
  >

    <canvas id="video-canvas"></canvas>

    <!-- {#if previewSrc}
      
    {:else}
      <div class="drop-hint">
        <svg width="64" height="64" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
          <polygon points="23 7 16 12 23 17 23 7"/>
          <rect x="1" y="5" width="15" height="14" rx="2" ry="2"/>
        </svg>
        <p>動画ファイルをドロップ<br/>または「Open Video」ボタン</p>
      </div>
    {/if}
    {#if errorMsg}
      <div class="error-banner">{errorMsg}</div>
    {/if} -->
  </main>

  <!-- Timeline / seekbar -->
  <footer class="timeline">
    {#if videoInfo}
      <span class="timecode">{msToTimecode(currentMs)}</span>
      <input
        class="seekbar"
        type="range"
        min="0"
        max={videoInfo.duration_ms}
        value={currentMs}
        oninput={onSeek}
      />
      <span class="timecode">{msToTimecode(videoInfo.duration_ms)}</span>
    {:else}
      <span class="timecode-placeholder">--:--:--.--</span>
      <div class="seekbar seekbar--disabled"></div>
      <span class="timecode-placeholder">--:--:--.--</span>
    {/if}
  </footer>

</div>

<!-- ========================================================================
     Styles
     ======================================================================== -->
<style>
  :global(*, *::before, *::after) {
    box-sizing: border-box;
    margin: 0;
    padding: 0;
  }

  :global(body) {
    background: #0d0d0d;
    color: #e0e0e0;
    font-family: "Inter", "Segoe UI", sans-serif;
    font-size: 13px;
    height: 100vh;
    overflow: hidden;
  }

  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }

  /* --- Toolbar --- */
  .toolbar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 6px 12px;
    background: #1a1a1a;
    border-bottom: 1px solid #2a2a2a;
    flex-shrink: 0;
  }

  .app-title {
    font-weight: 700;
    font-size: 14px;
    color: #a78bfa;
    margin-right: 4px;
  }

  .btn-open {
    padding: 5px 14px;
    background: #5b21b6;
    color: #fff;
    border: none;
    border-radius: 5px;
    cursor: pointer;
    font-size: 12px;
    transition: background 0.15s;
  }
  .btn-open:hover:not(:disabled) { background: #6d28d9; }
  .btn-open:disabled { opacity: 0.5; cursor: default; }

  .filename {
    color: #9ca3af;
    max-width: 320px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .meta {
    color: #6b7280;
    font-size: 11px;
  }

  /* --- Preview --- */
  .preview-area {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
    background: #111;
    position: relative;
  }

  .drop-zone {
    border: 2px dashed #2d2d2d;
  }

  #video-canvas {
    max-width: 100%;
    max-height: 100%;
    background: #000;
  }

  .drop-hint {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 16px;
    color: #4b5563;
    text-align: center;
    line-height: 1.8;
  }

  .error-banner {
    position: absolute;
    bottom: 8px;
    left: 50%;
    transform: translateX(-50%);
    background: #7f1d1d;
    color: #fca5a5;
    padding: 6px 16px;
    border-radius: 6px;
    font-size: 12px;
    max-width: 80%;
    text-align: center;
  }

  /* --- Timeline --- */
  .timeline {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 14px;
    background: #161616;
    border-top: 1px solid #2a2a2a;
    flex-shrink: 0;
  }

  .timecode {
    font-family: "Courier New", monospace;
    font-size: 12px;
    color: #a3a3a3;
    width: 80px;
    flex-shrink: 0;
  }

  .timecode-placeholder {
    font-family: "Courier New", monospace;
    font-size: 12px;
    color: #3a3a3a;
    width: 80px;
    flex-shrink: 0;
  }

  .seekbar {
    flex: 1;
    -webkit-appearance: none;
    appearance: none;
    height: 4px;
    border-radius: 2px;
    background: #374151;
    outline: none;
    cursor: pointer;
  }

  .seekbar::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: #7c3aed;
    cursor: pointer;
    border: 2px solid #0d0d0d;
  }

  .seekbar--disabled {
    opacity: 0.3;
    cursor: default;
    pointer-events: none;
  }
</style>

