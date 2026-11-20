<!-- «Начать с пустой базы» (spec 6.9): the name of the service, the first point, the masters as chips. One
     request sets them all; a base another window set up meanwhile (409) just goes on to the program. -->
<script lang="ts">
	import { ApiError, errorMessage } from '#lib/api/client.js';
	import { runSetup } from '#lib/api/setup.js';
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import TagInput from '#lib/ui/TagInput.svelte';
	import TextInput from '#lib/ui/TextInput.svelte';

	let {
		ondone,
		onalready,
		oncancel
	}: {
		ondone: () => void;
		/** The program is set up already (another window did it): the server's words. */
		onalready: (message: string) => void;
		oncancel?: () => void;
	} = $props();

	const t = ru.setup;
	let firm = $state('');
	let point = $state('');
	let masters = $state<string[]>([]);
	let firmError = $state<string | undefined>();
	let pointError = $state<string | undefined>();
	let failure = $state<string | null>(null);
	let busy = $state(false);

	function fieldErrors(e: unknown): boolean {
		const fields = e instanceof ApiError ? (e.body?.fields ?? []) : [];
		for (const f of fields) {
			const text = f.code === 'required' ? t.required : f.message;
			if (f.field === 'firm_name') firmError = text;
			else if (f.field === 'point_name') pointError = text;
		}
		return fields.length > 0;
	}

	async function submit(e: SubmitEvent) {
		e.preventDefault();
		if (busy) return;
		failure = null;
		firmError = firm.trim() ? undefined : t.required;
		pointError = point.trim() ? undefined : t.required;
		if (firmError || pointError) return;
		busy = true;
		try {
			await runSetup({ firm_name: firm.trim(), point_name: point.trim(), masters });
			ondone();
		} catch (err) {
			if (err instanceof ApiError && err.status === 409) onalready(err.message);
			else if (!fieldErrors(err)) failure = errorMessage(err);
		} finally {
			busy = false;
		}
	}
</script>

<form class="form" novalidate onsubmit={submit}>
	<TextInput
		label={t.firmName}
		hint={t.firmNameHint}
		bind:value={firm}
		error={firmError}
		required
		autofocus
	/>
	<TextInput
		label={t.point}
		hint={t.pointHint}
		help={ru.help.setup.point}
		bind:value={point}
		error={pointError}
		required
	/>
	<TagInput label={t.masters} hint={t.mastersHint} bind:values={masters} />
	{#if failure}<Callout tone="danger">{failure}</Callout>{/if}
	<div class="actions">
		{#if oncancel}<Button onclick={oncancel} disabled={busy}>{t.back}</Button>{/if}
		<Button type="submit" variant="primary" {busy}>{t.submit}</Button>
	</div>
</form>

<style>
	.form {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
	}
	.actions {
		display: flex;
		flex-wrap: wrap;
		justify-content: flex-end;
		gap: var(--space-2);
	}
</style>
