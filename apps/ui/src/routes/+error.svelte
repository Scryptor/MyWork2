<!-- A screen that could not open (spec 9): what happened and what to do. No shell: it may be the
     program itself that did not load. -->
<script lang="ts">
	import RotateCw from '@lucide/svelte/icons/rotate-cw';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import '#lib/app/page-card.css';
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';

	const title = $derived(page.error?.message || ru.app.unexpected);
</script>

<svelte:head><title>{title}</title></svelte:head>

<main class="mw-page-center" tabindex="-1">
	<div class="mw-page-card">
		<h1>{title}</h1>
		{#if page.error?.offline}<p>{ru.app.offlineHint}</p>{/if}
		{#if page.status === 404}
			<Button variant="primary" href={resolve('orders')}>{ru.app.toOrders}</Button>
		{:else}
			<Button variant="primary" icon={RotateCw} onclick={() => location.reload()}
				>{ru.common.retry}</Button
			>
		{/if}
	</div>
</main>
