import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { subWindows } from "./store";
import type { WindowOptions } from "@tauri-apps/api/window";
import type { WebviewOptions } from "@tauri-apps/api/webview";

export function openTimelineWindow() {
    createWindow("timeline-window", {
        url: "/timeline",
        title: "mejiro-streams - Timeline",
        width: 600,
        height: 400,
        resizable: true,
        maximizable: false,
        skipTaskbar: true,
    });
}

function createWindow(label: string, options: (Omit<WebviewOptions, "width" | "height" | "x" | "y"> & WindowOptions) | undefined) {
    const newWindow = new WebviewWindow(label, options);
    newWindow.onCloseRequested(() => {
        subWindows.update((windows) =>
            windows.filter((window) => window.label !== label),
        );
    });
    subWindows.update((windows) => [...windows, newWindow]);
}