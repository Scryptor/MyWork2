<script lang="ts" generics="T extends string | number">
	import { Select } from 'bits-ui';
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import { untrack } from 'svelte';
	import { layers } from '#lib/design/layers.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import Field from './Field.svelte';
	import './popup.css';

	let {
		value = $bindable(null),
		items,
		placeholder = ru.fields.select,
		label,
		hint,
		error,
		required = false,
		disabled = false,
		id
	}: {
		value: T | null;
		items: { value: T; label: string; disabled?: boolean }[];
		placeholder?: string;
		label?: string;
		hint?: string;
		error?: string;
		required?: boolean;
		disabled?: boolean;
		id?: string;
	} = $props();

	const uid = $props.id();
	const fid = $derived(id ?? uid);
	let open = $state(false);
	const bitsItems = $derived(
		items.map((i) => ({ value: String(i.value), label: i.label, disabled: i.disabled }))
	);
	const current = $derived(items.find((i) => i.value === value));

	$effect(() => {
		if (open) return untrack(() => layers.open('popup', () => (open = false)));
	});
</script>

<Field {label} {hint} {error} {required} id={fid}>
	{#snippet children(f)}
		<Select.Root
			type="single"
			{disabled}
			bind:open
			value={value === null ? '' : String(value)}
			onValueChange={(v) => {
				value = items.find((i) => String(i.value) === v)?.value ?? null;
			}}
			items={bitsItems}
		>
			<Select.Trigger
				id={f.id}
				class="mw-box select"
				aria-describedby={f.describedBy}
				aria-invalid={f.invalid ? 'true' : undefined}
				aria-required={required ? 'true' : undefined}
			>
				<span class="val" class:ph={!current}>{current ? current.label : placeholder}</span>
				<ChevronDown size={16} aria-hidden="true" class="chev" />
			</Select.Trigger>
			<Select.Portal>
				<Select.Content class="mw-popup" sideOffset={4}>
					<Select.Viewport>
						{#each items as i (i.value)}
							<Select.Item
								class="mw-option"
								value={String(i.value)}
								label={i.label}
								disabled={i.disabled}
							>
								{i.label}
							</Select.Item>
						{/each}
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
	:global(.chev) {
		flex: none;
		color: var(--text-3);
	}
</style>
