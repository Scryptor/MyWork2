<!-- One row of a «Поля» block: its title, its value (with «Вставить поле»), «Скрывать, если пусто», «Жирная»
     and «Удалить строку». Typed text makes one step to undo per pause. -->
<script lang="ts">
	import Trash2 from '@lucide/svelte/icons/trash-2';
	import type { DocumentKind } from '$api/DocumentKind';
	import type { FieldRow } from '$api/FieldRow';
	import { ru } from '#lib/i18n/ru.js';
	import IconButton from '#lib/ui/IconButton.svelte';
	import Switch from '#lib/ui/Switch.svelte';
	import Textarea from '#lib/ui/Textarea.svelte';
	import TextInput from '#lib/ui/TextInput.svelte';
	import InsertField from '../InsertField.svelte';

	let {
		kind,
		row,
		number,
		onchange,
		onremove
	}: {
		/** The document the template is of. */
		kind: DocumentKind;
		row: FieldRow;
		/** The place of the row in the list, from 1. */
		number: number;
		onchange: (patch: Partial<FieldRow>, field?: keyof FieldRow) => void;
		onremove: () => void;
	} = $props();

	const t = ru.templates;
	let valueField = $state<HTMLTextAreaElement | null>(null);
</script>

<div class="row" role="group" aria-label={t.fieldRow(number)}>
	<TextInput
		label={t.rowLabel}
		maxlength={200}
		bind:value={() => row.label, (v) => onchange({ label: v }, 'label')}
	/>
	<Textarea
		label={t.rowValue}
		minRows={1}
		maxlength={2000}
		bind:ref={valueField}
		bind:value={() => row.value, (v) => onchange({ value: v }, 'value')}
	/>
	<InsertField
		{kind}
		name={row.label || t.fieldRow(number)}
		target={valueField}
		value={row.value}
		onchange={(v) => onchange({ value: v }, 'value')}
	/>
	<Switch
		label={t.hideIfEmpty}
		bind:checked={() => row.hide_if_empty, (v) => onchange({ hide_if_empty: v })}
	/>
	<Switch label={t.rowBold} bind:checked={() => row.bold, (v) => onchange({ bold: v })} />
	<div>
		<IconButton label={t.removeRow} icon={Trash2} variant="ghost" onclick={onremove} />
	</div>
</div>

<style>
	.row {
		display: flex;
		flex: 1;
		flex-direction: column;
		gap: var(--space-2);
		min-width: 0;
		padding-block: var(--space-2);
	}
</style>
