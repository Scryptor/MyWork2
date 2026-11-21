<script lang="ts">
	import { untrack } from 'svelte';
	import type { OrderComment } from '$api/OrderComment';
	import { errorMessage } from '#lib/api/client.js';
	import { orderComments } from '#lib/api/orders.js';
	import { createPagedList, type Paged, type PagedList } from '#lib/app/paged-list.svelte.js';
	import { formatFull } from '#lib/format/date.js';
	import { ru } from '#lib/i18n/ru.js';
	import {
		commentDraft,
		editComment,
		latestCommentId,
		sendComment
	} from './comment-drafts.svelte.js';
	import Button from '#lib/ui/Button.svelte';
	import Section from '#lib/ui/Section.svelte';
	import Textarea from '#lib/ui/Textarea.svelte';

	let { orderId, canPost }: { orderId: number; canPost: boolean } = $props();
	const t = ru.orderComments;
	const previewSize = 3;
	const feed: PagedList<OrderComment, Paged> = createPagedList(async (query, signal) => {
		const shown = query.offset ?? 0;
		const cursor = shown ? feed.rows.at(-1)?.id : undefined;
		// Одна дополнительная строка показывает, есть ли продолжение именно за этим курсором.
		const page = await orderComments(orderId, previewSize + 1, cursor, signal);
		return { items: page.items.slice(0, previewSize), total: shown + page.items.length };
	});
	const draft = $derived(commentDraft(orderId));
	const sending = $derived(!!draft?.pending);
	$effect(() => {
		latestCommentId(orderId);
		untrack(() => feed.load({}));
	});
</script>

<Section title={t.title} region>
	{#if canPost}
		<Textarea
			label={t.draft}
			bind:value={() => draft?.body ?? '', (v) => editComment(orderId, v)}
			disabled={sending}
		/>
		{#if draft?.error}<p class="error" role="alert">{draft.error}</p>{/if}
		<div>
			<Button
				size="sm"
				disabled={!draft?.body.trim() || sending}
				busy={sending}
				onclick={() => canPost && sendComment(orderId)}>{t.add}</Button
			>
		</div>
	{/if}
	{#if feed.rows.length}
		<ol>
			{#each feed.rows as note (note.id)}
				<li>
					<div class="meta">
						<strong>{note.author_name}</strong>
						<time datetime={note.created_at}>{formatFull(note.created_at)}</time>
					</div>
					<p class="body">{note.body}</p>
				</li>
			{/each}
		</ol>
	{:else if !feed.loading && !feed.error}
		<p class="meta">{t.empty}</p>
	{/if}
	{#if feed.error}
		<p class="error" role="alert">{errorMessage(feed.error)}</p>
	{/if}
	{#if feed.error || feed.rows.length < feed.total}
		<div>
			<Button
				size="sm"
				disabled={feed.loading}
				onclick={() => (feed.rows.length ? feed.loadMore() : feed.load({}))}
				>{feed.error ? ru.common.retry : t.older}</Button
			>
		</div>
	{/if}
</Section>

<style>
	ol {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		gap: var(--space-3);
	}
	li {
		overflow-wrap: anywhere;
	}
	p {
		margin: 0;
	}
	.meta {
		display: flex;
		align-items: baseline;
		flex-wrap: wrap;
		gap: var(--space-2);
		color: var(--text-3);
		font-size: var(--text-sm);
	}
	.meta strong {
		color: var(--text-2);
	}
	.body {
		white-space: pre-wrap;
	}
	.error {
		color: var(--danger-text);
	}
</style>
