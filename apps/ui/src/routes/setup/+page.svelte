<!-- The first run (spec 6.9): no shell, a card in the middle. «Начать с пустой базы» — a short form, then
     the orders; «Перенести из MyWork 2» — the wizard, then the report. An import under way (the page was
     reloaded, another tab started it) shows its progress at once. -->
<script lang="ts">
	import FilePlus from '@lucide/svelte/icons/file-plus';
	import FolderInput from '@lucide/svelte/icons/folder-input';
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import type { ImportJob } from '$api/ImportJob';
	import { currentLegacyImport } from '#lib/api/import.js';
	import { ru } from '#lib/i18n/ru.js';
	import ImportWizard from '#lib/setup/ImportWizard.svelte';
	import { jobState } from '#lib/setup/progress.js';
	import SetupForm from '#lib/setup/SetupForm.svelte';
	import HelpHint from '#lib/ui/HelpHint.svelte';
	import OptionCard from '#lib/ui/OptionCard.svelte';
	import { toast } from '#lib/ui/toast/toast.svelte.js';

	const t = ru.setup;
	let step = $state<'choose' | 'empty' | 'import'>('choose');
	let resume = $state.raw<ImportJob | null>(null);

	onMount(() => {
		let gone = false;
		currentLegacyImport().then(
			(job) => {
				if (gone || job === null || jobState(job).kind !== 'running') return;
				resume = job;
				step = 'import';
			},
			// Nothing under way that this page may see: the choice stays.
			() => {}
		);
		return () => {
			gone = true;
		};
	});

	const title = $derived(
		step === 'empty' ? t.empty : step === 'import' ? ru.legacy.title : t.title
	);
</script>

<svelte:head><title>{title}</title></svelte:head>

<main class="screen" tabindex="-1">
	<div class="card">
		<h1>{title}</h1>
		{#if step === 'choose'}
			<div class="intro">
				<p>{t.intro}</p>
				<HelpHint title={t.choiceHelp} text={ru.help.setup.choice} />
			</div>
			<div class="options">
				<OptionCard
					icon={FilePlus}
					title={t.empty}
					description={t.emptyHint}
					onclick={() => (step = 'empty')}
				/>
				<OptionCard
					icon={FolderInput}
					title={t.import}
					description={t.importHint}
					onclick={() => (step = 'import')}
				/>
			</div>
		{:else if step === 'empty'}
			<SetupForm
				ondone={() => void goto(resolve('orders'))}
				onalready={(message) => {
					toast.show({ message });
					void goto(resolve('orders'));
				}}
				oncancel={() => (step = 'choose')}
			/>
		{:else}
			<ImportWizard {resume} onback={resume ? undefined : () => (step = 'choose')} />
		{/if}
	</div>
</main>

<style>
	/* The window itself never scrolls: a card taller than it scrolls inside. */
	.screen {
		box-sizing: border-box;
		display: flex;
		height: 100vh;
		height: 100dvh;
		overflow-y: auto;
		padding: var(--space-4);
		background: var(--bg);
		color: var(--text);
		outline: none;
	}
	.card {
		box-sizing: border-box;
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
		width: min(34rem, 100%);
		margin: auto;
		padding: var(--space-5);
		border: 1px solid var(--border);
		border-radius: var(--radius-lg);
		background: var(--surface);
		box-shadow: var(--shadow-sm);
	}
	h1 {
		margin: 0;
		font-size: var(--text-xl);
		font-weight: 600;
	}
	.intro {
		display: flex;
		align-items: center;
		gap: var(--space-1);
		color: var(--text-2);
	}
	.intro p {
		margin: 0;
	}
	.options {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}
</style>
