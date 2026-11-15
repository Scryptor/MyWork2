<script lang="ts">
	import { initials } from '#lib/format/name.js';

	let { name, size = 'md' }: { name: string; size?: 'sm' | 'md' } = $props();
</script>

<span class="avatar {size}">{initials(name)}</span>

<style>
	.avatar {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		border-radius: var(--radius-full);
		background: var(--accent-soft);
		color: var(--accent-text);
		font-weight: 600;
		flex: none;
	}
	.md {
		width: 2rem;
		height: 2rem;
		font-size: var(--text-sm);
	}
	.sm {
		width: 1.5rem;
		height: 1.5rem;
		font-size: var(--text-xs);
	}
</style>
