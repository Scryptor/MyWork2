<script lang="ts">
	import PatternInput from './PatternInput.svelte';

	let { value = '', disabled = false }: { value?: string; disabled?: boolean } = $props();

	/* svelte-ignore state_referenced_locally */
	let v = $state(value);
</script>

<PatternInput bind:value={v} {disabled} />
<output data-testid="out">{v}</output>
