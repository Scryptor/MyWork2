<!-- The properties of a QR code: where the link comes from (a link of the firm's list or one's own, with
     fields of the order), the side of the square in millimetres, the caption under it and the alignment. A
     link of the firm that is empty, switched off or gone from the list prints nothing: the hint says so
     and leads to where the links are set. -->
<script lang="ts">
	import { untrack } from 'svelte';
	import { resolve } from '$app/paths';
	import type { Align } from '$api/Align';
	import { app } from '#lib/app/context.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import Callout from '#lib/ui/Callout.svelte';
	import SegmentedControl from '#lib/ui/SegmentedControl.svelte';
	import Select from '#lib/ui/Select.svelte';
	import TextInput from '#lib/ui/TextInput.svelte';
	import { QR_LINK_MAX } from '../defaults.js';
	import type { TemplateEditor } from '../editor.svelte.js';
	import InsertField from '../InsertField.svelte';
	import { blockAt, replaceBlock, type BlockPath } from '../ops.js';
	import { chooseLink, linkName, qrLinkId, type QrBody } from '../qr.js';
	import NumberField from './NumberField.svelte';

	let { editor, path }: { editor: TemplateEditor; path: BlockPath } = $props();

	const t = ru.templates;
	const block = $derived(blockAt(editor.template, path));
	const body = $derived(block?.body.type === 'qr' ? block.body : null);
	const key = $derived(path.join('.'));
	const links = $derived(app.settings.firm_links);
	/** The id of the link of the list the block takes, null for a link of its own. */
	const linkId = $derived(body ? qrLinkId(body.link) : null);
	const firmLink = $derived(links.find((l) => l.id === linkId));
	/** A link the list lacks: the hint around the link to the settings, where it can be added again. */
	const gone = $derived(linkId !== null && !firmLink ? t.qrLinkGone(linkName(linkId)) : null);
	/** What else keeps the code from printing, as the hint says it. */
	const problem = $derived.by(() => {
		if (linkId === null || !firmLink) return null;
		if (!firmLink.enabled) return t.qrLinkOff;
		return firmLink.url.trim() ? null : t.qrMissing;
	});
	let linkField = $state<HTMLInputElement | null>(null);
	/**
	 * The link written here as «своя» (by «Своя ссылка» or in the field; the one the block opened with,
	 * when it is not a firm's): while the block's link is still it, the link stays «своя» even when it
	 * reads as a firm's one, so the field does not vanish under the caret. A link picked in the list
	 * ends it; so does an undo to another link, which shows as what it is.
	 */
	let own = $state<string | null>(untrack(() => (body && linkId === null ? body.link : null)));
	const custom = $derived(linkId === null || (body !== null && own === body.link));

	/** The link typed (or a field put) into the field. */
	function write(link: string) {
		own = link;
		set({ link }, 'link');
	}

	/** A change of the block; typed fields pass their key, so their keys make one step. */
	function set(patch: Partial<Omit<QrBody, 'type'>>, field?: string) {
		if (!block || !body) return;
		const next = { ...block, body: { ...body, ...patch } };
		editor.edit(
			replaceBlock(editor.template, path, next),
			field === undefined ? undefined : `${key}.${field}`
		);
	}

	function choose(value: string | null) {
		if (!block || !body || value === null) return;
		// «Своя ссылка» again leaves the link as it is typed.
		if (value === 'custom' && custom) return;
		const next = chooseLink(body, value, links);
		own = value === 'custom' ? next.link : null;
		editor.edit(replaceBlock(editor.template, path, { ...block, body: next }));
	}

	/** The links of the list in their order, the one of an old block that the list lacks (by its name), «своя ссылка». */
	const sources = $derived([
		...links.map((l) => ({ value: l.id, label: l.enabled ? l.name : t.qrLinkOffItem(l.name) })),
		...(linkId !== null && !firmLink ? [{ value: linkId, label: linkName(linkId) }] : []),
		{ value: 'custom', label: t.qrCustom }
	]);
	const ALIGNS: Align[] = ['left', 'center', 'right'];
	const aligns = ALIGNS.map((value) => ({ value, label: t.aligns[value] }));
</script>

{#if body}
	<div class="props">
		<Select
			label={t.qrSource}
			items={sources}
			bind:value={() => (custom || linkId === null ? 'custom' : linkId), choose}
		/>
		{#if custom}
			<div class="one">
				<TextInput
					label={t.qrLink}
					hint={t.qrLinkHint}
					maxlength={QR_LINK_MAX}
					bind:ref={linkField}
					bind:value={() => body.link, write}
				/>
				<InsertField name={t.qrLink} target={linkField} value={body.link} onchange={write} />
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
		{#if gone}
			<Callout tone="info">
				{gone[0]}<a href={resolve('settings/documents')}>{t.documentsSettingsLink}</a>{gone[1]}
			</Callout>
		{:else if problem}
			<Callout tone="info">
				{problem}
				<a href={resolve('settings/documents')}>{t.documentsSettingsLink}</a>
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
