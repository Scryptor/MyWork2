<!-- The properties of the logo: its size in millimetres (the picture keeps its proportions within them)
     and the alignment. Without a logo on the server the block is not printed: the hint says so and leads
     to where it is uploaded. -->
<script lang="ts">
	import { onMount } from 'svelte';
	import { resolve } from '$app/paths';
	import type { Align } from '$api/Align';
	import type { BlockBody } from '$api/BlockBody';
	import { getLogoInfo } from '#lib/api/logo.js';
	import SegmentedControl from '#lib/ui/SegmentedControl.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import NumberField from './NumberField.svelte';
	import { ru } from '#lib/i18n/ru.js';
	import type { TemplateEditor } from '../editor.svelte.js';
	import { blockAt, replaceBlock, type BlockPath } from '../ops.js';

	type Body = Extract<BlockBody, { type: 'logo' }>;

	let { editor, path }: { editor: TemplateEditor; path: BlockPath } = $props();

	const t = ru.templates;
	const block = $derived(blockAt(editor.template, path));
	const body = $derived(block?.body.type === 'logo' ? block.body : null);
	const key = $derived(path.join('.'));
	/** False until the answer says there is none: no hint flashes meanwhile. */
	let missing = $state(false);

	onMount(() => {
		let alive = true;
		getLogoInfo().then(
			(info) => alive && (missing = info === null),
			// Unknown: the hint would be a guess.
			() => {}
		);
		return () => (alive = false);
	});

	/** A change of the block; typed fields pass their key, so their keys make one step. */
	function set(patch: Partial<Omit<Body, 'type'>>, field?: string) {
		if (!block || !body) return;
		const next = { ...block, body: { ...body, ...patch } };
		editor.edit(
			replaceBlock(editor.template, path, next),
			field === undefined ? undefined : `${key}.${field}`
		);
	}

	const ALIGNS: Align[] = ['left', 'center', 'right'];
	const aligns = ALIGNS.map((value) => ({ value, label: t.aligns[value] }));
</script>

{#if body}
	<div class="props">
		<NumberField
			label={t.logoWidth}
			value={body.width_mm}
			min={5}
			max={200}
			onchange={(n) => n !== null && set({ width_mm: n }, 'width_mm')}
		/>
		<NumberField
			label={t.logoHeight}
			value={body.height_mm}
			min={5}
			max={100}
			onchange={(n) => n !== null && set({ height_mm: n }, 'height_mm')}
		/>
		<div class="field">
			<span class="label">{t.align}</span>
			<SegmentedControl
				label={t.align}
				options={aligns}
				bind:value={() => body.align, (v) => set({ align: v })}
			/>
		</div>
		{#if missing}
			<Callout tone="info">
				{t.logoMissing}
				<a href={resolve('settings/documents')}>{t.logoMissingLink}</a>
			</Callout>
		{/if}
	</div>
{/if}

<style>
	.props {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
	}
	.field {
		display: flex;
		flex-direction: column;
		gap: var(--space-1-5);
	}
	.label {
		font-size: var(--text-sm);
		font-weight: 500;
	}
</style>
