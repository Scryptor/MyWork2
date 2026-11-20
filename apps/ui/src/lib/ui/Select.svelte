<script lang="ts" generics="T extends string | number">
	import { Select } from 'bits-ui';
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import { untrack } from 'svelte';
	import { layers } from '#lib/design/layers.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import Field from './Field.svelte';
	import './popup.css';

	type Item = { value: T; label: string; disabled?: boolean };
	/** A heading over its options (`<optgroup>` of a plain select). */
	type Group = { label: string; options: Item[] };

	let {
		value = $bindable(null),
		items,
		placeholder = ru.fields.select,
		label,
		ariaLabel,
		hint,
		help,
		error,
		required = false,
		disabled = false,
		id
	}: {
		value: T | null;
		/** The options, or the options in groups. */
		items: Item[] | Group[];
		placeholder?: string;
		label?: string;
		/** The name for assistive technology when the visible label is the same in many places. */
		ariaLabel?: string;
		hint?: string;
		/** «Что это?»: an icon right of the label with an explanation. */
		help?: string | readonly string[];
		error?: string;
		required?: boolean;
		disabled?: boolean;
		id?: string;
	} = $props();

	const uid = $props.id();
	const fid = $derived(id ?? uid);
	let open = $state(false);
	const groups = $derived(
		items.length > 0 && items.every((i) => 'options' in i) ? (items as Group[]) : null
	);
	const flat = $derived(groups ? groups.flatMap((g) => g.options) : (items as Item[]));
	const bitsItems = $derived(
		flat.map((i) => ({ value: String(i.value), label: i.label, disabled: i.disabled }))
	);
	const current = $derived(flat.find((i) => i.value === value));

	$effect(() => {
		if (open) return untrack(() => layers.open('popup', () => (open = false)));
	});
</script>

{#snippet option(i: Item)}
	<Select.Item class="mw-option" value={String(i.value)} label={i.label} disabled={i.disabled}>
		{i.label}
	</Select.Item>
{/snippet}

<Field {label} {hint} {help} {error} {required} id={fid}>
	{#snippet children(f)}
		<Select.Root
			type="single"
			{disabled}
			bind:open
			value={value === null ? '' : String(value)}
			onValueChange={(v) => {
				value = flat.find((i) => String(i.value) === v)?.value ?? null;
			}}
			items={bitsItems}
		>
			<Select.Trigger
				id={f.id}
				class="mw-box select"
				aria-label={ariaLabel}
				aria-describedby={f.describedBy}
				aria-invalid={f.invalid ? 'true' : undefined}
				aria-required={required ? 'true' : undefined}
			>
				<!-- A long value (a time zone) is cut in a narrow field: whole in the title. -->
				<span class="val" class:ph={!current} title={current?.label}
					>{current ? current.label : placeholder}</span
				>
				<ChevronDown size={16} aria-hidden="true" class="chev" />
			</Select.Trigger>
			<Select.Portal>
				<Select.Content class="mw-popup mw-select-popup" sideOffset={4} collisionPadding={8}>
					<Select.Viewport>
						{#if groups}
							{#each groups as g (g.label)}
								<Select.Group>
									<Select.GroupHeading class="mw-group-heading">{g.label}</Select.GroupHeading>
									{#each g.options as i (i.value)}{@render option(i)}{/each}
								</Select.Group>
							{/each}
						{:else}
							{#each flat as i (i.value)}{@render option(i)}{/each}
						{/if}
					</Select.Viewport>
				</Select.Content>
			</Select.Portal>
		</Select.Root>
	{/snippet}
</Field>

<style>
	:global(.mw-box.select) {
		justify-content: space-between;
		text-align: left;
		cursor: pointer;
	}
	:global(.mw-box.select[aria-invalid='true']) {
		border-color: var(--danger);
	}
	:global(.mw-box.select[data-disabled]) {
		background: var(--surface-2);
		color: var(--text-disabled);
		cursor: not-allowed;
	}
	.val {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.ph {
		color: var(--text-3);
	}
	/* Long options (the time zones) wrap rather than run off a narrow window. */
	:global(.mw-select-popup) {
		max-width: var(--bits-select-content-available-width);
	}
	/* A little room between the lines of two wrapped options; a one-line option keeps its height. */
	:global(.mw-select-popup .mw-option) {
		padding-block: var(--space-0-5);
	}
	:global(.chev) {
		flex: none;
		color: var(--text-3);
	}
</style>
