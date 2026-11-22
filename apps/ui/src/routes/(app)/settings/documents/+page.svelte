<!-- «Документы и реквизиты» (spec 6.8, plan 6): the firm's phone, details and legal details and the terms
     of the acts, as they are printed; the server's settings, saved as they are typed. -->
<script lang="ts">
	import { onDestroy } from 'svelte';
	import { updateSettings } from '#lib/api/catalogs.js';
	import { app } from '#lib/app/context.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import SaveNote from '#lib/settings/SaveNote.svelte';
	import {
		createSettingsEditor,
		type SettingsField
	} from '#lib/settings/settings-editor.svelte.js';
	import Callout from '#lib/ui/Callout.svelte';
	import Card from '#lib/ui/Card.svelte';
	import TextInput from '#lib/ui/TextInput.svelte';
	import Textarea from '#lib/ui/Textarea.svelte';

	const t = ru.settings;
	const help = ru.help.documents;
	const editor = createSettingsEditor({
		can: () => app.can('settings_manage'),
		save: updateSettings,
		refresh: () => app.refreshCatalogs()
	});
	const s = $derived(app.settings);
	const locked = $derived(!editor.canEdit);

	type TextField = Extract<
		SettingsField,
		'firm_details' | 'firm_legal_details' | 'intake_terms' | 'issue_terms'
	>;
	/** The texts of several lines, in the order of the act: the head, the terms, the foot. */
	const TEXTS: { field: TextField; label: string; help: readonly string[]; rows: number }[] = [
		{ field: 'firm_details', label: t.firmDetails, help: help.firmDetails, rows: 3 },
		{ field: 'intake_terms', label: t.intakeTerms, help: help.intakeTerms, rows: 5 },
		{ field: 'issue_terms', label: t.issueTerms, help: help.issueTerms, rows: 4 },
		{ field: 'firm_legal_details', label: t.firmLegal, help: help.firmLegal, rows: 3 }
	];

	onDestroy(() => {
		void editor.flush().finally(() => editor.dispose());
	});
</script>

<h1 class="title">{t.documents}</h1>

{#if locked}<Callout tone="info">{t.readOnly}</Callout>{/if}

<Card>
	<div class="fields">
		<p class="note">{t.documentsNote}</p>
		<TextInput
			label={t.firmPhone}
			help={help.firmPhone}
			disabled={locked}
			error={editor.fieldError('firm_phone')}
			bind:value={
				() => editor.value('firm_phone', s.firm_phone) ?? '', (v) => editor.edit('firm_phone', v)
			}
			onblur={() => void editor.flush()}
		/>
		{#each TEXTS as f (f.field)}
			<Textarea
				label={f.label}
				help={f.help}
				minRows={f.rows}
				maxRows={16}
				disabled={locked}
				error={editor.fieldError(f.field)}
				bind:value={() => editor.value(f.field, s[f.field]) ?? '', (v) => editor.edit(f.field, v)}
				onblur={() => void editor.flush()}
			/>
		{/each}
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
	.note {
		margin: 0;
		color: var(--text-3);
		font-size: var(--text-sm);
	}
</style>
