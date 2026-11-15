<script lang="ts">
	import { formatRelative, type LocalDateTime } from '#lib/format/date.js';

	let {
		items,
		now,
		emptyText = ''
	}: {
		items: { id: string | number; at: string; who?: string; text: string }[];
		now: LocalDateTime;
		emptyText?: string;
	} = $props();
</script>

{#if items.length === 0}
	<p class="empty">{emptyText}</p>
{:else}
	<ol class="timeline">
		{#each items as item (item.id)}
			<li>
				<span class="dot" aria-hidden="true"></span>
				<div class="meta num">
					<time datetime={item.at}>{formatRelative(item.at, now)}</time>
					{#if item.who}<span>{item.who}</span>{/if}
				</div>
				<div>{item.text}</div>
			</li>
		{/each}
	</ol>
{/if}

<style>
	.timeline {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		border-inline-start: 1px solid var(--border);
		margin-inline-start: var(--space-1);
	}
	li {
		position: relative;
		padding-inline-start: var(--space-4);
		font-size: var(--text-md);
	}
	.dot {
		position: absolute;
		inset-inline-start: -0.1875rem;
		top: 0.4rem;
		width: 0.375rem;
		height: 0.375rem;
		border-radius: var(--radius-full);
		background: var(--border-strong);
	}
	.meta {
		display: flex;
		gap: var(--space-2);
		font-size: var(--text-xs);
		color: var(--text-3);
	}
	.empty {
		color: var(--text-3);
		margin: 0;
	}
</style>
