<!-- The properties of a «Typst-код» block: its code (typing in it is one step to undo per pause), the
     errors of the newest preview that are in this block, by line, and «Вставить поле», which puts
     `#v("order.number")` where the caret was (`v("order.number")` in code mode: `#{…}`, `if … {…}`). -->
<script lang="ts">
	import type { DocumentField } from '$api/DocumentField';
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';
	import CodeEditor from '#lib/ui/CodeEditor.svelte';
	import { CODE_MAX } from '../defaults.js';
	import { diagnosticText, type TemplateEditor } from '../editor.svelte.js';
	import InsertField from '../InsertField.svelte';
	import { inTypstCode } from '../insert.js';
	import { blockAt, replaceBlock, type BlockPath } from '../ops.js';
	import TypstHelp from '../TypstHelp.svelte';

	let { editor, path }: { editor: TemplateEditor; path: BlockPath } = $props();

	const t = ru.templates;

	const block = $derived(blockAt(editor.template, path));
	const body = $derived(block?.body.type === 'typst_code' ? block.body : null);
	const key = $derived(path.join('.'));
	let field = $state<HTMLTextAreaElement | null>(null);
	let helpOpen = $state(false);

	const samePath = (a: readonly number[], b: BlockPath) =>
		a.length === b.length && a.every((n, i) => n === b[i]);

	/** The errors of the newest preview that point into this block. */
	const errors = $derived(
		editor.diagnostics.flatMap((d) =>
			d.place !== null && samePath(d.place.block, path)
				? [{ line: d.place.line, column: d.place.column, message: diagnosticText(d) }]
				: []
		)
	);

	/** A field in own code: its value by its key; `#` only in markup, in code mode it is an error. */
	const asCode = (f: DocumentField, before: string) =>
		`${inTypstCode(before) ? '' : '#'}v("${f.key}")`;

	function set(code: string) {
		if (!block || !body) return;
		editor.edit(
			replaceBlock(editor.template, path, { ...block, body: { ...body, code } }),
			`${key}.code`
		);
	}
</script>

{#if body}
	<div class="props">
		<CodeEditor
			label={t.code}
			hint={t.codeHelp}
			bind:ref={field}
			{errors}
			maxlength={CODE_MAX}
			bind:value={() => body.code, set}
		/>
		<InsertField target={field} value={body.code} onchange={set} format={asCode} />
		<div>
			<Button onclick={() => (helpOpen = true)}>{t.typstHelpOpen}</Button>
		</div>
		<TypstHelp bind:open={helpOpen} />
	</div>
{/if}

<style>
	.props {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
	}
</style>
