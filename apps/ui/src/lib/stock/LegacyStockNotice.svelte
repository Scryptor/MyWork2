<!-- Above the list of «Склад» while the stock of MyWork 2 kept by a past import waits to be transferred (decision
     17): the first position added here, even one deleted later, closes that transfer for good. The one who may
     transfer (settings_manage + users_manage) gets the way to «Импорт»; the others are told who can. -->
<script lang="ts">
	import { resolve } from '$app/paths';
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';

	let { canMigrate }: { canMigrate: boolean } = $props();

	const t = ru.stock;
</script>

{#if canMigrate}
	<Callout tone="warning" title={t.legacyTitle}>
		{t.legacyText}
		{#snippet actions()}
			<Button size="sm" href={resolve('settings/import')}>{t.legacyOpen}</Button>
		{/snippet}
	</Callout>
{:else}
	<Callout tone="warning" title={t.legacyTitle}>{t.legacyTextNoRights}</Callout>
{/if}
