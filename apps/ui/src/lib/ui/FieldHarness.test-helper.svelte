<script lang="ts">
	import MoneyInput from './MoneyInput.svelte';
	import PhoneInput from './PhoneInput.svelte';
	import ChipGroup from './ChipGroup.svelte';
	import Switch from './Switch.svelte';

	let {
		kind,
		money = null,
		nullable = true,
		phone = '',
		selected = [],
		options = [],
		checked = false,
		oncreate
	}: {
		kind: 'money' | 'phone' | 'chips' | 'switch';
		money?: number | null;
		nullable?: boolean;
		phone?: string;
		selected?: string[];
		options?: string[];
		checked?: boolean;
		oncreate?: (v: string) => void;
	} = $props();

	/* svelte-ignore state_referenced_locally */
	let m = $state(money);
	/* svelte-ignore state_referenced_locally */
	let p = $state(phone);
	/* svelte-ignore state_referenced_locally */
	let s = $state(selected);
	/* svelte-ignore state_referenced_locally */
	let c = $state(checked);
</script>

{#if kind === 'money'}
	<MoneyInput bind:value={m} {nullable} label="Сумма" />
	<output data-testid="out">{JSON.stringify(m)}</output>
{:else if kind === 'phone'}
	<PhoneInput bind:value={p} label="Телефон" />
	<output data-testid="out">{p}</output>
{:else if kind === 'chips'}
	<ChipGroup bind:selected={s} {options} label="Комплектность" allowCustom {oncreate} />
	<output data-testid="out">{JSON.stringify(s)}</output>
{:else}
	<Switch bind:checked={c} label="Включено" />
{/if}
