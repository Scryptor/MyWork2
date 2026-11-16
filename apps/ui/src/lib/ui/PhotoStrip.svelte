<!-- Device photos: thumbnails with a remove button, a «＋» tile while there is room, «3 из 10». -->
<script lang="ts">
	import Plus from '@lucide/svelte/icons/plus';
	import X from '@lucide/svelte/icons/x';
	import { ru } from '#lib/i18n/ru.js';

	let {
		photos,
		max = 10,
		label = ru.media.photos,
		onadd,
		onremove,
		onopen
	}: {
		photos: { id: string; src: string; alt: string }[];
		max?: number;
		label?: string;
		onadd?: () => void;
		onremove?: (id: string) => void;
		onopen?: (id: string) => void;
	} = $props();

	const labelId = $props.id();
</script>

<div class="strip" role="group" aria-labelledby={labelId}>
	<div class="head">
		<span class="label-caps" id={labelId}>{label}</span>
		<span class="count num">{photos.length} {ru.media.of} {max}</span>
	</div>
	<ul class="tiles">
		{#each photos as photo (photo.id)}
			<li class="tile">
				{#if onopen}
					<button type="button" class="thumb" onclick={() => onopen(photo.id)}>
						<img src={photo.src} alt={photo.alt} />
					</button>
				{:else}
					<img class="thumb" src={photo.src} alt={photo.alt} />
				{/if}
				{#if onremove}
					<button
						type="button"
						class="remove"
						aria-label="{ru.media.removePhoto}: {photo.alt}"
						onclick={() => onremove(photo.id)}
					>
						<X size={14} aria-hidden="true" />
					</button>
				{/if}
			</li>
		{/each}
		{#if onadd && photos.length < max}
			<li class="tile">
				<button type="button" class="add" aria-label={ru.media.addPhoto} onclick={onadd}>
					<Plus size={20} aria-hidden="true" />
				</button>
			</li>
		{/if}
	</ul>
</div>

<style>
	.strip {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		min-width: 0;
	}
	.head {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: var(--space-2);
	}
	.count {
		color: var(--text-3);
		font-size: var(--text-sm);
	}
	.tiles {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
		margin: 0;
		padding: var(--space-1-5) 0 0;
		list-style: none;
	}
	.tile {
		position: relative;
		width: 4.5rem;
		height: 4.5rem;
	}
	.thumb {
		display: block;
		box-sizing: border-box;
		width: 100%;
		height: 100%;
		padding: 0;
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
		background: var(--surface-2);
		overflow: hidden;
	}
	button.thumb {
		cursor: zoom-in;
	}
	img.thumb,
	.thumb img {
		display: block;
		width: 100%;
		height: 100%;
		object-fit: cover;
	}
	.remove {
		position: absolute;
		top: calc(-1 * var(--space-1-5));
		right: calc(-1 * var(--space-1-5));
		display: inline-flex;
		align-items: center;
		justify-content: center;
		box-sizing: border-box;
		width: 1.5rem;
		height: 1.5rem;
		padding: 0;
		border: 1px solid var(--border-strong);
		border-radius: var(--radius-full);
		background: var(--surface);
		color: var(--text-2);
		box-shadow: var(--shadow-sm);
		cursor: pointer;
	}
	/* A bigger target than the visible circle. */
	.remove::before {
		content: '';
		position: absolute;
		inset: calc(-1 * var(--space-1-5));
	}
	.remove:hover {
		background: var(--danger-soft);
		color: var(--danger-text);
	}
	.add {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		box-sizing: border-box;
		width: 100%;
		height: 100%;
		padding: 0;
		border: 1px dashed var(--border-strong);
		border-radius: var(--radius-md);
		background: var(--surface);
		color: var(--text-2);
		cursor: pointer;
	}
	.add:hover {
		background: var(--surface-3);
		color: var(--text);
	}
	@media (pointer: coarse) {
		.tile {
			width: 5rem;
			height: 5rem;
		}
		/* 1.5rem + 2 × 0.625rem = 2.75rem. */
		.remove::before {
			inset: calc(-1 * (var(--space-2) + var(--space-0-5)));
		}
	}
</style>
