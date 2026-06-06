<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onDestroy, onMount } from "svelte";
  import type { VideoInfo} from "../bindings";
  import {commands} from "../bindings";
  import {open} from "@tauri-apps/plugin-dialog";
  import { Channel } from "@tauri-apps/api/core";

  // ---------------------------------------------------------------------------
  // State
  // ---------------------------------------------------------------------------

  let videoInfo = $state<VideoInfo | null>(null);
  let currentMs = $state(0);
  let isLoading = $state(false);
  let errorMsg = $state("");
  let filename = $state("");
  let canvas = $state<HTMLCanvasElement>(null!);
  let ctx: CanvasRenderingContext2D;
  let frameFetchStartTime: number = 0;
  

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
      canvas.width = videoInfo.width;
      canvas.height = videoInfo.height;
      await requestCustomData(0);
    } catch (e) {
      errorMsg = String(e);
    } finally {
      isLoading = false;
    }
  }

  async function requestCustomData(ms: number) {
    frameFetchStartTime = performance.now();

    const url = `tauri://localhost/frame?num=${ms}`;

    try {
      // 1. 標準の fetch を使ってリクエストを送信
      const response = await fetch(url);
      
      if (!response.ok) {
        throw new Error(`HTTPエラー: ${response.status}`);
      }

      if (response.status !== 200) {
        const buf = await response.arrayBuffer();
        const text = new TextDecoder().decode(buf);
        throw new Error(`サーバーエラー: ${text}`);
      }

      // 2. レスポンスをバイナリ（ArrayBuffer）として受け取る
      const arrayBuffer = await response.arrayBuffer();
      
      // 3. JavaScriptで扱いやすいように Uint8Array に変換
      const binaryData = new Uint8Array(arrayBuffer);

      console.log("データの受信に成功しました バイト数:", binaryData.length);
      console.log("先頭のデータ（フレーム番号）:", binaryData[0]);

      const frame = new VideoFrame(new Uint8Array(arrayBuffer), {
        format: 'NV12',
        codedWidth: canvas.width, // 256の倍数でパディングされた幅
        codedHeight: canvas!.height,
        timestamp: performance.now() * 1000,
        colorSpace: {
          matrix: 'bt709',
          primaries: 'bt709',
          transfer: 'bt709',
          fullRange: false,
        }
      });

      if (!ctx) {
        ctx = canvas.getContext('2d')!;
      }

      ctx.drawImage(frame, 0, 0, canvas!.width, canvas!.height);
      console.log(`フレームの描画にかかった時間: ${performance.now() - frameFetchStartTime} ms`);
      frame.close();
    
    } catch (error) {
      console.error("カスタムプロトコルからのデータ取得に失敗:", error);
    }
  }
  // Debounced seek – avoids flooding the Rust side while dragging
  let seekTimer: ReturnType<typeof setTimeout> | null = null;
  function onSeek(e: Event) {
    const target = e.target as HTMLInputElement;
    const newMs = Number(target.value);
    currentMs = newMs; // UIは即座に更新
    if (seekTimer) {
      clearTimeout(seekTimer);
    }
    seekTimer = setTimeout(() => {
      requestCustomData(newMs);
      seekTimer = null;
    }, 100); // 100msのデバウンス
  }

  // ---------------------------------------------------------------------------
  // Drag-and-drop (Tauri window-level)
  // ---------------------------------------------------------------------------
  onMount(() => {
    const appWindow = getCurrentWindow();
    appWindow.onDragDropEvent(async (event) => {
      if (event.payload.type === "drop") {
        const paths: string[] = event.payload.paths;
        if (paths.length > 0) {
          await loadVideo(paths[0]);
        }
      }
    });

    commands.startFrameServer().then(() => {
      console.log("フレームサーバーが起動しました");
    }).catch((e) => {
      console.error("フレームサーバーの起動に失敗:", e);
    });
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

    <canvas class="preview-canvas" bind:this={canvas}></canvas>

    {#if errorMsg}
      <div class="error-banner">{errorMsg}</div>
    {/if}
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
    display: flex;
  }

  .app {
    display: flex;
    flex-direction: column;
    flex: 1;
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

  .preview-canvas {
    height: 100%;
    width: 100%;
    background: #111;
    object-fit: contain;
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

