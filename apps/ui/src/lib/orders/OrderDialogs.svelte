<!-- The dialogs of the order's actions, shared by the panel and the full card: «Выдать», «Вернуть в
     работу», «Принять по гарантии». -->
<script lang="ts">
	import type { MasterList } from './masters.js';
	import { app } from '#lib/app/context.svelte.js';
	import type { OrderActions } from './actions.svelte.js';
	import IssueDialog from './IssueDialog.svelte';
	import ReturnToWorkDialog from './ReturnToWorkDialog.svelte';
	import WarrantyCaseDialog from './WarrantyCaseDialog.svelte';

	let {
		actions,
		dictionaries
	}: {
		actions: OrderActions;
		dictionaries: { masters: MasterList; warranty: string[] };
	} = $props();
</script>

<IssueDialog
	bind:open={actions.issueOpen}
	order={actions.editor?.current ?? null}
	{dictionaries}
	defaultWarranty={app.settings.default_warranty}
	onissue={(values) => actions.issue(values)}
/>
<ReturnToWorkDialog
	bind:open={actions.returnOpen}
	order={actions.editor?.order ?? null}
	statuses={actions.backToWork}
	onreturn={(id) => actions.returnToWork(id)}
/>
<WarrantyCaseDialog
	bind:open={actions.warrantyOpen}
	order={actions.editor?.order ?? null}
	onaccept={(c) => actions.acceptWarranty(c)}
/>
