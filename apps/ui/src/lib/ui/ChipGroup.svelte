<script lang="ts">
	import { normalizeSearch } from '#lib/format/search.js';
	import { ru } from '#lib/i18n/ru.js';

	let {
		selected = $bindable([]),
		options,
		label,
		allowCustom = false,
		customLabel = ru.fields.custom,
		oncreate
	}: {
		selected: string[];
		options: string[];
		label: string;
		allowCustom?: boolean;
		customLabel?: string;
		oncreate?: (value: string) => void;
	} = $props();

	let extra = $state<string[]>([]);
	let adding = $state(false);
	let draft = $state('');
	let draftEl = $state<HTMLInputElement | null>(null);

	const all = $derived([...options, ...extra.filter((x) => !options.includes(x))]);

	function toggle(o: string) {
		selected = selected.includes(o) ? selected.filter((x) => x !== o) : [...selected, o];
	}

	function commit() {
		const text = draft.trim().replace(/\s+/g, ' ');
		draft = '';
		adding = false;
		if (text === '') return;
		const key = normalizeSearch(text);
		const existing = all.find((o) => normalizeSearch(o) === key);
		if (existing !== undefined) {
			if (!selected.includes(existing)) selected = [...selected, existing];
			return;
		}
		extra = [...extra, text];
		selected = [...selected, text];
		oncreate?.(text);
	}

	$effect(() => {
		if (adding) draftEl?.focus();
	});
</script>

<div class="chips" role="group" aria-label={label}>
	{#each all as o (o)}
		<button
			type="button"
			class="chip"
			aria-pressed={selected.includes(o)}
			onclick={() => toggle(o)}
		>
			{o}
		</button>
	{/each}
	{#if allowCustom}
		{#if adding}
			<input
				class="draft"
				type="text"
				aria-label={customLabel}
				bind:this={draftEl}
				bind:value={draft}
				onkeydown={(e) => {
					if (e.key === 'Enter') {
						e.preventDefault();
						commit();
					} else if (e.key === 'Escape') {
						e.stopPropagation();
						draft = '';
						adding = false;
					}
				}}
				onblur={commit}
			/>
		{:else}
			<button type="button" class="chip add" onclick={() => (adding = true)}>{customLabel}</button>
		{/if}
	{/if}
</div>

<style>
	.chips {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
	}
	.chip,
	.draft {
		box-sizing: border-box;
		height: var(--control-sm);
		padding: 0 var(--space-3);
		border: 1px solid var(--border-strong);
		border-radius: var(--radius-full);
		background: var(--surface);
		color: var(--text);
		font: inherit;
		font-size: var(--text-md);
	}
	.chip {
		cursor: pointer;
	}
	.chip:hover {
		background: var(--surface-3);
	}
	.chip[aria-pressed='true'] {
		background: var(--accent-soft);
		border-color: var(--accent);
		color: var(--accent-text);
	}
	.add {
		border-style: dashed;
		color: var(--text-2);
	}
	@media (pointer: coarse) {
		.chip,
		.draft {
			min-height: var(--control-md);
			font-size: max(16px, var(--text-lg));
		}
	}
</style>
