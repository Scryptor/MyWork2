<script lang="ts">
	import CodeEditor from './CodeEditor.svelte';

	let {
		value = '',
		errors,
		hint
	}: {
		value?: string;
		errors?: readonly { line: number; column: number; message: string }[];
		hint?: string;
	} = $props();
	/* svelte-ignore state_referenced_locally */
	let v = $state(value);
</script>

<CodeEditor bind:value={v} label="Код" {errors} {hint} />
