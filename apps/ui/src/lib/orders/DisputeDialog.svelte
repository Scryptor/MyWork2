<!-- «Есть несохранённый спор»: leaving an order whose conflicting fields are undecided (see `dispute`). -->
<script lang="ts">
	import { ru } from '#lib/i18n/ru.js';
	import ConfirmDialog from '#lib/ui/ConfirmDialog.svelte';
	import { dispute } from './dispute.svelte.js';
</script>

<ConfirmDialog
	bind:open={() => dispute.pending !== null, (v) => !v && dispute.answer(false)}
	title={ru.order.disputeTitle}
	description={ru.order.disputeText(dispute.pending ?? 0)}
	cancelLabel={ru.order.disputeStay}
	confirmLabel={ru.order.disputeLeave}
	onconfirm={() => dispute.answer(true)}
/>
