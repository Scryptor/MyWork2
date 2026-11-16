<!-- A plain list for phones that asks for more when its last item comes into view. -->
<script lang="ts" generics="T">
	import type { Snippet } from 'svelte';
	import { ru } from '#lib/i18n/ru.js';
	import EmptyState from './EmptyState.svelte';
	import Skeleton from './Skeleton.svelte';
	import type { RowId } from './table/types.js';

	let {
		items,
		key,
		item,
		onendreached,
		loading = false,
		empty
	}: {
		items: T[];
		key: (t: T) => RowId;
		item: Snippet<[T]>;
		onendreached?: () => void;
		loading?: boolean;
		empty?: Snippet;
	} = $props();

	let list = $state<HTMLElement | null>(null);

	// A new observer for every new last item and after loading: its first report tells whether the end
	// is already in view (a short list asks again until it fills the screen).
	$effect(() => {
		const last = items.length > 0 ? list?.lastElementChild : null;
		if (!last || loading || !onendreached || typeof IntersectionObserver === 'undefined') return;
		const ask = onendreached;
		const observer = new IntersectionObserver(
			(entries) => {
				if (entries.some((e) => e.isIntersecting)) {
					observer.disconnect();
					ask();
				}
			},
			{ rootMargin: '0px 0px 200px 0px' }
		);
		observer.observe(last);
		return () => observer.disconnect();
	});
</script>

<div class="list" role="list" bind:this={list} aria-busy={loading ? 'true' : undefined}>
	{#each items as t (key(t))}
		<div class="item" role="listitem">{@render item(t)}</div>
	{/each}
</div>
{#if loading}
	<div class="loading" class:more={items.length > 0} aria-hidden="true">
		{#each [0, 1, 2] as i (i)}
			<Skeleton height="4.5rem" radius="md" />
		{/each}
	</div>
{:else if items.length === 0}
	{#if empty}{@render empty()}{:else}<EmptyState title={ru.table.empty} />{/if}
{/if}

<style>
	.list,
	.loading {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}
	.loading.more {
		margin-top: var(--space-2);
	}
</style>
