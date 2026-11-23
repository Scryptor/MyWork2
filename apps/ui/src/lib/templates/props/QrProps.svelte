<!-- The properties of a QR code: where the link comes from (a link of the firm or one's own, with fields
     of the order), the side of the square in millimetres, the caption under it and the alignment. A link
     of the firm that is not filled in prints nothing: the hint says so and leads to where it is set. -->
<script lang="ts">
	import { resolve } from '$app/paths';
	import type { Align } from '$api/Align';
	import { app } from '#lib/app/context.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import Callout from '#lib/ui/Callout.svelte';
	import SegmentedControl from '#lib/ui/SegmentedControl.svelte';
	import Select from '#lib/ui/Select.svelte';
	import TextInput from '#lib/ui/TextInput.svelte';
	import type { TemplateEditor } from '../editor.svelte.js';
	import InsertField from '../InsertField.svelte';
	import { blockAt, replaceBlock, type BlockPath } from '../ops.js';
	import { choosePreset, qrPreset, QR_PRESETS, type QrBody, type QrPreset } from '../qr.js';
	import NumberField from './NumberField.svelte';

	let { editor, path }: { editor: TemplateEditor; path: BlockPath } = $props();

	const t = ru.templates;
	const block = $derived(blockAt(editor.template, path));
	const body = $derived(block?.body.type === 'qr' ? block.body : null);
	const key = $derived(path.join('.'));
	const preset = $derived(body ? qrPreset(body.link) : null);
	const unset = $derived(preset !== null && !app.settings[QR_PRESETS[preset].setting]?.trim());
	let linkField = $state<HTMLInputElement | null>(null);

	/** A change of the block; typed fields pass their key, so their keys make one step. */
	function set(patch: Partial<Omit<QrBody, 'type'>>, field?: string) {
		if (!block || !body) return;
		const next = { ...block, body: { ...body, ...patch } };
		editor.edit(
			replaceBlock(editor.template, path, next),
			field === undefined ? undefined : `${key}.${field}`
		);
	}

	function choose(value: QrPreset | 'custom' | null) {
		if (!block || !body || value === null) return;
		editor.edit(replaceBlock(editor.template, path, { ...block, body: choosePreset(body, value) }));
	}

	const sources = [...(Object.keys(QR_PRESETS) as QrPreset[]), 'custom' as const].map((value) => ({
		value,
		label: t.qrPresets[value]
	}));
	const ALIGNS: Align[] = ['left', 'center', 'right'];
	const aligns = ALIGNS.map((value) => ({ value, label: t.aligns[value] }));
</script>

{#if body}
	<div class="props">
		<Select label={t.qrLink} items={sources} bind:value={() => preset ?? 'custom', choose} />
		{#if preset === null}
			<div class="one">
				<TextInput
					label={t.qrLink}
					hint={t.qrLinkHint}
					bind:ref={linkField}
					bind:value={() => body.link, (v) => set({ link: v }, 'link')}
				/>
				<InsertField
					name={t.qrLink}
					target={linkField}
					value={body.link}
					onchange={(v) => set({ link: v }, 'link')}
				/>
			</div>
		{/if}
		<NumberField
			label={t.qrSize}
			value={body.size_mm}
			min={10}
			max={100}
			onchange={(n) => n !== null && set({ size_mm: n }, 'size_mm')}
		/>
		<TextInput
			label={t.qrCaption}
			maxlength={200}
			bind:value={() => body.caption, (v) => set({ caption: v }, 'caption')}
		/>
		<div class="field">
			<span class="label">{t.align}</span>
			<SegmentedControl
				label={t.align}
				options={aligns}
				bind:value={() => body.align, (v) => set({ align: v })}
			/>
		</div>
		{#if unset}
			<Callout tone="info">
				{t.qrMissing}
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
	.one {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
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
