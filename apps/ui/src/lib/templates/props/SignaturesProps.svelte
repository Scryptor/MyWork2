<!-- The properties of the signatures: the text under the left and under the right line. -->
<script lang="ts">
	import type { BlockBody } from '$api/BlockBody';
	import TextInput from '#lib/ui/TextInput.svelte';
	import InsertField from '../InsertField.svelte';
	import { ru } from '#lib/i18n/ru.js';
	import type { TemplateEditor } from '../editor.svelte.js';
	import { blockAt, replaceBlock, type BlockPath } from '../ops.js';

	type Body = Extract<BlockBody, { type: 'signatures' }>;

	let { editor, path }: { editor: TemplateEditor; path: BlockPath } = $props();

	const t = ru.templates;
	const block = $derived(blockAt(editor.template, path));
	const body = $derived(block?.body.type === 'signatures' ? block.body : null);
	const key = $derived(path.join('.'));

	/** A change of the block; typed fields pass their key, so their keys make one step. */
	function set(patch: Partial<Omit<Body, 'type'>>, field?: string) {
		if (!block || !body) return;
		const next = { ...block, body: { ...body, ...patch } };
		editor.edit(
			replaceBlock(editor.template, path, next),
			field === undefined ? undefined : `${key}.${field}`
		);
	}

	let leftField = $state<HTMLInputElement | null>(null);
	let rightField = $state<HTMLInputElement | null>(null);
</script>

{#if body}
	<div class="props">
		<div class="one">
			<TextInput
				label={t.signatureLeft}
				maxlength={200}
				bind:ref={leftField}
				bind:value={() => body.left, (v) => set({ left: v }, 'left')}
			/>
			<InsertField
				kind={editor.template.kind}
				name={t.signatureLeft}
				target={leftField}
				value={body.left}
				onchange={(v) => set({ left: v }, 'left')}
			/>
		</div>
		<div class="one">
			<TextInput
				label={t.signatureRight}
				maxlength={200}
				bind:ref={rightField}
				bind:value={() => body.right, (v) => set({ right: v }, 'right')}
			/>
			<InsertField
				kind={editor.template.kind}
				name={t.signatureRight}
				target={rightField}
				value={body.right}
				onchange={(v) => set({ right: v }, 'right')}
			/>
		</div>
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
		gap: var(--space-1);
	}
</style>
