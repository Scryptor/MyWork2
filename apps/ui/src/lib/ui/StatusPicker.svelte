<!-- The status of an order: the first `max` statuses as labels in their hue, the rest in the «Ещё» menu;
     the chosen one filled with its dot colour and outlined. Arrows move focus, Space, Enter or a click
     choose. On a phone: a button with the current status, the list rises from the bottom. -->
<script lang="ts">
	import { display } from '#lib/design/prefs.svelte.js';
	import Check from '@lucide/svelte/icons/check';
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import { statusHue } from '#lib/design/status-colors.js';
	import { ru } from '#lib/i18n/ru.js';
	import Dialog from './Dialog.svelte';
	import DropdownMenu, { type MenuItem } from './DropdownMenu.svelte';
	import StatusBadge from './StatusBadge.svelte';

	let {
		statuses,
		value = $bindable(),
		label = ru.status.label,
		disabled = false,
		max = 7,
		onchange
	}: {
		statuses: { id: number; name: string; color: string }[];
		value: number;
		label?: string;
		disabled?: boolean;
		/** Statuses shown as labels; the rest go to the «Ещё» menu. */
		max?: number;
		onchange?: (id: number, previous: number) => void;
	} = $props();

	const name = $props.id();
	const current = $derived(statuses.find((s) => s.id === value));
	let listOpen = $state(false);
	let group = $state<HTMLElement | null>(null);
	const shown = $derived(statuses.slice(0, max));
	const rest = $derived(statuses.slice(max));
	/** The chosen status when it is in the «Ещё» menu: the menu button shows it. */
	const chosenInMenu = $derived(rest.find((s) => s.id === value));
	const menu = $derived<MenuItem[]>(
		rest.map((s) => ({
			label: s.name,
			icon: s.id === value ? Check : undefined,
			onselect: () => pick(s.id)
		}))
	);

	/**
	 * With `onchange` the parent decides: it sets the new `value`, or keeps the old one to refuse the change
	 * (issuing, for one, goes through a dialog). Without `onchange` a bound `value` takes the choice.
	 */
	function pick(id: number) {
		const previous = value;
		if (disabled || id === previous) return;
		if (onchange) onchange(id, previous);
		else value = id;
		syncRadios();
	}

	/** The browser has checked the radio that was clicked or reached by an arrow: show `value` instead. */
	function syncRadios() {
		for (const input of group?.querySelectorAll<HTMLInputElement>('input[type="radio"]') ?? []) {
			input.checked = Number(input.value) === value;
		}
	}

	function choose(id: number) {
		listOpen = false;
		pick(id);
	}

	const STEP: Record<string, number> = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 };

	/** Arrows move focus over the labels without choosing (a choice may open «Выдать»); Enter chooses. */
	function onkeydown(e: KeyboardEvent, id: number) {
		if (e.altKey || e.ctrlKey || e.metaKey) return;
		if (e.shiftKey && STEP[e.key] === undefined) return;
		if (e.key === 'Enter') {
			e.preventDefault();
			pick(id);
			return;
		}
		const step = STEP[e.key];
		if (step === undefined) return;
		// Shift+arrow too: the native radio would check the status it reaches, and a choice may open «Выдать».
		e.preventDefault();
		const radios = [...(group?.querySelectorAll<HTMLInputElement>('input[type="radio"]') ?? [])];
		const at = radios.indexOf(e.currentTarget as HTMLInputElement);
		radios[(at + step + radios.length) % radios.length]?.focus();
	}
</script>

