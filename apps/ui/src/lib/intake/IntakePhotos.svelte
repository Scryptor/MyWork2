<!-- «Фото» of the intake (spec 6.5): the computer's camera and files, also dropped or pasted. They go to
     the draft on the server at once, ten at most, and become the order's photos with «Принять». -->
<script lang="ts" module>
	/** Photos of one intake: the server keeps no more in a draft. */
	export const MAX_INTAKE_PHOTOS = 10;
</script>

<script lang="ts">
	import Camera from '@lucide/svelte/icons/camera';
	import Smartphone from '@lucide/svelte/icons/smartphone';
	import Upload from '@lucide/svelte/icons/upload';
	import { ru } from '#lib/i18n/ru.js';
	import { limitPhotos, type Media } from '#lib/orders/photos.svelte.js';
	import AttachmentImage from '#lib/ui/AttachmentImage.svelte';
	import Button from '#lib/ui/Button.svelte';
	import CameraDialog from '#lib/ui/CameraDialog.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import FileDrop from '#lib/ui/FileDrop.svelte';
	import Kbd from '#lib/ui/Kbd.svelte';
	import PhotoStrip from '#lib/ui/PhotoStrip.svelte';
	import PhotoViewer from '#lib/ui/PhotoViewer.svelte';
	import Section from '#lib/ui/Section.svelte';

	let {
		media,
		locked = false,
		reason = null,
		onadd
	}: {
		/** The photos of the draft (`createDraftMedia`). */
		media: Media;
		/**
		 * Nothing is added or removed: the order is on its way, or may already be there — then its photos
		 * are what the draft had, and `reason` says why.
		 */
		locked?: boolean;
		reason?: 'accepted' | 'unanswered' | null;
		/** Photos are on their way to the draft: it is worth keeping now. */
		onadd?: () => void;
	} = $props();

	const t = ru.intake;

	let drop = $state<{ open: () => void } | null>(null);
	let cameraOpen = $state(false);
	let phoneOpen = $state(false);
	let viewerOpen = $state(false);
	let viewerIndex = $state(0);

	const waiting = $derived(media.pending.filter((p) => p.photo));
	/** Room left in the draft: the photos there and those on their way count. */
	const room = $derived(Math.max(0, MAX_INTAKE_PHOTOS - media.photos.length - waiting.length));
	const strip = $derived(
		media.photos.map((a) => ({ id: String(a.id), src: '', alt: a.caption || a.name }))
	);

	// «Принять» takes the photos the draft has: the camera does not go on shooting meanwhile.
	$effect(() => {
		if (locked) cameraOpen = false;
	});

	function add(files: File[]) {
		if (locked) return;
		const fit = limitPhotos(files, room, t.photoLimit);
		if (fit.length === 0) return;
		onadd?.();
		void media.add(fit);
	}
</script>

{#snippet picture(photo: { id: string; alt: string })}
	<AttachmentImage
		id={Number(photo.id)}
		variant="thumb"
		alt={photo.alt}
		missing={media.photos.find((a) => String(a.id) === photo.id)?.missing ?? false}
	/>
{/snippet}

<Section title={t.photos}>
	<FileDrop
		bind:this={drop}
		accept="image/*"
		label={ru.media.photoZone}
		showChoose={false}
		disabled={locked}
		onfiles={add}
	>
		<PhotoStrip
			photos={strip}
			max={MAX_INTAKE_PHOTOS}
			pending={waiting}
			image={picture}
			onopen={(id) => {
				viewerIndex = Math.max(
					0,
					strip.findIndex((p) => p.id === id)
				);
				viewerOpen = true;
			}}
			onremove={locked ? undefined : (id) => void media.remove(Number(id))}
		/>
		<div class="actions">
			<Button
				size="sm"
				icon={Camera}
				disabled={locked || room === 0}
				onclick={() => (cameraOpen = true)}>{ru.media.takeWithCamera}</Button
			>
			<Button size="sm" icon={Upload} disabled={locked || room === 0} onclick={() => drop?.open()}
				>{ru.media.chooseFiles}</Button
			>
			<!-- Shooting with a phone needs the host mode (plan 10): until then the button says how. -->
			<Button size="sm" variant="ghost" icon={Smartphone} onclick={() => (phoneOpen = true)}
				>{t.takeWithPhone}</Button
			>
			{#if reason === 'accepted'}
				<span class="hint" role="status">{t.photosAccepted}</span>
			{:else if reason === 'unanswered'}
				<span class="hint" role="status">{t.photosUnanswered}</span>
			{:else if !locked}
				<span class="hint">{ru.media.dropHint} <Kbd shortcut="Mod+V" /></span>
			{/if}
		</div>
	</FileDrop>
	{#if media.error}
		<p class="error" role="alert">
			{ru.media.loadFailed}
			<Button size="sm" onclick={() => void media.reload()}>{ru.common.retry}</Button>
		</p>
	{/if}
</Section>

<CameraDialog bind:open={cameraOpen} max={room} oncapture={(file) => add([file])} />

<PhotoViewer
	photos={media.photos}
	bind:index={viewerIndex}
	bind:open={viewerOpen}
	readonly={locked}
	oncaption={(id, text) => void media.caption(id, text)}
	ondelete={(id) => void media.remove(id)}
/>

<Dialog bind:open={phoneOpen} title={t.takeWithPhone} size="sm">
	<p class="how">{t.phoneHowTo}</p>
	{#snippet footer()}
		<Button variant="primary" onclick={() => (phoneOpen = false)}>{ru.common.understood}</Button>
	{/snippet}
</Dialog>

<style>
	.actions {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
	}
	.hint {
		color: var(--text-3);
		font-size: var(--text-sm);
	}
	.how {
		margin: 0;
	}
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
