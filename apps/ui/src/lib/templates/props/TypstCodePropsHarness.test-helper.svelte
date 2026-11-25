<script lang="ts">
	import { Tooltip } from 'bits-ui';
	import type { TemplateEditor } from '../editor.svelte.js';
	import type { BlockPath } from '../ops.js';
	import TypstCodeProps from './TypstCodeProps.svelte';

	let { editor, path }: { editor: TemplateEditor; path: BlockPath } = $props();
</script>

<Tooltip.Provider>
	<TypstCodeProps {editor} {path} />
</Tooltip.Provider>
