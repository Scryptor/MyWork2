<!-- The photos of an order (spec 6.2, 6.3): thumbnails, «＋» with «Снять камерой» / «Выбрать файлы», drop and
     paste on the block, the viewer for captions and deletion. -->
<script lang="ts">
	import Camera from '@lucide/svelte/icons/camera';
	import Upload from '@lucide/svelte/icons/upload';
	import { untrack } from 'svelte';
	import { ru } from '#lib/i18n/ru.js';
	import AttachmentImage from '#lib/ui/AttachmentImage.svelte';
	import Button from '#lib/ui/Button.svelte';
	import CameraDialog from '#lib/ui/CameraDialog.svelte';
	import type { MenuItem } from '#lib/ui/DropdownMenu.svelte';
	import FileDrop from '#lib/ui/FileDrop.svelte';
	import PhotoStrip from '#lib/ui/PhotoStrip.svelte';
	import PhotoViewer from '#lib/ui/PhotoViewer.svelte';
	import { limitPhotos, MAX_PHOTOS_PER_ADD, type OrderMedia } from './photos.svelte.js';

	let { media, readonly = false }: { media: OrderMedia; readonly?: boolean } = $props();

	let drop = $state<{ open: () => void } | null>(null);
	let viewerOpen = $state(false);
	let viewerIndex = $state(0);
	let cameraOpen = $state(false);
	/** Pictures taken since the camera was opened: it stops at the limit of one adding. */
	let shots = $state(0);

	$effect(() => {
		if (cameraOpen) untrack(() => (shots = 0));
	});

	const strip = $derived(
		media.photos.map((a) => ({ id: String(a.id), src: '', alt: a.caption || a.name }))
	);
	const items: MenuItem[] = [
		{ label: ru.media.takeWithCamera, icon: Camera, onselect: () => (cameraOpen = true) },
		{ label: ru.media.chooseFiles, icon: Upload, onselect: () => drop?.open() }
	];

	const addFiles = (files: File[]) => void media.add(limitPhotos(files));
</script>

{#snippet picture(photo: { id: string; alt: string })}
	<AttachmentImage
		id={Number(photo.id)}
		variant="thumb"
		alt={photo.alt}
		missing={media.photos.find((a) => String(a.id) === photo.id)?.missing ?? false}
	/>
{/snippet}

{#snippet strips()}
	<PhotoStrip
		photos={strip}
		max={null}
		label={ru.media.photoSection}
		pending={media.pending.filter((p) => p.photo)}
		image={picture}
		addItems={readonly ? undefined : items}
		onopen={(id) => {
			viewerIndex = Math.max(
				0,
				strip.findIndex((p) => p.id === id)
			);
			viewerOpen = true;
		}}
		onremove={readonly ? undefined : (id) => void media.remove(Number(id))}
	/>
{/snippet}

{#if readonly}
	{@render strips()}
{:else}
	<FileDrop
		bind:this={drop}
		accept="image/*"
		label={ru.media.photoZone}
		showChoose={false}
		onfiles={addFiles}
	>
		{@render strips()}
	</FileDrop>
	<CameraDialog
		bind:open={cameraOpen}
		max={MAX_PHOTOS_PER_ADD - shots}
		oncapture={(file) => {
			shots += 1;
			void media.add([file]);
		}}
	/>
{/if}
{#if media.error}
	<p class="error" role="alert">
		{ru.media.loadFailed}
		<Button size="sm" onclick={() => void media.reload()}>{ru.common.retry}</Button>
	</p>
{/if}

<PhotoViewer
	photos={media.photos}
	bind:index={viewerIndex}
	bind:open={viewerOpen}
	{readonly}
	oncaption={(id, text) => void media.caption(id, text)}
	ondelete={(id) => void media.remove(id)}
/>

<style>
	.error {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
		margin: 0;
		color: var(--danger-text);
		font-size: var(--text-sm);
	}
</style>
