<!-- «Выгрузить» of the clients' list: the menu with «В Excel» and «Контакты для телефона…»; only with «data_export». -->
<script lang="ts">
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import Contact from '@lucide/svelte/icons/contact';
	import FileSpreadsheet from '@lucide/svelte/icons/file-spreadsheet';
	import type { ClientListQuery } from '$api/ClientListQuery';
	import { downloadClients } from '#lib/api/exports.js';
	import ContactsExportDialog from '#lib/clients/ContactsExportDialog.svelte';
	import { app } from '#lib/app/context.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';
	import DropdownMenu, { type MenuItem } from '#lib/ui/DropdownMenu.svelte';
	import { runExport } from './run-export.js';

	let { query }: { query: ClientListQuery } = $props();

	let busy = $state(false);
	let contactsOpen = $state(false);
	async function run(download: () => Promise<void>) {
		busy = true;
		await runExport(download);
		busy = false;
	}
	const items = $derived<MenuItem[]>([
		{
			label: ru.exports.toExcel,
			icon: FileSpreadsheet,
			onselect: () => void run(() => downloadClients(query))
		},
		{
			label: ru.exports.contacts.menu,
			icon: Contact,
			onselect: () => (contactsOpen = true)
		}
	]);
</script>

{#if app.can('data_export')}
	<DropdownMenu {items} label={ru.exports.menu}>
		{#snippet trigger(props)}
			<Button {...props} iconEnd={ChevronDown} {busy} disabled={busy}>{ru.exports.menu}</Button>
		{/snippet}
	</DropdownMenu>
	<ContactsExportDialog bind:open={contactsOpen} {query} />
{/if}