{#if display.phone}
	<button
		type="button"
		class="trigger"
		aria-haspopup="dialog"
		aria-label="{label}: {current?.name ?? ''}"
		{disabled}
		onclick={() => (listOpen = true)}
	>
		{#if current}<StatusBadge name={current.name} color={current.color} />{/if}
		<ChevronDown size={16} aria-hidden="true" />
	</button>
	<Dialog bind:open={listOpen} title={label} position="bottom">
		<ul class="list">
			{#each statuses as s (s.id)}
				<li>
					<button
						type="button"
						class="choice"
						aria-current={s.id === value ? 'true' : undefined}
						onclick={() => choose(s.id)}
					>
						<StatusBadge name={s.name} color={s.color} />
						{#if s.id === value}<Check size={18} aria-hidden="true" />{/if}
					</button>
				</li>
			{/each}
		</ul>
	</Dialog>
{:else}
	<div class="picker" class:disabled>
		<div
			bind:this={group}
			class="options"
			role="radiogroup"
			aria-label={label}
			aria-disabled={disabled ? 'true' : undefined}
		>
			{#each shown as s (s.id)}
				{@const on = s.id === value}
				<label class="option st-{statusHue(s.color)}" class:on>
					<input
						type="radio"
						{name}
						value={s.id}
						checked={on}
						{disabled}
						onchange={() => pick(s.id)}
						onkeydown={(e) => onkeydown(e, s.id)}
					/>
					{#if on}
						<Check size={14} aria-hidden="true" />
					{:else}
						<span class="dot" aria-hidden="true"></span>
					{/if}
					<span>{s.name}</span>
				</label>
			{/each}
		</div>
		{#if rest.length > 0}
			<DropdownMenu items={menu} label="{label}: {ru.common.more.toLowerCase()}">
				{#snippet trigger(props)}
					<button
						{...props}
						type="button"
						class={chosenInMenu ? `option on st-${statusHue(chosenInMenu.color)}` : 'option more'}
						aria-label={chosenInMenu ? `${ru.common.more}: ${chosenInMenu.name}` : undefined}
						{disabled}
					>
						{#if chosenInMenu}
							<Check size={14} aria-hidden="true" />
							<span>{chosenInMenu.name}</span>
						{:else}
							<span>{ru.common.more}</span>
						{/if}
						<ChevronDown size={14} aria-hidden="true" />
					</button>
				{/snippet}
			</DropdownMenu>
		{/if}
	</div>
{/if}

<style>
	.picker,
	.options {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
	}
	.picker {
		padding: var(--space-1);
	}
	.option {
		position: relative;
		display: inline-flex;
		align-items: center;
		gap: var(--space-1-5);
		box-sizing: border-box;
		height: var(--control-sm);
		padding: 0 var(--space-3) 0 var(--space-2);
		border-radius: var(--radius-full);
		background: var(--st-bg);
		color: var(--st-fg);
		font-size: var(--text-sm);
		font-weight: 500;
		white-space: nowrap;
		cursor: pointer;
	}
	.option:hover:not(.on) {
		box-shadow: inset 0 0 0 1px var(--st-dot);
	}
	button.option {
		border: 0;
		font-family: inherit;
	}
	button.option:disabled {
		cursor: not-allowed;
	}
	/* «Ещё» while the chosen status is among the labels. */
	.option.more {
		--st-dot: var(--border-strong);
		background: var(--surface);
		color: var(--text-2);
		box-shadow: inset 0 0 0 1px var(--border);
	}
	/* The chosen one: filled with its dot colour and outlined, with a check, not by colour alone. */
	.option.on {
		background: var(--st-dot);
		color: var(--st-on-dot);
		font-weight: 600;
		box-shadow:
			0 0 0 2px var(--surface),
			0 0 0 4px var(--st-dot);
	}
	.dot {
		width: 0.5rem;
		height: 0.5rem;
		border-radius: var(--radius-full);
		background: var(--st-dot);
	}
	input {
		position: absolute;
		inset: 0;
		margin: 0;
		opacity: 0;
		cursor: inherit;
	}
	.option:has(input:focus-visible) {
		outline: 2px solid var(--focus);
		outline-offset: 6px;
	}
	.disabled .option {
		cursor: not-allowed;
	}
	.disabled .option:not(.on) {
		opacity: 0.55;
	}
	.trigger {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		box-sizing: border-box;
		min-height: var(--control-md);
		padding: 0 var(--space-2) 0 var(--space-1);
		border: 1px solid var(--border-strong);
		border-radius: var(--radius-full);
		background: var(--surface);
		color: var(--text-2);
		font: inherit;
		cursor: pointer;
	}
	.trigger:disabled {
		color: var(--text-disabled);
		cursor: not-allowed;
	}
	.list {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		margin: 0;
		padding: 0;
		list-style: none;
	}
	.choice {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
		box-sizing: border-box;
		width: 100%;
		min-height: var(--touch-target);
		padding: 0 var(--space-2);
		border: 0;
		border-radius: var(--radius-md);
		background: transparent;
		color: var(--text);
		font: inherit;
		cursor: pointer;
	}
	.choice:hover {
		background: var(--surface-3);
	}
	.choice[aria-current='true'] {
		background: var(--accent-soft);
		color: var(--accent-text);
	}
</style>
