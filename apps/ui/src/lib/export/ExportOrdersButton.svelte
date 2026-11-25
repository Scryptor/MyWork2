<!-- «В Excel» of the orders' list: the file has what the list shows, in its order; only with «data_export». -->
<script lang="ts">
	import FileSpreadsheet from '@lucide/svelte/icons/file-spreadsheet';
	import type { OrderListQuery } from '$api/OrderListQuery';
	import { downloadOrders } from '#lib/api/exports.js';
	import { app } from '#lib/app/context.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';
	import { runExport } from './run-export.js';

	let { query }: { query: OrderListQuery } = $props();

	let busy = $state(false);
	async function run() {
		busy = true;
		await runExport(() => downloadOrders(query));
		busy = false;
	}
</script>

{#if app.can('data_export')}
	<Button icon={FileSpreadsheet} {busy} disabled={busy} onclick={() => void run()}
		>{ru.exports.toExcel}</Button
	>
{/if}
