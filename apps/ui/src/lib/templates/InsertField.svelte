<!-- «Вставить поле» under a text field of a template: opens a search over the fields an order offers
     (`documentFields()`), and puts `{field}` into the text where the caret was (own Typst code puts what
     its `format` makes of the field and the text before the caret); the focus goes back to the field
     with the caret after what was put in. -->
<script lang="ts">
	import Braces from '@lucide/svelte/icons/braces';
	import { tick } from 'svelte';
	import type { DocumentField } from '$api/DocumentField';
	import { documentFields } from '#lib/api/document-templates.js';
	import { errorMessage } from '#lib/api/client.js';
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';
	import Combobox from '#lib/ui/Combobox.svelte';
	import { toast } from '#lib/ui/toast/toast.svelte.js';
	import { insertAtCaret } from './insert.js';

	let {
		target,
		value,
		onchange,
		name,
		format = (field) => `{${field.placeholder}}`
	}: {
		/** The field the text is typed into; its caret is where the placeholder goes. */
		target: HTMLInputElement | HTMLTextAreaElement | null;
		value: string;
		/** The text with the placeholder in it. */
		onchange: (value: string) => void;
		/** Which of several fields of a block this is, for the name of the button. */
		name?: string;
		/**
		 * What goes into the text for the field, put after `before` (the text before the caret); the
		 * placeholder `{заказ.номер}` of a text by default.
		 */
		format?: (field: DocumentField, before: string) => string;
	} = $props();

	const t = ru.templates;
	const uid = $props.id();
	let open = $state(false);
	let query = $state('');
	let fields = $state.raw<DocumentField[]>([]);
	let loaded = false;

	const items = $derived(fields.map((f) => ({ value: f.label })));

	async function toggle() {
		open = !open;
		if (!open) return;
		if (!loaded) {
			loaded = true;
			documentFields().then(
				(list) => (fields = list),
				(e) => {
					loaded = false;
					toast.show({ message: errorMessage(e), tone: 'danger' });
				}
			);
		}
		await tick();
		document.getElementById(uid)?.focus();
	}

	function pick(label: string) {
		const field = fields.find((f) => f.label === label);
		if (!field) return;
		const at = target
			? { start: target.selectionStart ?? value.length, end: target.selectionEnd ?? value.length }
			: { start: value.length, end: value.length };
		const next = insertAtCaret(value, at, format(field, value.slice(0, at.start)));
		onchange(next.value);
		open = false;
		query = '';
		// After the field shows the new text: the caret is set on the text that is there.
		void tick().then(() => {
			target?.focus();
			target?.setSelectionRange(next.caret, next.caret);
		});
	}
</script>

<div class="insert">
	<Button
		size="sm"
		variant="ghost"
		icon={Braces}
		aria-label={name ? t.insertFieldTo(name) : undefined}
		aria-expanded={open}
		onclick={toggle}>{t.insertField}</Button
	>
	{#if open}
		<Combobox
			id={uid}
			label={t.insertFieldSearch}
			{items}
			allowCustom={false}
			bind:value={query}
			onselect={pick}
		/>
	{/if}
</div>

<style>
	.insert {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: var(--space-2);
	}
	.insert > :global(:not(button)) {
		align-self: stretch;
	}
</style>
