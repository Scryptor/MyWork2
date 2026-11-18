<script lang="ts">
	import { Command } from 'bits-ui';
	import { untrack } from 'svelte';
	import { registerHotkey } from '#lib/design/hotkeys.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import type { CommandGroup, CommandItem } from './commands.js';
	import Dialog from './Dialog.svelte';
	import Kbd from './Kbd.svelte';
	import Spinner from './Spinner.svelte';
	import StatusBadge from './StatusBadge.svelte';

	let {
		open = $bindable(false),
		query = $bindable(''),
		groups,
		placeholder = ru.palette.placeholder,
		loading = false,
		emptyText = ru.palette.empty,
		onselect
	}: {
		open: boolean;
		query: string;
		groups: CommandGroup[];
		placeholder?: string;
		loading?: boolean;
		emptyText?: string;
		onselect: (item: CommandItem, opts: { alt: boolean }) => void;
	} = $props();

	let highlighted = $state('');

	const key = (g: CommandGroup, i: CommandItem) => `${g.id}:${i.id}`;
	const total = $derived(groups.reduce((n, g) => n + g.items.length, 0));

	$effect(() => registerHotkey('Mod+K', () => (open = true), { allowInFields: true }));

	// A closed palette forgets the query and the highlighted item: it opens empty, on «Недавние»
	// (spec 6.10). A query set while it is closed is kept for the next opening.
	$effect(() => {
		if (open) return;
		untrack(() => {
			query = '';
			highlighted = '';
		});
	});

	function choose(item: CommandItem, alt: boolean) {
		open = false;
		onselect(item, { alt });
	}

	// Ctrl/⌘+Enter opens the full view: bits-ui only knows plain Enter, so take it first.
	function onkeydown(e: KeyboardEvent) {
		if (e.key !== 'Enter' || !(e.ctrlKey || e.metaKey) || e.isComposing) return;
		e.preventDefault();
		e.stopPropagation();
		for (const g of groups) {
			const item = g.items.find((i) => key(g, i) === highlighted);
			if (item) return choose(item, true);
		}
	}
</script>

<Dialog bind:open title={ru.palette.title} size="lg">
	<div class="mw-palette" role="presentation" onkeydowncapture={onkeydown}>
		<Command.Root shouldFilter={false} loop bind:value={highlighted}>
			<Command.Input bind:value={query} {placeholder} aria-label={ru.palette.title} class="input" />
			<Command.List class="list">
				<Command.Viewport>
					{#each groups as group (group.id)}
						{#if group.items.length > 0}
							<Command.Group value={group.id}>
								<Command.GroupHeading class="heading label-caps">{group.title}</Command.GroupHeading
								>
								<Command.GroupItems>
									{#each group.items as item (item.id)}
										{@const Icon = item.icon}
										<Command.Item
											value={key(group, item)}
											keywords={item.keywords}
											onSelect={() => choose(item, false)}
											class="item"
										>
											{#if Icon}<Icon size={16} aria-hidden="true" />{/if}
											<span class="label">{item.label}</span>
											{#if item.hint}<span class="hint" title={item.hint}>{item.hint}</span>{/if}
											{#if item.status}
												<StatusBadge name={item.status.name} color={item.status.color} size="sm" />
											{/if}
											{#if item.shortcut}<Kbd shortcut={item.shortcut} />{/if}
										</Command.Item>
									{/each}
								</Command.GroupItems>
							</Command.Group>
						{/if}
					{/each}
				</Command.Viewport>
				{#if loading}
					<div class="note"><Spinner size="sm" /></div>
				{:else if total === 0}
					<div class="note" role="status">{emptyText}</div>
				{/if}
			</Command.List>
		</Command.Root>
	</div>
</Dialog>

<style>
	/* Palette sits in the upper part of the screen; its heading stays for screen readers only. */
	:global(.mw-dialog.lg:has(.mw-palette)) {
		top: 15vh;
		translate: -50% 0;
		width: min(40rem, 100% - 2 * var(--space-4));
		max-height: 70vh;
	}
	:global(.mw-dialog.phone:has(.mw-palette)) {
		top: 0;
		translate: none;
		width: auto;
		max-height: none;
	}
	:global(.mw-dialog:has(.mw-palette) .mw-dialog-head) {
		position: absolute;
		width: 1px;
		height: 1px;
		padding: 0;
		overflow: hidden;
		clip-path: inset(50%);
		white-space: nowrap;
	}
	:global(.mw-dialog:not(.phone):has(.mw-palette) .mw-dialog-close) {
		display: none;
	}
	:global(.mw-dialog.phone:has(.mw-palette) .mw-dialog-body) {
		padding-top: calc(var(--control-md) + 2 * var(--space-3));
	}
	.mw-palette {
		display: flex;
		flex-direction: column;
	}
	:global(.mw-palette [data-command-input]) {
		box-sizing: border-box;
		width: 100%;
		height: var(--control-lg);
		padding: 0 var(--space-3);
		border: 1px solid var(--border-strong);
		border-radius: var(--radius-md);
		background: var(--surface);
		color: var(--text);
		font: inherit;
	}
	:global(.mw-palette [data-command-list]) {
		margin-top: var(--space-2);
	}
	:global(.mw-palette [data-command-group-heading]) {
		padding: var(--space-2) var(--space-2) var(--space-1);
	}
	:global(.mw-palette [data-command-item]) {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-height: var(--control-lg);
		padding: 0 var(--space-2);
		border-radius: var(--radius-md);
		color: var(--text);
		cursor: pointer;
	}
	:global(.mw-palette [data-command-item][data-selected]) {
		background: var(--accent-soft);
		color: var(--accent-text);
	}
	/* From its own width (not 0): with a long hint beside it the label still keeps its text. */
	:global(.mw-palette .label) {
		flex: 1 1 auto;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	/* The hint (the device and the client of an order) gives way first: the label («№ 178») stays whole. */
	:global(.mw-palette .hint) {
		flex: 0 1000 auto;
		min-width: 0;
		overflow: hidden;
		color: var(--text-3);
		font-size: var(--text-sm);
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.note {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-4) var(--space-2);
		color: var(--text-3);
	}
	@media (pointer: coarse) {
		:global(.mw-palette [data-command-item]) {
			min-height: 2.75rem;
		}
	}
</style>
