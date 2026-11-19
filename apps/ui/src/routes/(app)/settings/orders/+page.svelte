<!-- «Заказы» (spec 6.8): what a new order starts with and when an order needs attention. -->
<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { listDictionary, updateSettings } from '#lib/api/catalogs.js';
	import { app } from '#lib/app/context.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import SaveNote from '#lib/settings/SaveNote.svelte';
	import { createSettingsEditor } from '#lib/settings/settings-editor.svelte.js';
	import Callout from '#lib/ui/Callout.svelte';
	import Card from '#lib/ui/Card.svelte';
	import Combobox from '#lib/ui/Combobox.svelte';
	import Select from '#lib/ui/Select.svelte';
	import Switch from '#lib/ui/Switch.svelte';
	import TextInput from '#lib/ui/TextInput.svelte';

	const t = ru.settings;
	const editor = createSettingsEditor({
		can: () => app.can('settings_manage'),
		save: updateSettings,
		refresh: () => app.refreshCatalogs()
	});
	const s = $derived(app.settings);
	const locked = $derived(!editor.canEdit);

	const NONE = 0;
	const points = $derived([
		{ value: NONE, label: t.noPoint },
		...app.activePoints.map((p) => ({ value: p.id, label: p.name }))
	]);

	let masters = $state<{ value: string }[]>([]);
	let warranties = $state<{ value: string }[]>([]);
	onMount(async () => {
		try {
			const [m, w] = await Promise.all([listDictionary('master'), listDictionary('warranty')]);
			masters = m.filter((i) => !i.archived).map((i) => ({ value: i.value }));
			warranties = w.filter((i) => !i.archived).map((i) => ({ value: i.value }));
		} catch {
			// Without suggestions the fields are still free text.
		}
	});

	onDestroy(() => {
		void editor.flush().finally(() => editor.dispose());
	});
</script>

<h1 class="title">{t.ordersTitle}</h1>

{#if locked}<Callout tone="info">{t.readOnly}</Callout>{/if}

<Card>
	<div class="fields">
		<Select
			label={t.defaultPoint}
			items={points}
			disabled={locked}
			error={editor.fieldError('default_service_point_id')}
			bind:value={
				() => editor.value('default_service_point_id', s.default_service_point_id) ?? NONE,
				(v) => editor.commit('default_service_point_id', v === null || v === NONE ? null : v)
			}
		/>
		<Combobox
			label={t.defaultMaster}
			items={masters}
			disabled={locked}
			error={editor.fieldError('default_master')}
			bind:value={
				() => editor.value('default_master', s.default_master) ?? '',
				(v) => editor.edit('default_master', v)
			}
			onselect={(v) => editor.commit('default_master', v)}
		/>
		<Combobox
			label={t.defaultWarranty}
			items={warranties}
			disabled={locked}
			error={editor.fieldError('default_warranty')}
			bind:value={
				() => editor.value('default_warranty', s.default_warranty) ?? '',
				(v) => editor.edit('default_warranty', v)
			}
			onselect={(v) => editor.commit('default_warranty', v)}
		/>
		<TextInput
			label={t.diagnosticsDays}
			inputmode="numeric"
			disabled={locked}
			error={editor.fieldError('diagnostics_overdue_days')}
			bind:value={
				() => editor.draft('diagnostics_overdue_days', s.diagnostics_overdue_days),
				(v) => editor.editNumber('diagnostics_overdue_days', v)
			}
			onblur={() => {
				void editor.flush();
				editor.leave('diagnostics_overdue_days');
			}}
		/>
		<TextInput
			label={t.readyDays}
			inputmode="numeric"
			disabled={locked}
			error={editor.fieldError('ready_stale_days')}
			bind:value={
				() => editor.draft('ready_stale_days', s.ready_stale_days),
				(v) => editor.editNumber('ready_stale_days', v)
			}
			onblur={() => {
				void editor.flush();
				editor.leave('ready_stale_days');
			}}
		/>
		<Switch
			label={t.requiredFields}
			disabled={locked}
			bind:checked={
				() => editor.value('intake_required_fields', s.intake_required_fields) ?? false,
				(v) => editor.commit('intake_required_fields', v)
			}
		/>
		<SaveNote {editor} />
	</div>
</Card>

<style>
	.title {
		margin: 0;
		font-size: var(--text-xl);
		font-weight: 600;
	}
	.fields {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
	}
</style>
