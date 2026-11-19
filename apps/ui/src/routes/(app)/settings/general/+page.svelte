<!-- «Общие» (spec 6.8): the name, the currency, the time zone, the next order number, the master's share. -->
<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { ApiError, errorMessage } from '#lib/api/client.js';
	import { getNextOrderNumber, setNextOrderNumber, updateSettings } from '#lib/api/catalogs.js';
	import { app } from '#lib/app/context.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import SaveNote from '#lib/settings/SaveNote.svelte';
	import { createSettingsEditor } from '#lib/settings/settings-editor.svelte.js';
	import Callout from '#lib/ui/Callout.svelte';
	import Card from '#lib/ui/Card.svelte';
	import Combobox from '#lib/ui/Combobox.svelte';
	import TextInput from '#lib/ui/TextInput.svelte';

	const t = ru.settings;
	const editor = createSettingsEditor({
		can: () => app.can('settings_manage'),
		save: updateSettings,
		refresh: () => app.refreshCatalogs()
	});
	const s = $derived(app.settings);
	const locked = $derived(!editor.canEdit);

	const zones = $derived.by(() => {
		const all = Intl.supportedValuesOf('timeZone');
		const current = editor.value('timezone', s.timezone) ?? s.timezone;
		return (all.includes(current) ? all : [current, ...all]).map((value) => ({ value }));
	});

	// «Следующий номер заказа» is not a setting but a counter: it has its own request.
	let next = $state('');
	let nextServer: number | null = null;
	let nextError = $state<string | undefined>();
	let nextBusy = false;

	onMount(async () => {
		if (!app.can('settings_manage')) return;
		try {
			nextServer = (await getNextOrderNumber()).next;
			next = String(nextServer);
		} catch (e) {
			nextError = errorMessage(e);
		}
	});

	async function commitNext() {
		const raw = next.trim();
		if (!editor.canEdit || nextServer === null || nextBusy || raw === String(nextServer)) return;
		if (!/^\d+$/.test(raw) || Number(raw) < 1) {
			nextError = t.nextOrderNotNumber;
			next = String(nextServer);
			return;
		}
		nextBusy = true;
		try {
			await setNextOrderNumber(Number(raw));
			nextServer = Number(raw);
			next = String(nextServer);
			nextError = undefined;
		} catch (e) {
			nextError =
				e instanceof ApiError && e.code === 'validation' ? t.nextOrderTooSmall : errorMessage(e);
			next = String(nextServer);
		} finally {
			nextBusy = false;
		}
	}

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
			disabled={locked}
			error={editor.fieldError('firm_name')}
			bind:value={
				() => editor.value('firm_name', s.firm_name) ?? '', (v) => editor.edit('firm_name', v)
			}
			onblur={() => void editor.flush()}
		/>
		<TextInput
			label={t.currency}
			disabled={locked}
			maxlength={8}
			error={editor.fieldError('currency_symbol')}
			bind:value={
				() => editor.value('currency_symbol', s.currency_symbol) ?? '',
				(v) => editor.edit('currency_symbol', v)
			}
			onblur={() => void editor.flush()}
		/>
		<Combobox
			label={t.timezone}
			items={zones}
			allowCustom={false}
			disabled={locked}
			error={editor.fieldError('timezone')}
			bind:value={() => editor.value('timezone', s.timezone) ?? '', () => {}}
			onselect={(v) => editor.commit('timezone', v)}
		/>
		<form
			onsubmit={(e) => {
				e.preventDefault();
				void commitNext();
			}}
		>
			<TextInput
				label={t.nextOrder}
				hint={t.nextOrderHint}
				inputmode="numeric"
				disabled={locked}
				error={nextError}
				bind:value={() => next, (v) => ((next = v), (nextError = undefined))}
				onblur={() => void commitNext()}
			/>
		</form>
		<TextInput
			label={t.masterPercent}
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
