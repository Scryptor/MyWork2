<script lang="ts">
	import DateInput from './DateInput.svelte';
	import MoneyInput from './MoneyInput.svelte';
	import PhoneInput from './PhoneInput.svelte';
	import QuantityInput from './QuantityInput.svelte';
	import ChipGroup from './ChipGroup.svelte';
	import Switch from './Switch.svelte';

	let {
		kind,
		money = null,
		quantity = 0,
		max,
		nullable = true,
		phone = '',
		date = null,
		min,
		selected = [],
		options = [],
		checked = false,
		oncreate,
		onwrite
	}: {
		kind: 'money' | 'phone' | 'date' | 'chips' | 'switch' | 'quantity';
		money?: number | null;
		quantity?: number;
		max?: number;
		nullable?: boolean;
		phone?: string;
		date?: string | null;
		min?: string;
		selected?: string[];
		options?: string[];
		checked?: boolean;
		oncreate?: (v: string) => void;
		/** Every write of the field into its bound value. */
		onwrite?: (v: unknown) => void;
	} = $props();

	// The values follow the props, like a panel switched to another record, and take the field's writes.
	let m = $derived(money);
	let p = $derived(phone);
	let d = $derived(date);
	let q = $derived(quantity);
	let invalid = $state(false);
	/* svelte-ignore state_referenced_locally */
	let s = $state(selected);
	/* svelte-ignore state_referenced_locally */
	let c = $state(checked);
</script>

{#if kind === 'money'}
	<MoneyInput
		bind:value={
			() => m,
			(v) => {
				onwrite?.(v);
				m = v;
			}
		}
		bind:invalid
		{nullable}
		label="Сумма"
	/>
	<output data-testid="out">{JSON.stringify(m)}</output>
	<output data-testid="invalid">{invalid}</output>
{:else if kind === 'quantity'}
	<QuantityInput
		bind:value={
			() => q,
			(v) => {
				onwrite?.(v);
				q = v;
			}
		}
		{max}
		label="Количество"
	/>
	<output data-testid="out">{q}</output>
{:else if kind === 'phone'}
	<PhoneInput
		bind:value={
			() => p,
			(v) => {
				onwrite?.(v);
				p = v;
			}
		}
		label="Телефон"
	/>
	<output data-testid="out">{p}</output>
{:else if kind === 'date'}
	<DateInput
		bind:value={
			() => d,
			(v) => {
				onwrite?.(v);
				d = v;
			}
		}
		bind:invalid
		{min}
		label="Дата"
	/>
	<output data-testid="out">{JSON.stringify(d)}</output>
	<output data-testid="invalid">{invalid}</output>
{:else if kind === 'chips'}
	<ChipGroup bind:selected={s} {options} label="Комплектность" allowCustom {oncreate} />
	<output data-testid="out">{JSON.stringify(s)}</output>
{:else}
	<Switch bind:checked={c} label="Включено" />
{/if}
