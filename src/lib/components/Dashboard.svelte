<script>
    import { onMount } from "svelte";
    import { createMenubar } from "./menu/Menubar";
    import { getCurrentWindow } from "@tauri-apps/api/window";
    import { Pane, Splitpanes } from "svelte-splitpanes";
    import Preview from "./preview/Preview.svelte";

    onMount(() => {
        const currentWindow = getCurrentWindow();
        createMenubar(currentWindow);
    });
</script>

<div class="h-dvh w-full">
    <Splitpanes horizontal={true} id="main-splitpanes">
        <Pane minSize={15}>
            <Splitpanes horizontal={false}>
                <Pane minSize={15}>
                    <Preview />
                </Pane>
                <Pane minSize={15}>
                    <div class="p-4">
                        <h2 class="text-lg font-bold">Assets</h2>
                        <!-- Assets content goes here -->
                    </div>
                </Pane>
            </Splitpanes>
        </Pane>
        <Pane minSize={15}>
            <div class="p-4">
                <h2 class="text-lg font-bold">Timeline</h2>
                <!-- Timeline content goes here -->
            </div>
        </Pane>
    </Splitpanes>
</div>

<style lang="postcss">
    @reference "../../app.css";

    :global {
        #main-splitpanes {
            @apply bg-neutral-900 text-neutral-200;
        }

        .splitpanes__splitter {
            @apply bg-primary-500;
        }
    }
</style>
