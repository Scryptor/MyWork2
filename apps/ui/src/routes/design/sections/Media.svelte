<script lang="ts">
	import { onMount } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import type { StatusHue } from '#lib/design/status-colors.js';
	import { DEMO_STATUSES } from '#lib/demo/fake.js';
	import { demoPhoto } from '#lib/demo/photos.js';
	import type { Attachment } from '$api/Attachment';
	import {
		Button,
		CameraDialog,
		FileDrop,
		FileList,
		PhotoStrip,
		PhotoViewer,
		StatusPicker
	} from '#lib/ui/index.js';

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

	// Attachments without a server: demo pictures answer for the viewer.
	const attachment = (id: number, name: string, over: Partial<Attachment> = {}): Attachment => ({
		id,
		order_id: 1,
		kind: 'photo',
		name,
		caption: '',
		content_type: 'image/png',
		size: 120_000,
		width: 160,
		height: 160,
		legacy: false,
		missing: false,
		created_at: '2026-03-07 10:30:00',
		...over
	});
	let attached = $state.raw<Attachment[]>([
		attachment(1, 'ekran.png', { caption: 'Экран' }),
		attachment(2, 'kryshka.png', { caption: 'Задняя крышка' }),
		attachment(3, 'razyom.png', { caption: 'Разъём зарядки' })
	]);
	const demoFiles: Attachment[] = [
		attachment(11, 'Акт.pdf', { kind: 'file', content_type: 'application/pdf', size: 1_234_567 }),
		attachment(12, 'Квитанция.pdf', {
			kind: 'file',
			content_type: 'application/pdf',
			missing: true
		})
	];
	const demoUrls = new SvelteMap<number, string>();
	async function resolveDemo(id: number): Promise<string> {
		let url = demoUrls.get(id);
		if (!url) {
			url = demoPhoto(SAMPLES[(id - 1) % SAMPLES.length]![1], 640);
			demoUrls.set(id, url);
		}
		return url;
	}
	let viewerOpen = $state(false);
	let viewerIndex = $state(0);
	let savedCaption = $state('');
	let cameraOpen = $state(false);
	let shot = $state(0);
	let downloaded = $state('');
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

<h3>Просмотр фото</h3>
<p class="muted">
	Стрелки ← → листают, Esc закрывает, подпись сохраняется при уходе из поля и по Enter.
</p>
<div class="stack">
	<ul class="openers">
		{#each attached as a, i (a.id)}
			<li>
				<Button
					onclick={() => {
						viewerIndex = i;
						viewerOpen = true;
					}}
				>
					Открыть фото: {a.caption}
				</Button>
			</li>
		{/each}
	</ul>
	<p class="muted">Подпись сохранена: {savedCaption || 'ещё нет'}</p>
</div>
<PhotoViewer
	photos={attached}
	bind:index={viewerIndex}
	bind:open={viewerOpen}
	resolveUrl={resolveDemo}
	oncaption={(id, text) => {
		attached = attached.map((a) => (a.id === id ? { ...a, caption: text } : a));
		savedCaption = text;
	}}
	ondelete={(id) => (attached = attached.filter((a) => a.id !== id))}
/>

<h3>Камера</h3>
<div class="stack">
	<div><Button onclick={() => (cameraOpen = true)}>Открыть камеру</Button></div>
	<p class="muted">Снято: {shot}. Пробел тоже снимает; снимков можно сделать несколько подряд.</p>
</div>
<CameraDialog bind:open={cameraOpen} max={MAX - shot} oncapture={() => (shot += 1)} />

<h3>Список файлов</h3>
<FileList files={demoFiles} ondownload={(f) => (downloaded = f.name)} ondelete={() => {}} />
<p class="muted">Скачано: {downloaded || 'ничего'}</p>

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
	<p class="muted">
		Смена: {change || 'ещё не было'}. Стрелки двигают фокус, пробел или Enter выбирают; статусы
		после седьмого — в «Ещё».
	</p>
	<StatusPicker statuses={DEMO_STATUSES} value={8} label="Статус выданного заказа" disabled />
</div>

<style>
	.muted {
		color: var(--text-3);
	}
	p.muted {
		max-width: 48rem;
	}
	.openers {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
		margin: 0;
		padding: 0;
		list-style: none;
	}
	.stack {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}
</style>
