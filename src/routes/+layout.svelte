<script lang="ts">
    import "../app.css";
    import "tailwindcss";
    let { children } = $props();
</script>

<div class="box-border m-0 p-0 w-full h-full font-kaisei">
    {@render children()}
</div>

