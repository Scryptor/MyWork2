<script lang="ts">
	let { vertical = false }: { vertical?: boolean } = $props();
</script>

<div
	class="divider"
	class:vertical
	role="separator"
	aria-orientation={vertical ? 'vertical' : 'horizontal'}
></div>

<style>
	.divider {
		background: var(--border);
		width: 100%;
		height: 1px;
		flex: none;
	}
	.vertical {
		width: 1px;
		height: auto;
		align-self: stretch;
	}
</style>
