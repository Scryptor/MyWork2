<script lang="ts">
	import { onMount } from 'svelte';
	import type { StatusHue } from '#lib/design/status-colors.js';
	import { DEMO_STATUSES } from '#lib/demo/fake.js';
	import { demoPhoto } from '#lib/demo/photos.js';
	import { FileDrop, PhotoStrip, StatusPicker } from '#lib/ui/index.js';

	interface Photo {
		id: string;
		src: string;
		alt: string;
	}

	const SAMPLES: [string, StatusHue][] = [
		['Экран', 'teal'],
		['Задняя крышка', 'violet'],
		['Разъём зарядки', 'amber']
	];
	const MAX = 10;
	const nameOf = (id: number) => DEMO_STATUSES.find((s) => s.id === id)?.name ?? '';

	let photos = $state.raw<Photo[]>([]);
	let opened = $state('');
	let files = $state.raw<string[]>([]);
	let status = $state(1);
	let change = $state('');
	let drop = $state<ReturnType<typeof FileDrop>>();
	let seq = 0;
	// Pictures chosen here live as object URLs until they are removed or the page goes.
	let urls: string[] = [];

	onMount(() => {
		photos = SAMPLES.map(([alt, hue], i) => ({ id: `sample-${i}`, src: demoPhoto(hue), alt }));
		return () => urls.forEach((u) => URL.revokeObjectURL(u));
	});

	function addPhotos(chosen: File[]) {
		const added = chosen.slice(0, MAX - photos.length).map((f) => {
			const src = URL.createObjectURL(f);
			urls.push(src);
			return { id: `photo-${++seq}`, src, alt: f.name };
		});
		photos = [...photos, ...added];
	}
	function removePhoto(id: string) {
		const photo = photos.find((p) => p.id === id);
		if (photo && urls.includes(photo.src)) {
			URL.revokeObjectURL(photo.src);
			urls = urls.filter((u) => u !== photo.src);
		}
		photos = photos.filter((p) => p.id !== id);
	}
</script>

<h3>Фото</h3>
<p class="muted">
	«＋» и «Выбрать файлы» открывают выбор картинок; их можно перетащить на рамку или вставить из
	буфера. Плиток не больше {MAX}: на последней «＋» пропадает.
</p>
<FileDrop bind:this={drop} onfiles={addPhotos}>
	<PhotoStrip
		{photos}
		max={MAX}
		onadd={() => drop?.open()}
		onremove={removePhoto}
		onopen={(id) => (opened = photos.find((p) => p.id === id)?.alt ?? '')}
	/>
</FileDrop>
<p class="muted">Открыто: {opened || 'ничего'}</p>

<h3>Файлы</h3>
<FileDrop accept=".pdf,image/*" onfiles={(chosen) => (files = chosen.map((f) => f.name))} />
<p class="muted">Выбрано: {files.length > 0 ? files.join(', ') : 'ничего'}</p>

<h3>Статус</h3>
<div class="stack">
	<StatusPicker
		statuses={DEMO_STATUSES}
		value={status}
		onchange={(id, previous) => {
			status = id;
			change = `${nameOf(previous)} → ${nameOf(id)}`;
		}}
	/>
	<p class="muted">Смена: {change || 'ещё не было'}. Стрелки меняют выбор.</p>
	<StatusPicker statuses={DEMO_STATUSES} value={8} label="Статус выданного заказа" disabled />
</div>

<style>
	.muted {
		color: var(--text-3);
	}
	p.muted {
		max-width: 48rem;
	}
	.stack {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}
</style>
