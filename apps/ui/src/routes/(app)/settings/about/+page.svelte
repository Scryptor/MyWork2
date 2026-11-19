<!-- «О программе» (spec 6.8): the version, the database, the data folder, how many orders and clients. -->
<script lang="ts">
	import Copy from '@lucide/svelte/icons/copy';
	import { onMount } from 'svelte';
	import type { AboutInfo } from '$api/AboutInfo';
	import { getAbout } from '#lib/api/catalogs.js';
	import { errorMessage } from '#lib/api/client.js';
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import Card from '#lib/ui/Card.svelte';
	import { toast } from '#lib/ui/toast/toast.svelte.js';

	const t = ru.settings;
	let about = $state<AboutInfo | null>(null);
	let failure = $state<string | null>(null);

	onMount(async () => {
		try {
			about = await getAbout();
		} catch (e) {
			failure = errorMessage(e);
		}
	});

	async function copyDir(dir: string) {
		try {
			await navigator.clipboard.writeText(dir);
			toast.show({ message: t.copied });
		} catch {
			toast.show({ message: ru.api.failed });
		}
	}
</script>

<h1 class="title">{t.about}</h1>

{#if failure}
	<Callout tone="danger" title={t.loadFailed}>{failure}</Callout>
{:else if about}
	<Card>
		<dl class="kv">
			<dt>{t.version}</dt>
			<dd>{about.version}</dd>
			<dt>{t.database}</dt>
			<dd>{about.database}</dd>
			<dt>{t.dataDir}</dt>
			<dd class="dir">
				{#if about.data_dir}
					<span class="path">{about.data_dir}</span>
					<Button size="sm" icon={Copy} onclick={() => copyDir(about!.data_dir!)}>{t.copy}</Button>
				{:else}
					<span class="muted">{t.dataDirUnknown}</span>
				{/if}
			</dd>
			<dt>{t.orderCount}</dt>
			<dd>{about.orders}</dd>
			<dt>{t.clientCount}</dt>
			<dd>{about.clients}</dd>
		</dl>
	</Card>
{/if}

<style>
	.title {
		margin: 0;
		font-size: var(--text-xl);
		font-weight: 600;
	}
	.kv {
		display: grid;
		grid-template-columns: max-content minmax(0, 1fr);
		gap: var(--space-2) var(--space-4);
		align-items: center;
		margin: 0;
		font-size: var(--text-md);
	}
	dt {
		color: var(--text-3);
	}
	dd {
		margin: 0;
		min-width: 0;
	}
	.dir {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
	}
	.path {
		min-width: 0;
		overflow-wrap: anywhere;
	}
	.muted {
		color: var(--text-3);
	}
</style>
