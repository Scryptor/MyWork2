<!-- «Настройки» itself: on a phone the list of subsections (drawn by the layout), elsewhere the first one. -->
<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { display } from '#lib/design/prefs.svelte.js';
	import { SETTINGS_SECTIONS } from '#lib/settings/sections.js';

	// After the first paint: on a direct visit the size of the window is read by the root layout's effect,
	// which runs after this one.
	$effect(() => {
		const first = SETTINGS_SECTIONS[0];
		const timer = setTimeout(() => {
			if (!display.phone && first) void goto(resolve(first.href), { replaceState: true });
		});
		return () => clearTimeout(timer);
	});
</script>
