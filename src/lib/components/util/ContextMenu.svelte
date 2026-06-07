<script lang="ts">
    import { tick } from "svelte";

    export type ContextMenuItem = {
        label?: string;
        disabled?: boolean;
        separator?: boolean;
        action?: () => void;
        children?: ContextMenuItem[];
    };

    interface Props {
        open: boolean;
        x: number;
        y: number;
        items: ContextMenuItem[];
        onclose?: () => void;
    }

    let { open, x, y, items, onclose }: Props = $props();

    let menuX = $state(0);
    let menuY = $state(0);
    let rootMenuEl = $state<HTMLUListElement | null>(null);

    $effect(() => {
        if (!open) return;

        menuX = x;
        menuY = y;

        const adjust = async () => {
            await tick();
            if (!rootMenuEl) return;
            const rect = rootMenuEl.getBoundingClientRect();
            if (rect.bottom > window.innerHeight - 8) {
                menuY = Math.max(8, y - rect.height);
            }
            if (rect.right > window.innerWidth - 8) {
                menuX = Math.max(8, window.innerWidth - rect.width - 8);
            }
        };

        adjust();
    });

    function closeMenu() {
        onclose?.();
    }

    function handleItemClick(item: ContextMenuItem) {
        if (item.disabled) return;
        item.action?.();
        closeMenu();
    }
</script>

{#if open}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="ctx-overlay" onclick={closeMenu}>
        <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
        <ul
            class="ctx-menu"
            style="left:{menuX}px; top:{menuY}px;"
            bind:this={rootMenuEl}
            onclick={(e) => e.stopPropagation()}
        >
            {#each items as item}
                {#if item.separator}
                    <li class="ctx-separator"></li>
                {:else if item.children && item.children.length > 0}
                    <li class="ctx-item has-submenu">
                        <button
                            type="button"
                            class="submenu-trigger"
                            disabled={item.disabled}>{item.label}</button
                        >
                        <ul class="ctx-submenu">
                            {#each item.children as child}
                                {#if child.separator}
                                    <li class="ctx-separator"></li>
                                {:else}
                                    <li>
                                        <button
                                            type="button"
                                            onclick={() =>
                                                handleItemClick(child)}
                                            disabled={child.disabled}
                                        >
                                            {child.label}
                                        </button>
                                    </li>
                                {/if}
                            {/each}
                        </ul>
                    </li>
                {:else}
                    <li>
                        <button
                            type="button"
                            onclick={() => handleItemClick(item)}
                            disabled={item.disabled}>{item.label}</button
                        >
                    </li>
                {/if}
            {/each}
        </ul>
    </div>
{/if}

<style>
    .ctx-overlay {
        position: fixed;
        inset: 0;
        z-index: 500;
    }

    .ctx-menu {
        position: fixed;
        min-width: 180px;
        list-style: none;
        margin: 0;
        padding: 4px 0;
        border-radius: 6px;
        border: 1px solid #2a2a50;
        background: #12122a;
        box-shadow: 0 8px 18px rgba(0, 0, 0, 0.45);
        z-index: 501;
    }

    .ctx-item {
        position: relative;
    }

    .ctx-menu li button {
        width: 100%;
        border: none;
        background: transparent;
        color: var(--main-text-color);
        text-align: left;
        font-size: 12px;
        padding: 7px 12px;
        cursor: pointer;
    }

    .ctx-menu li button:hover:not(:disabled) {
        background: #2a2a50;
    }

    .ctx-menu li button:disabled {
        color: #5a5a78;
        cursor: not-allowed;
    }

    .submenu-trigger::after {
        content: "▶";
        float: right;
        color: #8d92b9;
        font-size: 10px;
        margin-top: 1px;
    }

    .ctx-submenu {
        display: none;
        position: absolute;
        top: -4px;
        left: calc(100% - 2px);
        min-width: 120px;
        list-style: none;
        margin: 0;
        padding: 4px 0;
        border-radius: 6px;
        border: 1px solid #2a2a50;
        background: #12122a;
        box-shadow: 0 8px 18px rgba(0, 0, 0, 0.45);
    }

    .has-submenu:hover > .ctx-submenu {
        display: block;
    }

    .ctx-separator {
        height: 1px;
        background: #2a2a50;
        margin: 4px 0;
    }
</style>
