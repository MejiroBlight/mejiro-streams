<script lang="ts">
    import { getCurrentWindow } from "@tauri-apps/api/window";
    import { onDestroy, onMount } from "svelte";
    import type { VideoInfo } from "../bindings";
    import { commands } from "../bindings";
    import { open } from "@tauri-apps/plugin-dialog";
    import { Button } from "bits-ui";
    import PreviewMenubar from "./preview/PreviewMenubar.svelte";
    import { subWindows } from "../store";

    let videoInfo = $state<VideoInfo | null>(null);
    let currentMs = $state(0);
    let isLoading = $state(false);
    let errorMsg = $state("");
    let filename = $state("");
    let canvas = $state<HTMLCanvasElement>(null!);
    let ctx: CanvasRenderingContext2D;
    let frameFetchStartTime: number = 0;

    onMount(() => {
        commands
            .startFrameServer()
            .then(() => {
                console.log("フレームサーバーが起動しました");
            })
            .catch((e) => {
                console.error("フレームサーバーの起動に失敗:", e);
            });
    });

    function msToTimecode(ms: number): string {
        const h = Math.floor(ms / 3_600_000);
        const m = Math.floor((ms % 3_600_000) / 60_000);
        const s = Math.floor((ms % 60_000) / 1_000);
        const f = Math.floor((ms % 1_000) / (1000 / 30)); // approx 30fps frame number
        return `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}.${String(f).padStart(2, "0")}`;
    }

    async function openFile() {
        isLoading = true;
        try {
            const selected = await open({
                multiple: false,
                filters: [
                    {
                        name: "Video Files",
                        extensions: ["mp4", "mkv", "avi", "mov", "webm"],
                    },
                    { name: "All Files", extensions: ["*"] },
                ],
            });
            if (typeof selected === "string") {
                await loadVideo(selected);
            }
        } catch (e) {
            // User probably cancelled the dialog, so we can ignore errors
        } finally {
            isLoading = false;
        }
    }

    async function loadVideo(path: string) {
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

            console.log(
                "データの受信に成功しました バイト数:",
                binaryData.length,
            );
            console.log("先頭のデータ（フレーム番号）:", binaryData[0]);

            const frame = new VideoFrame(new Uint8Array(arrayBuffer), {
                format: "NV12",
                codedWidth: canvas.width, // 256の倍数でパディングされた幅
                codedHeight: canvas!.height,
                timestamp: performance.now() * 1000,
                colorSpace: {
                    matrix: "bt709",
                    primaries: "bt709",
                    transfer: "bt709",
                    fullRange: false,
                },
            });

            if (!ctx) {
                ctx = canvas.getContext("2d")!;
            }

            ctx.drawImage(frame, 0, 0, canvas!.width, canvas!.height);
            console.log(
                `フレームの描画にかかった時間: ${performance.now() - frameFetchStartTime} ms`,
            );
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
</script>

<div class="flex flex-col bg-neutral-700 text-neutral-200 h-dvh">
    <PreviewMenubar onOpenFile={openFile} />
    <main
        ondragover={(e) => e.preventDefault()}
        class="grow flex items-center justify-center relative overflow-hidden"
    >
        <canvas bind:this={canvas} class="w-full h-full object-contain bg-black"
        ></canvas>
    </main>
    <footer
        class="flex items-center gap-4 px-4 py-2 bg-neutral-900 border-t-2 border-secondary-500 shrink-0"
    >
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
            <span class="timecode">--:--:--.--</span>
            <div class="seekbar" id="disabled"></div>
            <span class="timecode">--:--:--.--</span>
        {/if}
    </footer>
</div>

<style lang="postcss">
    @reference "../../app.css";

    .timecode {
        @apply font-mono text-sm text-neutral-400;
    }

    .seekbar {
        @apply flex-1 appearance-none h-1 rounded bg-neutral-600 cursor-pointer;
        &::-webkit-slider-thumb {
            @apply appearance-none w-4 h-4 rounded-full bg-primary-500 cursor-pointer border-2 border-neutral-900;
        }
        &#disabled {
            @apply opacity-50 cursor-not-allowed;
        }
    }
</style>
