<!-- «Общие» (spec 6.8): the name, the currency, the time zone, the next order number, the master's share. -->
<script lang="ts">
	import { onDestroy } from 'svelte';
	import { getNextOrderNumber, setNextOrderNumber, updateSettings } from '#lib/api/catalogs.js';
	import { app } from '#lib/app/context.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import SaveNote from '#lib/settings/SaveNote.svelte';
	import { createNextOrder } from '#lib/settings/next-order.svelte.js';
	import { createSettingsEditor } from '#lib/settings/settings-editor.svelte.js';
	import { canListTimezones, timezoneOptions } from '#lib/settings/timezones.js';
	import Callout from '#lib/ui/Callout.svelte';
	import Card from '#lib/ui/Card.svelte';
	import Combobox from '#lib/ui/Combobox.svelte';
	import Select from '#lib/ui/Select.svelte';
	import TextInput from '#lib/ui/TextInput.svelte';

	const t = ru.settings;
	const editor = createSettingsEditor({
		can: () => app.can('settings_manage'),
		save: updateSettings,
		refresh: () => app.refreshCatalogs()
	});
	const s = $derived(app.settings);
	const locked = $derived(!editor.canEdit);

	// The offsets of the list are those of the moment the screen opens (summer time moves some of them).
	const now = new Date();
	const zone = $derived(editor.value('timezone', s.timezone) ?? s.timezone);
	/** An engine that cannot list the zones gets the field of before: the name is typed, the server checks it. */
	const listed = canListTimezones();
	const zones = $derived(listed ? timezoneOptions(now, zone) : []);

	// «Следующий номер заказа» is not a setting but a counter: it has its own requests.
	const nextOrder = createNextOrder({
		can: () => app.can('settings_manage'),
		get: getNextOrderNumber,
		set: setNextOrderNumber
	});
	// The right may come after the screen is open: the number is read when it does.
	$effect(() => {
		if (editor.canEdit) void nextOrder.load();
	});

	onDestroy(() => {
		void editor.flush().finally(() => editor.dispose());
	});
</script>

<h1 class="title">{t.general}</h1>

{#if locked}<Callout tone="info">{t.readOnly}</Callout>{/if}

<Card>
	<div class="fields">
		<TextInput
			label={t.firmName}
			help={ru.help.general.firmName}
			disabled={locked}
			error={editor.fieldError('firm_name')}
			bind:value={
				() => editor.value('firm_name', s.firm_name) ?? '', (v) => editor.edit('firm_name', v)
			}
			onblur={() => void editor.flush()}
		/>
		<TextInput
			label={t.currency}
			help={ru.help.general.currency}
			disabled={locked}
			maxlength={8}
			error={editor.fieldError('currency_symbol')}
			bind:value={
				() => editor.value('currency_symbol', s.currency_symbol) ?? '',
				(v) => editor.edit('currency_symbol', v)
			}
			onblur={() => void editor.flush()}
		/>
		{#if listed}
			<Select
				label={t.timezone}
				help={ru.help.general.timezone}
				items={zones}
				disabled={locked}
				error={editor.fieldError('timezone')}
				bind:value={() => zone || null, (v) => v && v !== zone && editor.commit('timezone', v)}
			/>
		{:else}
			<Combobox
				label={t.timezone}
				help={ru.help.general.timezone}
				items={zone ? [{ value: zone }] : []}
				allowCustom
				disabled={locked}
				error={editor.fieldError('timezone')}
				bind:value={() => zone, (v) => editor.type('timezone', v)}
				onselect={(v) => editor.commit('timezone', v)}
				onenter={() => editor.commitTyped('timezone')}
				onblur={() => editor.commitTyped('timezone')}
			/>
		{/if}
		<!-- The counter is read only with «settings_manage»: without it there is nothing to show. -->
		{#if !locked}
			<form
				onsubmit={(e) => {
					e.preventDefault();
					void nextOrder.commit();
				}}
			>
				<TextInput
					label={t.nextOrder}
					hint={t.nextOrderHint}
					help={ru.help.general.nextOrder}
					inputmode="numeric"
					error={nextOrder.error}
					bind:value={() => nextOrder.text, (v) => (nextOrder.text = v)}
					onblur={() => void nextOrder.commit()}
				/>
			</form>
		{/if}
		<TextInput
			label={t.masterPercent}
			help={ru.help.general.masterPercent}
			inputmode="numeric"
			disabled={locked}
			error={editor.fieldError('master_percent')}
			bind:value={
				() => editor.draft('master_percent', s.master_percent),
				(v) => editor.editNumber('master_percent', v)
			}
			onblur={() => {
				void editor.flush();
				editor.leave('master_percent');
			}}
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
