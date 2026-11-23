<!-- BlockList for unit tests: the tooltips of its buttons need their provider. -->
<script lang="ts">
	import { Tooltip } from 'bits-ui';
	import BlockList from './BlockList.svelte';
	import type { TemplateEditor } from './editor.svelte.js';

	let { editor }: { editor: TemplateEditor } = $props();
</script>

<Tooltip.Provider>
	<BlockList {editor} />
</Tooltip.Provider>
