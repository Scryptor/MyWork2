<!-- A picture of an attachment: loads its blob URL, shows a spinner meanwhile and a note on failure. -->
<script lang="ts">
	import { attachmentUrl, type AttachmentVariant } from '#lib/api/attachments.js';
	import { ru } from '#lib/i18n/ru.js';
	import Spinner from './Spinner.svelte';

	let {
		id,
		variant,
		alt,
		missing = false,
		resolveUrl = attachmentUrl,
		class: className = ''
	}: {
		id: number;
		variant: AttachmentVariant;
		alt: string;
		missing?: boolean;
		resolveUrl?: (id: number, variant: AttachmentVariant) => Promise<string>;
		class?: string;
	} = $props();

	let src = $state('');
	let failed = $state(false);

	$effect(() => {
		const wanted = [id, variant, missing, resolveUrl] as const;
		src = '';
		failed = false;
		if (wanted[2]) return;
		let cancelled = false;
		wanted[3](wanted[0], wanted[1]).then(
			(url) => {
				if (!cancelled) src = url;
			},
			() => {
				if (!cancelled) failed = true;
			}
		);
		return () => {
			cancelled = true;
		};
	});
</script>

{#if src}
	<img class={className} {src} {alt} />
{:else if missing || failed}
	<span class="note {className}">{ru.media.missing}</span>
{:else}
	<span class="note {className}"><Spinner size="sm" /></span>
{/if}

<style>
	.note {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		color: var(--text-3);
		font-size: var(--text-sm);
		text-align: center;
	}
</style>
