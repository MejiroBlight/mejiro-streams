import type { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import type { Hash } from "phosphor-svelte";
import { readable, writable } from "svelte/store";

export const videoPath = writable<string | null>(null);

export const subWindows = writable<WebviewWindow[]>([]);