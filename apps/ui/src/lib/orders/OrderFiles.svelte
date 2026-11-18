<!-- The files of an order: the list with download and delete, «Добавить файл» takes anything. -->
<script lang="ts">
	import Plus from '@lucide/svelte/icons/plus';
	import { downloadAttachment } from '#lib/api/attachments.js';
	import { errorMessage } from '#lib/api/client.js';
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';
	import FileList from '#lib/ui/FileList.svelte';
	import { toast } from '#lib/ui/toast/toast.svelte.js';
	import type { OrderMedia } from './photos.svelte.js';

	let { media, readonly = false }: { media: OrderMedia; readonly?: boolean } = $props();

	let input = $state<HTMLInputElement | null>(null);
	const waiting = $derived(media.pending.filter((p) => !p.photo));
</script>

{#if media.files.length === 0 && waiting.length === 0}
	<p class="muted">{ru.media.noFiles}</p>
{:else}
	{#each waiting as p (p.id)}
		<p class="muted">{p.name}: {ru.media.uploading}</p>
	{/each}
	<FileList
		files={media.files}
		{readonly}
		ondownload={(f) =>
			void downloadAttachment(f).catch((e) =>
				toast.show({ message: errorMessage(e), tone: 'danger' })
			)}
		ondelete={(f) => void media.remove(f.id)}
	/>
{/if}
{#if !readonly}
	<div>
		<Button size="sm" icon={Plus} onclick={() => input?.click()}>{ru.media.addFile}</Button>
	</div>
	<input
		bind:this={input}
		type="file"
		hidden
		multiple
		onchange={(e) => {
			void media.add(Array.from(e.currentTarget.files ?? []));
			e.currentTarget.value = '';
		}}
	/>
{/if}

<style>
	.muted {
		margin: 0;
		color: var(--text-3);
		font-size: var(--text-sm);
	}
</style>
