<!-- «Ссылки для QR-кодов» for unit tests: with the toasts, as on the settings page of the app. -->
<script lang="ts">
	import { Tooltip } from 'bits-ui';
	import Toaster from '#lib/ui/toast/Toaster.svelte';
	import FirmLinks from './FirmLinks.svelte';
	import type { SettingsEditor } from './settings-editor.svelte.js';

	let { editor }: { editor: SettingsEditor } = $props();
</script>

<Tooltip.Provider>
	<FirmLinks {editor} />
	<Toaster />
</Tooltip.Provider>
