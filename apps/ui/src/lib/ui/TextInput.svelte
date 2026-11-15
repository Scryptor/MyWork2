<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { HTMLInputAttributes } from 'svelte/elements';
	import Eye from '@lucide/svelte/icons/eye';
	import EyeOff from '@lucide/svelte/icons/eye-off';
	import { ru } from '#lib/i18n/ru.js';
	import Field from './Field.svelte';

	let {
		value = $bindable(''),
		type = 'text',
		placeholder,
		revealable = false,
		prefix,
		suffix,
		size = 'md',
		readonly = false,
		autofocus = false,
		inputmode,
		maxlength,
		name,
		oninput,
		onblur,
		label,
		hint,
		error,
		required = false,
		disabled = false,
		id
	}: {
		value: string;
		type?: 'text' | 'search' | 'password' | 'tel' | 'email';
		placeholder?: string;
		revealable?: boolean;
		prefix?: Snippet;
		suffix?: Snippet;
		size?: 'sm' | 'md';
		readonly?: boolean;
		autofocus?: boolean;
		inputmode?: HTMLInputAttributes['inputmode'];
		maxlength?: number;
		name?: string;
		oninput?: (v: string) => void;
		onblur?: () => void;
		label?: string;
		hint?: string;
		error?: string;
		required?: boolean;
		disabled?: boolean;
		id?: string;
	} = $props();

	const uid = $props.id();
	const fid = $derived(id ?? uid);
	let revealed = $state(false);
	const shownType = $derived(type === 'password' && revealed ? 'text' : type);
</script>

<Field {label} {hint} {error} {required} id={fid}>
	{#snippet children(f)}
		<div class="mw-box {size}" class:invalid={f.invalid} class:disabled>
			{#if prefix}<span class="mw-affix">{@render prefix()}</span>{/if}
			<!-- svelte-ignore a11y_autofocus -->
			<input
				class="mw-input"
				id={f.id}
				type={shownType}
				bind:value
				{placeholder}
				{readonly}
				{disabled}
				{autofocus}
				{inputmode}
				{maxlength}
				{name}
				aria-describedby={f.describedBy}
				aria-invalid={f.invalid ? 'true' : undefined}
				aria-required={required ? 'true' : undefined}
				oninput={() => oninput?.(value)}
				{onblur}
			/>
			{#if type === 'password' && revealable}
				<button
					type="button"
					class="reveal"
					{disabled}
					aria-pressed={revealed}
					aria-label={revealed ? ru.fields.hidePassword : ru.fields.showPassword}
					onclick={() => (revealed = !revealed)}
				>
					{#if revealed}<EyeOff size={16} aria-hidden="true" />{:else}<Eye
							size={16}
							aria-hidden="true"
						/>{/if}
				</button>
			{/if}
			{#if suffix}<span class="mw-affix">{@render suffix()}</span>{/if}
		</div>
	{/snippet}
</Field>

<style>
	.reveal {
		display: inline-flex;
		align-items: center;
		padding: 0;
		border: 0;
		background: transparent;
		color: var(--text-3);
		cursor: pointer;
	}
	.reveal:hover {
		color: var(--text);
	}
</style>
