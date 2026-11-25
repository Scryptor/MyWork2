<!-- «Справка по Typst-коду»: short sections with examples from `typst-snippets.json` (the same file is
     rendered by a test of `mw-docs`, so every example is real working code). -->
<script lang="ts">
	import { ru } from '#lib/i18n/ru.js';
	import Dialog from '#lib/ui/Dialog.svelte';
	import snippets from './typst-snippets.json';

	let { open = $bindable(false) }: { open: boolean } = $props();

	const t = ru.templates.typstHelp;
	const code = (id: string) => snippets.examples.find((e) => e.id === id)?.code ?? '';
</script>

<Dialog bind:open title={t.title} description={t.description} size="lg">
	<div class="help">
		{#each t.sections as section (section.title)}
			<section>
				<h3>{section.title}</h3>
				{#if section.text}<p>{section.text}</p>{/if}
				{#if section.items}
					<ul>
						{#each section.items as item (item)}<li>{item}</li>{/each}
					</ul>
				{/if}
				{#if section.example}<pre>{code(section.example)}</pre>{/if}
			</section>
		{/each}
	</div>
</Dialog>

<style>
	.help {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
		max-height: 60vh;
		overflow: auto;
	}
	h3 {
		margin: 0 0 var(--space-1);
		font-size: inherit;
		font-weight: 600;
	}
	p,
	ul {
		margin: 0 0 var(--space-2);
	}
	ul {
		padding-left: var(--space-5);
	}
	pre {
		margin: 0;
		padding: var(--space-3);
		overflow-x: auto;
		white-space: pre-wrap;
		overflow-wrap: anywhere;
		font-family: var(--font-mono);
		background: var(--bg);
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
	}
</style>
