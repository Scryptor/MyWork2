<!-- «Вставить поле» under a text field of a template: opens a search over the fields an order offers
     (`documentFields(kind)`), and puts `{field}` into the text where the caret was (own Typst code puts what
     its `format` makes of the field and the text before the caret); the focus goes back to the field
     with the caret after what was put in. -->
<script lang="ts">
	import Braces from '@lucide/svelte/icons/braces';
	import { tick } from 'svelte';
	import type { DocumentField } from '$api/DocumentField';
	import type { DocumentKind } from '$api/DocumentKind';
	import { documentFields } from '#lib/api/document-templates.js';
	import { errorMessage } from '#lib/api/client.js';
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';
	import Combobox from '#lib/ui/Combobox.svelte';
	import InsertChip from '#lib/ui/InsertChip.svelte';
	import { toast } from '#lib/ui/toast/toast.svelte.js';
	import { insertIntoField } from './insert.js';

	let {
		kind,
		target,
		value,
		onchange,
		name,
		format = (field) => `{${field.placeholder}}`,
		trigger = 'button'
	}: {
		/** The document the template is of: the menu offers the fields of that document. */
		kind: DocumentKind;
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
		/** A button under the field, or a badge «Другое поле…» in a row of badges. */
		trigger?: 'button' | 'chip';
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
			documentFields(kind).then(
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
		insertIntoField(target, value, (before) => format(field, before), onchange);
		open = false;
		query = '';
	}
</script>

<div class="insert" class:chip={trigger === 'chip'}>
	{#if trigger === 'chip'}
		<InsertChip
			label={t.badgeOtherField}
			code={t.badgeOtherCode}
			aria-expanded={open}
			onclick={toggle}
		/>
	{:else}
		<Button
			size="sm"
			variant="ghost"
			icon={Braces}
			aria-label={name ? t.insertFieldTo(name) : undefined}
			aria-expanded={open}
			onclick={toggle}>{t.insertField}</Button
		>
	{/if}
	{#if open}
		<div class="search">
			<Combobox
				id={uid}
				label={t.insertFieldSearch}
				{items}
				allowCustom={false}
				bind:value={query}
				onselect={pick}
			/>
		</div>
	{/if}
</div>

<style>
	.insert {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: var(--space-2);
	}
	.search {
		align-self: stretch;
	}
	/* As a badge the trigger sits in the row of badges; the search takes a row of its own below it. */
	.chip {
		display: contents;
	}
	.chip .search {
		flex: 1 0 100%;
	}
</style>
