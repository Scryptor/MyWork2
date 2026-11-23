<script lang="ts">
	import { ru } from '#lib/i18n/ru.js';
	import Field from './Field.svelte';

	let {
		value = $bindable(''),
		ref = $bindable(null),
		label,
		hint,
		errors = [],
		maxlength,
		rows = 16,
		onblur,
		id
	}: {
		value: string;
		/** The element itself, for a caret or a focus the caller moves. */
		ref?: HTMLTextAreaElement | null;
		label: string;
		hint?: string;
		/** Compile errors; `line` and `column` count from 1. */
		errors?: readonly { line: number; column: number; message: string }[];
		maxlength?: number;
		/** Visible lines. */
		rows?: number;
		onblur?: () => void;
		id?: string;
	} = $props();

	const uid = $props.id();
	const fid = $derived(id ?? uid);
	const errorsId = $derived(`${fid}-errors`);
	let el = $state<HTMLTextAreaElement | null>(null);
	let gutter = $state<HTMLElement | null>(null);
	$effect(() => {
		ref = el;
	});

	const numbers = $derived(Array.from({ length: value.split('\n').length }, (_, i) => i + 1));
	const failed = $derived(new Set(errors.map((e) => e.line)));

	function sync() {
		if (gutter && el) gutter.scrollTop = el.scrollTop;
	}
</script>

<Field {label} {hint} id={fid}>
	{#snippet children(f)}
		<div class="editor" class:invalid={errors.length > 0} style:--rows={rows}>
			<div class="gutter" aria-hidden="true" bind:this={gutter}>
				{#each numbers as n (n)}
					<span data-error={failed.has(n) ? '' : undefined}>{n}</span>
				{/each}
			</div>
			<textarea
				id={f.id}
				bind:this={el}
				bind:value
				{rows}
				{maxlength}
				spellcheck="false"
				autocapitalize="off"
				autocomplete="off"
				wrap="off"
				aria-describedby={[f.describedBy, errors.length > 0 ? errorsId : null]
					.filter(Boolean)
					.join(' ') || undefined}
				aria-invalid={errors.length > 0 ? 'true' : undefined}
				onscroll={sync}
				{onblur}></textarea>
		</div>
		{#if errors.length > 0}
			<ul class="errors" id={errorsId}>
				{#each errors as e, i (i)}
					<li>{ru.ui.codeEditor.error(e.line, e.column, e.message)}</li>
				{/each}
			</ul>
		{/if}
	{/snippet}
</Field>

<style>
	.editor {
		display: flex;
		box-sizing: border-box;
		width: 100%;
		height: calc(var(--rows) * var(--text-sm) * var(--leading-normal) + 2 * var(--space-2) + 2px);
		border: 1px solid var(--border-strong);
		border-radius: var(--radius-md);
		background: var(--surface);
		color: var(--text);
		font-family: var(--font-mono);
		font-size: var(--text-sm);
		line-height: var(--leading-normal);
		resize: vertical;
		overflow: hidden;
	}
	.editor:focus-within {
		outline: 2px solid var(--focus);
		outline-offset: 1px;
		box-shadow: var(--shadow-sm);
	}
	.editor.invalid {
		border-color: var(--danger);
	}
	.gutter,
	textarea {
		margin: 0;
		padding: var(--space-2) 0;
		font: inherit;
		line-height: inherit;
	}
	.gutter {
		flex: none;
		min-width: 3ch;
		padding-inline: var(--space-2);
		box-sizing: content-box;
		overflow: hidden;
		background: var(--surface-2);
		color: var(--text-3);
		text-align: right;
		user-select: none;
	}
	.gutter span {
		display: block;
	}
	.gutter span[data-error] {
		background: var(--danger-soft);
		color: var(--danger-text);
		font-weight: 600;
	}
	textarea {
		flex: 1;
		min-width: 0;
		padding-inline: var(--space-3);
		border: 0;
		outline: none;
		background: transparent;
		color: inherit;
		white-space: pre;
		overflow: auto;
		resize: none;
		tab-size: 2;
	}
	.errors {
		margin: 0;
		padding-left: var(--space-4);
		font-size: var(--text-sm);
		color: var(--danger-text);
	}
</style>
