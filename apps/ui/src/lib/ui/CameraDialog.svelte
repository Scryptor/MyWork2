<!-- Live camera in a dialog: «Снять» (or Space) shoots a JPEG into `oncapture`, several in a row. -->
<script lang="ts">
	import Camera from '@lucide/svelte/icons/camera';
	import { untrack } from 'svelte';
	import { nowLocal } from '#lib/format/date.js';
	import { ru } from '#lib/i18n/ru.js';
	import Button from './Button.svelte';
	import { cameraErrorText, cameraFileName } from './camera.js';
	import Dialog from './Dialog.svelte';
	import Select from './Select.svelte';

	let {
		open = $bindable(false),
		max,
		oncapture
	}: {
		open: boolean;
		/** How many more pictures may be taken; the parent lowers it as they are added. */
		max: number;
		oncapture: (file: File) => void;
	} = $props();

	let video = $state<HTMLVideoElement | null>(null);
	let stream = $state.raw<MediaStream | null>(null);
	let devices = $state.raw<MediaDeviceInfo[]>([]);
	let deviceId = $state('');
	// The chooser binds to this; picking a camera restarts the stream through `deviceId`.
	let chosen = $state<string | null>(null);
	let error = $state('');
	let ready = $state(false);
	let shots = $state(0);

	function stop() {
		stream?.getTracks().forEach((t) => t.stop());
		stream = null;
		ready = false;
	}

	// Starts the camera when the dialog opens (or another camera is chosen), stops it when it closes.
	$effect(() => {
		if (!open) return;
		const wanted = deviceId;
		let cancelled = false;
		untrack(() => {
			error = '';
			shots = 0;
		});
		(async () => {
			try {
				if (!navigator.mediaDevices?.getUserMedia) throw new Error('no mediaDevices');
				const next = await navigator.mediaDevices.getUserMedia({
					video: {
						...(wanted ? { deviceId: { exact: wanted } } : {}),
						width: { ideal: 1920 },
						height: { ideal: 1080 }
					},
					audio: false
				});
				if (cancelled) {
					next.getTracks().forEach((t) => t.stop());
					return;
				}
				stream = next;
				// Labels of cameras appear only after permission is given.
				const all = await navigator.mediaDevices.enumerateDevices();
				if (!cancelled) devices = all.filter((d) => d.kind === 'videoinput');
			} catch (e) {
				if (!cancelled) error = cameraErrorText(e);
			}
		})();
		return () => {
			cancelled = true;
			untrack(stop);
		};
	});

	$effect(() => {
		if (video) video.srcObject = stream;
	});

	$effect(() => {
		if (chosen && chosen !== deviceId) deviceId = chosen;
	});
	$effect(() => {
		if (!chosen && devices.length > 0) chosen = deviceId || devices[0]!.deviceId;
	});

	const canShoot = $derived(ready && !error && max > 0);

	function shoot() {
		if (!canShoot || !video || video.videoWidth === 0) return;
		const canvas = document.createElement('canvas');
		canvas.width = video.videoWidth;
		canvas.height = video.videoHeight;
		canvas.getContext('2d')?.drawImage(video, 0, 0);
		canvas.toBlob(
			(blob) => {
				if (!blob) return;
				shots += 1;
				oncapture(new File([blob], cameraFileName(nowLocal()), { type: 'image/jpeg' }));
			},
			'image/jpeg',
			0.9
		);
	}

	function onkeydown(e: KeyboardEvent) {
		if (!open || e.code !== 'Space' || e.repeat || e.defaultPrevented) return;
		// Buttons, the camera chooser and fields take Space for themselves.
		if (
			e.target instanceof Element &&
			e.target.closest('button, select, input, textarea, [role="combobox"], [role="option"]')
		) {
			return;
		}
		e.preventDefault();
		shoot();
	}
</script>

<svelte:window {onkeydown} />

<Dialog bind:open title={ru.camera.title} size="lg">
	<div class="camera">
		{#if error}
			<p class="error" role="alert">{error}</p>
		{:else}
			<video
				bind:this={video}
				class="live"
				autoplay
				muted
				playsinline
				onloadedmetadata={() => (ready = true)}
			></video>
		{/if}
		{#if devices.length > 1 && !error}
			<Select
				label={ru.camera.choose}
				bind:value={chosen}
				items={devices.map((d, i) => ({
					value: d.deviceId,
					label: d.label || `${ru.camera.choose} ${i + 1}`
				}))}
			/>
		{/if}
		{#if shots > 0}<p class="shots num">{ru.camera.shot}: {shots}</p>{/if}
	</div>
	{#snippet footer()}
		<Button icon={Camera} variant="primary" disabled={!canShoot} onclick={shoot}>
			{ru.camera.shoot}
		</Button>
		<Button onclick={() => (open = false)}>{ru.camera.done}</Button>
	{/snippet}
</Dialog>

<style>
	.camera {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}
	.live {
		display: block;
		width: 100%;
		max-height: 60vh;
		border-radius: var(--radius-md);
		background: var(--surface-2);
		object-fit: contain;
	}
	.error {
		margin: 0;
		padding: var(--space-4);
		border-radius: var(--radius-md);
		background: var(--danger-soft);
		color: var(--danger-text);
	}
	.shots {
		margin: 0;
		color: var(--text-3);
		font-size: var(--text-sm);
	}
</style>
