<!-- FieldsProps for unit tests: the tooltips of its buttons need their provider. -->
<script lang="ts">
	import { Tooltip } from 'bits-ui';
	import type { TemplateEditor } from '../editor.svelte.js';
	import FieldsProps from './FieldsProps.svelte';

	let { editor, path }: { editor: TemplateEditor; path: number[] } = $props();
</script>

<Tooltip.Provider>
	<FieldsProps {editor} {path} />
</Tooltip.Provider>
