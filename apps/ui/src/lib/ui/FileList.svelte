<!-- Files of an order: name, size, date, a «Файл не найден» mark; download and delete. -->
<script lang="ts">
	import Download from '@lucide/svelte/icons/download';
	import File from '@lucide/svelte/icons/file';
	import Trash2 from '@lucide/svelte/icons/trash-2';
	import type { Attachment } from '$api/Attachment';
	import { formatBytes } from '#lib/format/bytes.js';
	import { formatDate } from '#lib/format/date.js';
	import { ru } from '#lib/i18n/ru.js';
	import IconButton from './IconButton.svelte';

	let {
		files,
		ondownload,
		ondelete,
		readonly = false
	}: {
		files: Attachment[];
		ondownload?: (file: Attachment) => void;
		ondelete?: (file: Attachment) => void;
		readonly?: boolean;
	} = $props();
</script>

<ul class="files" aria-label={ru.media.files}>
	{#each files as file (file.id)}
		<li class="file">
			<File size={18} aria-hidden="true" />
			<div class="info">
				<span class="name">{file.name}</span>
				<span class="meta">
					<span class="num">{formatBytes(file.size)}</span>
					<span class="num">{formatDate(file.created_at)}</span>
					{#if file.missing}<span class="missing">{ru.media.missing}</span>{/if}
				</span>
			</div>
			{#if ondownload && !file.missing}
				<IconButton
					label="{ru.media.download}: {file.name}"
					icon={Download}
					onclick={() => ondownload(file)}
				/>
			{/if}
			{#if ondelete && !readonly}
				<IconButton
					label="{ru.media.deleteFile}: {file.name}"
					icon={Trash2}
					onclick={() => ondelete(file)}
				/>
			{/if}
		</li>
	{/each}
</ul>

<style>
	.files {
		display: flex;
		flex-direction: column;
		margin: 0;
		padding: 0;
		list-style: none;
	}
	.file {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		padding: var(--space-2) 0;
		border-bottom: 1px solid var(--border);
		color: var(--text-2);
	}
	.info {
		flex: 1 1 auto;
		min-width: 0;
		display: flex;
		flex-direction: column;
	}
	.name {
		color: var(--text);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.meta {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-3);
		color: var(--text-3);
		font-size: var(--text-sm);
	}
	.missing {
		color: var(--danger-text);
	}
</style>
