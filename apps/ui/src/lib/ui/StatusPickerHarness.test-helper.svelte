<script lang="ts">
	import { DEMO_STATUSES } from '#lib/demo/fake.js';
	import StatusPicker from './StatusPicker.svelte';

	/** `take`: onchange sets the new value; `keep`: onchange leaves it (a veto); `bind`: bind:value only. */
	let { mode, initial = 1 }: { mode: 'take' | 'keep' | 'bind'; initial?: number } = $props();

	/* svelte-ignore state_referenced_locally */
	let value = $state(initial);
	let changes = $state<string[]>([]);
</script>

{#if mode === 'bind'}
	<StatusPicker statuses={DEMO_STATUSES} bind:value />
{:else}
	<StatusPicker
		statuses={DEMO_STATUSES}
		{value}
		onchange={(id, previous) => {
			changes = [...changes, `${previous}→${id}`];
			if (mode === 'take') value = id;
		}}
	/>
{/if}
<output data-testid="value">{value}</output>
<output data-testid="changes">{changes.join(' ')}</output>
