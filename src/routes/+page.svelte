<script>
    import { goto } from "$app/navigation";
    import { onMount } from "svelte";
    import { openTimelineWindow } from "../lib/window";

    onMount(() => {
        goto("/preview");
        openTimelineWindow();
    });
</script>
