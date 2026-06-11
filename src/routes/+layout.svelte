<script lang="ts">
    import "../app.css";
    let { children } = $props();
</script>

<div class="box-border m-0 p-0 w-full h-full font-kaisei">
    {@render children()}
</div>

<style lang="postcss">
</style>
