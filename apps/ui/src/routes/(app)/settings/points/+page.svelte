<!-- «Точки» (spec 6.7): the service points — the same list as «Статусы», without kind or colour. -->
<script lang="ts">
	import { onMount } from 'svelte';
	import type { ServicePoint } from '$api/ServicePoint';
	import type { ServicePointPatch } from '$api/ServicePointPatch';
	import {
		createServicePoint,
		listServicePoints,
		reorderServicePoints,
		servicePointUsage,
		updateServicePoint
	} from '#lib/api/catalogs.js';
	import { app } from '#lib/app/context.svelte.js';
	import { resolve } from '$app/paths';
	import { countLabel as countOf } from '#lib/format/index.js';
	import { ru } from '#lib/i18n/ru.js';
	import CatalogRows from '#lib/settings/CatalogRows.svelte';
	import { createNamedCatalog } from '#lib/settings/named-catalog.svelte.js';
	import Callout from '#lib/ui/Callout.svelte';
	import Card from '#lib/ui/Card.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import Skeleton from '#lib/ui/Skeleton.svelte';
	import TextInput from '#lib/ui/TextInput.svelte';
	import { toast } from '#lib/ui/toast/toast.svelte.js';

	const t = ru.points;
	const catalog = createNamedCatalog<ServicePoint, string, ServicePointPatch>(
		{
			list: () => listServicePoints(true),
			usage: servicePointUsage,
			add: createServicePoint,
			update: updateServicePoint,
			reorder: reorderServicePoints
		},
		() => app.refreshCatalogs(),
		() => app.can('settings_manage')
	);
	const readonly = $derived(!app.can('settings_manage'));
	const countLabel = (n: number) => ru.dictionaries.countIn(countOf(n, ru.dictionaries.inOrders));

	onMount(() => void catalog.load());

	const fail = (message: string) => toast.show({ message, tone: 'danger' });
	let name = $state('');
	let nameError = $state<string | undefined>();

	async function add() {
		const value = name.trim();
		if (readonly) return;
		if (!value) {
			nameError = ru.settings.required;
			return;
		}
		const error = await catalog.add(value);
		if (error) nameError = error;
		else {
			name = '';
			nameError = undefined;
		}
	}

	async function patch(id: number, p: ServicePointPatch) {
		const error = await catalog.update(id, p);
		if (error) fail(error);
	}

	const nothing = $derived(!catalog.loading && !catalog.loadError && catalog.rows.length === 0);
</script>

<h1 class="title">{ru.settings.points}</h1>

{#if readonly}<Callout tone="info">{ru.settings.readOnly}</Callout>{/if}

<Card>
	<div class="body">
		<p class="note">
			{t.hint}
			<a href={resolve('settings/orders')}>{t.defaultPoint}</a>
		</p>

		{#if !readonly}
			<form
				onsubmit={(e) => {
					e.preventDefault();
					void add();
				}}
			>
				<TextInput
					ariaLabel={t.add}
					placeholder={t.add}
					autocomplete="off"
					error={nameError}
					bind:value={() => name, (v) => ((name = v), (nameError = undefined))}
				/>
			</form>
		{/if}

		{#if catalog.loadError}
			<Callout tone="danger">{t.loadFailed}: {catalog.loadError}</Callout>
		{:else if catalog.loading && catalog.rows.length === 0}
			<Skeleton height="2rem" />
		{:else if nothing}
			<EmptyState title={t.empty} />
		{:else}
			<CatalogRows
				rows={catalog.rows}
				{countLabel}
				{readonly}
				onrename={(id, label) => void patch(id, { name: label })}
				onarchive={(id, archived) => void patch(id, { archived })}
				onreorder={async (ids) => {
					const error = await catalog.reorder(ids);
					if (error) fail(error);
				}}
			/>
		{/if}
	</div>
</Card>

<style>
	.title {
		margin: 0;
		font-size: var(--text-xl);
		font-weight: 600;
	}
	.body {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		padding: var(--space-3);
	}
	.note {
		margin: 0;
		font-size: var(--text-sm);
		color: var(--text-3);
	}
	.note a {
		color: var(--accent-text);
	}
</style>
