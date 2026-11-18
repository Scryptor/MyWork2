<!-- Full-screen photo viewer: ← → page through, Esc closes, the caption is edited in place. -->
<script lang="ts">
	import { Dialog } from 'bits-ui';
	import ChevronLeft from '@lucide/svelte/icons/chevron-left';
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import Trash2 from '@lucide/svelte/icons/trash-2';
	import X from '@lucide/svelte/icons/x';
	import { untrack } from 'svelte';
	import type { Attachment } from '$api/Attachment';
	import { attachmentUrl, type AttachmentVariant } from '#lib/api/attachments.js';
	import { layers } from '#lib/design/layers.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import AttachmentImage from './AttachmentImage.svelte';
	import Button from './Button.svelte';
	import { dialogFocus, keepOpenOnToasts } from './dialog.js';
	import IconButton from './IconButton.svelte';
	import TextInput from './TextInput.svelte';
	import VisuallyHidden from './VisuallyHidden.svelte';
	import { toast } from './toast/toast.svelte.js';
	import './dialog.css';

	let {
		photos,
		index = $bindable(0),
		open = $bindable(false),
		oncaption,
		ondelete,
		readonly = false,
		resolveUrl = attachmentUrl
	}: {
		photos: Attachment[];
		index: number;
		open: boolean;
		oncaption?: (id: number, text: string) => void;
		ondelete?: (id: number) => void;
		readonly?: boolean;
		resolveUrl?: (id: number, variant: AttachmentVariant) => Promise<string>;
	} = $props();

	const current = $derived(photos[index]);
	let caption = $derived(photos[index]?.caption ?? '');
	let content = $state<HTMLElement | null>(null);

	// A photo deleted from under the viewer: step back, or close when none are left.
	$effect(() => {
		if (!open) return;
		if (photos.length === 0) open = false;
		else if (index >= photos.length) index = photos.length - 1;
	});

	const focusReturn = dialogFocus();
	$effect(() => {
		if (!open) return;
		return untrack(() => {
			focusReturn.opened();
			return layers.open('modal', () => (open = false));
		});
	});

	// What was already reported, so blur + Esc (or a slow parent) report a caption once.
	let sent: { id: number; text: string } | null = null;
	function save() {
		if (readonly || !current || caption === current.caption) return;
		if (sent?.id === current.id && sent.text === caption) return;
		sent = { id: current.id, text: caption };
		oncaption?.(current.id, caption);
	}
	// Closing by Esc, the close button or the overlay may not blur the field: save on every close path.
	let wasOpen = false;
	$effect(() => {
		if (open) wasOpen = true;
		else if (wasOpen) {
			wasOpen = false;
			untrack(save);
		}
	});
	function remove() {
		if (!current) return;
		const id = current.id;
		ondelete?.(id);
		toast.show({ message: ru.media.photoDeleted });
	}
	function go(step: number) {
		const next = index + step;
		if (next < 0 || next >= photos.length) return;
		save();
		index = next;
	}
	function onkeydown(e: KeyboardEvent) {
		if (!open || e.defaultPrevented) return;
		if (e.key === 'Enter' && !e.isComposing && e.target instanceof Element) {
			if (e.target.closest('.caption')) save();
			return;
		}
		if (e.key !== 'ArrowLeft' && e.key !== 'ArrowRight') return;
		// In the caption the arrows move the caret.
		if (e.target instanceof Element && e.target.closest('input, textarea')) return;
		e.preventDefault();
		go(e.key === 'ArrowLeft' ? -1 : 1);
	}
</script>

<svelte:window {onkeydown} />

