<script lang="ts">
    import { Menubar, Portal } from "bits-ui";
    import { BirdIcon } from "phosphor-svelte";
    import { openTimelineWindow } from "../../window";

    interface Props {
        onOpenFile: () => void;
    }

    let { onOpenFile }: Props = $props();
</script>

<Menubar.Root
    class="bg-neutral-900 text-white flex items-center h-8 px-0 border-b-2 border-secondary-500"
>
    <div class="text-primary-200 px-2 text-2xl">
        <BirdIcon weight="regular" />
    </div>
    <Menubar.Menu>
        <Menubar.Trigger class="menubar-trigger border-l">File</Menubar.Trigger>
        <Menubar.Portal>
            <Menubar.Content class="menubar-content">
                <Menubar.Item onclick={onOpenFile} class="menubar-item">
                    Open Video
                </Menubar.Item>
                <Menubar.Item disabled class="menubar-item">
                    Settings
                </Menubar.Item>
            </Menubar.Content>
        </Menubar.Portal>
    </Menubar.Menu>
    <Menubar.Menu>
        <Menubar.Trigger class="menubar-trigger">Edit</Menubar.Trigger>
    </Menubar.Menu>
    <Menubar.Menu>
        <Menubar.Trigger class="menubar-trigger">View</Menubar.Trigger>
        <Menubar.Portal>
            <Menubar.Content class="menubar-content">
                <Menubar.Item class="menubar-item" onclick={openTimelineWindow}>
                    Open Timeline Window
                </Menubar.Item>
            </Menubar.Content>
        </Menubar.Portal>
    </Menubar.Menu>
</Menubar.Root>

<style lang="postcss">
    @reference "../../../app.css";

    :global {
        .menubar-trigger {
            @apply px-3 py-1 hover:bg-primary-700 cursor-pointer border-r;
        }

        .menubar-content {
            @apply bg-neutral-800 font-kaisei text-white shadow-lg rounded mt-1 border p-1;
        }

        .menubar-item {
            @apply px-3 py-1 hover:bg-primary-700 cursor-pointer rounded;
        }
    }
</style>
