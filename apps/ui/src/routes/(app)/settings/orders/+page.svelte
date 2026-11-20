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
	import Checkbox from '#lib/ui/Checkbox.svelte';
	import Combobox from '#lib/ui/Combobox.svelte';
	import HelpHint from '#lib/ui/HelpHint.svelte';
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

	const ISSUE_FIELDS = ['final_price', 'work_done', 'master', 'warranty'] as const;
	type IssueField = (typeof ISSUE_FIELDS)[number];
	const required = $derived<IssueField[]>(
		editor.value('issue_required', s.issue_required) ?? s.issue_required
	);
	function toggle(f: IssueField, on: boolean) {
		const next = ISSUE_FIELDS.filter((x) => (x === f ? on : required.includes(x)));
		editor.commit('issue_required', next);
	}

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
			help={ru.help.orders.defaultPoint}
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
			help={ru.help.orders.defaultMaster}
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
			help={ru.help.orders.defaultWarranty}
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
			help={ru.help.orders.diagnosticsDays}
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
			help={ru.help.orders.readyDays}
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
			help={ru.help.orders.requiredFields}
			disabled={locked}
			bind:checked={
				() => editor.value('intake_required_fields', s.intake_required_fields) ?? false,
				(v) => editor.commit('intake_required_fields', v)
			}
		/>
		<fieldset class="group">
			<legend>
				<span class="legend">
					{t.issueRequired}
					<HelpHint title={t.issueRequired} text={ru.help.orders.issueRequired} />
				</span>
			</legend>
			<p class="hint">{t.issueRequiredHint}</p>
			{#each ISSUE_FIELDS as f (f)}
				<Checkbox
					label={t.issueFields[f]!}
					disabled={locked}
					bind:checked={() => required.includes(f), (v) => toggle(f, v)}
				/>
			{/each}
		</fieldset>
		<SaveNote {editor} />
	</div>
</Card>

<style>
	.title {
		margin: 0;
		font-size: var(--text-xl);
		font-weight: 600;
	}
	.group {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		margin: 0;
		padding: 0;
		border: 0;
	}
	.group legend {
		padding: 0;
		font-weight: 600;
	}
	.legend {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
	}
	.hint {
		margin: 0;
		color: var(--text-3);
		font-size: var(--text-sm);
	}
	.fields {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
	}
</style>
