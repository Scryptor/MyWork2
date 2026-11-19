<script lang="ts">
	import TagInput from './TagInput.svelte';

	let { values = [], onsubmit }: { values?: string[]; onsubmit?: () => void } = $props();

	/* svelte-ignore state_referenced_locally */
	let v = $state(values);
</script>

<form
	onsubmit={(e) => {
		e.preventDefault();
		onsubmit?.();
	}}
>
	<TagInput bind:values={v} label="Мастера" />
	<button type="submit">Отправить</button>
</form>
<output data-testid="out">{JSON.stringify(v)}</output>
