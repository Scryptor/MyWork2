<!-- «Документы и реквизиты» (spec 6.8, plan 6): the firm's phone, details and legal details and the terms
     of the acts, as they are printed; the server's settings, saved as they are typed. Below, every document
     with its «Образец»: the preview on a sample order and these settings; with the right to change the
     settings, «Изменить» opens the editor of its template (all but the calibration label). -->
<script lang="ts">
	import { onDestroy } from 'svelte';
	import { resolve } from '$app/paths';
	import type { DocumentKind } from '$api/DocumentKind';
	import { updateSettings } from '#lib/api/catalogs.js';
	import { app } from '#lib/app/context.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import { documentName, openPreview } from '#lib/print/print.svelte.js';
	import Button from '#lib/ui/Button.svelte';
	import SaveNote from '#lib/settings/SaveNote.svelte';
	import {
		createSettingsEditor,
		type SettingsField
	} from '#lib/settings/settings-editor.svelte.js';
	import FirmLogo from '#lib/settings/FirmLogo.svelte';
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

	/** Every document, in the order of the work: the acts, the blank one, the labels. */
	const DOCUMENTS: DocumentKind[] = [
		'intake_act',
		'issue_act',
		'warranty_intake_act',
		'warranty_issue_act',
		'blank_intake_act',
		'sticker',
		'calibration'
	];

	/** The sample is drawn with what is typed: the save of it (in its pause or on its way) comes first. */
	async function sample(kind: DocumentKind) {
		await editor.flush();
		openPreview(kind, {});
	}

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

<Card>
	<FirmLogo canEdit={!locked} />
</Card>

<Card>
	<div class="fields">
		<h2 class="subtitle">{ru.print.documentsList}</h2>
		<p class="note">{ru.print.documentsNote}</p>
		<ul class="documents" aria-label={ru.print.documentsList}>
			{#each DOCUMENTS as kind (kind)}
				<li>
					<span>{documentName(kind)}</span>
					<span class="buttons">
						<Button
							size="sm"
							aria-label={ru.print.sampleOf(documentName(kind))}
							onclick={() => void sample(kind)}>{ru.print.sampleButton}</Button
						>
						{#if !locked && kind !== 'calibration'}
							<Button
								size="sm"
								href={resolve(`templates/${kind}`)}
								aria-label={ru.templates.editOf(documentName(kind))}
								>{ru.templates.editButton}</Button
							>
						{/if}
					</span>
				</li>
			{/each}
		</ul>
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
	.subtitle {
		margin: 0;
		font-size: var(--text-lg);
		font-weight: 600;
	}
	.documents {
		display: flex;
		flex-direction: column;
		margin: 0;
		padding: 0;
		list-style: none;
	}
	.documents li {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-3);
		padding: var(--space-2) 0;
		border-top: 1px solid var(--border);
	}
	.documents li:first-child {
		border-top: 0;
	}
	.buttons {
		display: flex;
		flex-wrap: wrap;
		justify-content: flex-end;
		gap: var(--space-2);
	}
</style>