<Dialog.Root bind:open>
	<Dialog.Portal>
		<Dialog.Overlay class="mw-scrim" />
		<Dialog.Content
			bind:ref={content}
			onInteractOutside={keepOpenOnToasts}
			onCloseAutoFocus={focusReturn.closed}
			onOpenAutoFocus={(e) => {
				e.preventDefault();
				content?.focus({ preventScroll: true });
			}}
			class="mw-dialog viewer"
			aria-label={ru.media.viewer}
		>
			<Dialog.Title class="mw-dialog-title" level={2}>
				<VisuallyHidden>{ru.media.viewer}</VisuallyHidden>
			</Dialog.Title>
			<div class="stage">
				<div class="nav prev">
					<IconButton
						label={ru.media.previous}
						icon={ChevronLeft}
						disabled={index <= 0}
						onclick={() => go(-1)}
					/>
				</div>
				<div class="picture">
					{#if current}
						<AttachmentImage
							id={current.id}
							variant="file"
							alt={current.caption || current.name}
							missing={current.missing}
							{resolveUrl}
							class="big"
						/>
					{/if}
				</div>
				<div class="nav next">
					<IconButton
						label={ru.media.next}
						icon={ChevronRight}
						disabled={index >= photos.length - 1}
						onclick={() => go(1)}
					/>
				</div>
			</div>
			<div class="bar">
				<span class="count num">{index + 1} {ru.media.of} {photos.length}</span>
				<div class="caption">
					<TextInput
						bind:value={caption}
						ariaLabel={ru.media.caption}
						placeholder={ru.media.captionPlaceholder}
						{readonly}
						onblur={save}
					/>
				</div>
				{#if !readonly}
					<Button variant="danger" icon={Trash2} onclick={remove}>{ru.media.deletePhoto}</Button>
				{/if}
			</div>
			<ul class="strip" aria-label={ru.media.photos}>
				{#each photos as photo, i (photo.id)}
					<li>
						<button
							type="button"
							class="mini"
							aria-current={i === index}
							aria-label="{ru.media.openPhoto}: {photo.caption || photo.name}"
							onclick={() => {
								save();
								index = i;
							}}
						>
							<AttachmentImage
								id={photo.id}
								variant="thumb"
								alt=""
								missing={photo.missing}
								{resolveUrl}
								class="mini-img"
							/>
						</button>
					</li>
				{/each}
			</ul>
			<!-- Last in the DOM, like in Dialog. -->
			<button
				type="button"
				class="mw-dialog-close"
				aria-label={ru.common.close}
				onclick={() => (open = false)}
			>
				<X size={16} aria-hidden="true" />
			</button>
		</Dialog.Content>
	</Dialog.Portal>
</Dialog.Root>

<style>
	:global(.mw-dialog.viewer) {
		inset: 0;
		translate: none;
		width: auto;
		max-height: none;
		border: 0;
		border-radius: 0;
		padding: var(--space-3);
		gap: var(--space-3);
	}
	.stage {
		flex: 1 1 auto;
		min-height: 0;
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}
	.picture {
		flex: 1 1 auto;
		min-width: 0;
		height: 100%;
		display: flex;
		align-items: center;
		justify-content: center;
	}
	.picture :global(.big) {
		max-width: 100%;
		max-height: 100%;
		object-fit: contain;
	}
	.bar {
		flex: none;
		display: flex;
		align-items: center;
		gap: var(--space-3);
	}
	.count {
		color: var(--text-3);
		white-space: nowrap;
	}
	.caption {
		flex: 1 1 auto;
		min-width: 0;
	}
	.strip {
		flex: none;
		display: flex;
		gap: var(--space-2);
		margin: 0;
		padding: 0 0 var(--space-1);
		list-style: none;
		overflow-x: auto;
	}
	.mini {
		display: block;
		box-sizing: border-box;
		width: 3.5rem;
		height: 3.5rem;
		padding: 0;
		border: 2px solid transparent;
		border-radius: var(--radius-md);
		background: var(--surface-2);
		overflow: hidden;
		cursor: pointer;
	}
	.mini[aria-current='true'] {
		border-color: var(--accent);
	}
	.mini :global(.mini-img) {
		width: 100%;
		height: 100%;
		object-fit: cover;
		font-size: var(--text-xs);
	}
	@media (pointer: coarse) {
		.mini {
			width: 3rem;
			height: 3rem;
		}
	}
</style>
