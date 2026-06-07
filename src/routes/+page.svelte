<script>
    import Preview from "$lib/components/Preview.svelte";
    import { WebviewWindow } from "@tauri-apps/api/webviewWindow";

    async function openNewWindow() {
        // 一意のラベル（ID）を指定して新しいウィンドウを作成
        const newWindow = new WebviewWindow("sub-window-label", {
            url: "/timeline", // SvelteKitなどのルーティング先のパス
            title: "サブウィンドウ",
            width: 600,
            height: 400,
            resizable: true,
            maximizable: false,
            // 必要に応じてその他のオプションを指定
        });

        // ウィンドウが正常に作成されたか確認（任意）
        newWindow.once("tauri://created", () => {
            console.log("新しいウィンドウが正常に作成されました");
        });

        newWindow.once("tauri://error", (e) => {
            console.error("ウィンドウ作成エラー:", e);
        });
    }
</script>

<Preview />
