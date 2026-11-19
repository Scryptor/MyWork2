<!-- The report of the last transfer: where the sign-in leads after an import that brought users. The import
     may still be placing its files: its progress until it ends, then its report. A server restarted since
     knows no import any more: the latest of the past imports. -->
<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import type { ImportJob } from '$api/ImportJob';
	import { errorMessage } from '#lib/api/client.js';
	import { currentLegacyImport, listLegacyImports } from '#lib/api/import.js';
	import { ru } from '#lib/i18n/ru.js';
	import { reportRoute } from '#lib/import/report.js';
	import ImportProgress from '#lib/setup/ImportProgress.svelte';
	import { followImport, jobState, type Followed } from '#lib/setup/progress.js';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import Skeleton from '#lib/ui/Skeleton.svelte';

	const t = ru.legacy;
	let job = $state.raw<ImportJob | null>(null);
	let problem = $state<string | null>(null);
	/** The import wrote users of its own: not a failure, a sign-in. */
	let signin = $state(false);
	let none = $state(false);
	let gone = false;
	let stop: (() => void) | null = null;

	const open = (id: number) => goto(resolve(reportRoute(id)), { replaceState: true });

	async function latestOfThePast() {
		try {
			const [latest] = await listLegacyImports();
			if (gone) return;
			if (latest) await open(latest.id);
			else none = true;
		} catch (e) {
			if (!gone) problem = errorMessage(e);
		}
	}

	function onstate(s: Followed) {
		if (s.kind === 'running') job = s.job;
		else if (s.kind === 'done') void open(s.importId);
		// The base was written (the files may wait): its report says so.
		else if (s.kind === 'failed' && s.importId !== null) void open(s.importId);
		else if (s.kind === 'failed' || s.kind === 'forbidden') problem = s.message;
		else if (s.kind === 'signin') signin = true;
		else void latestOfThePast();
	}

	onMount(() => {
		void (async () => {
			let current: ImportJob | null = null;
			try {
				current = await currentLegacyImport();
			} catch {
				// Not here (a server on the network) or not for this user: the past imports say.
			}
			if (gone) return;
			if (current === null) return latestOfThePast();
			const state = jobState(current);
			if (state.kind === 'running') {
				job = current;
				stop = followImport(currentLegacyImport, onstate);
			} else onstate(state);
		})();
		return () => {
			gone = true;
			stop?.();
		};
	});
</script>

<h1 class="title">{t.report}</h1>
{#if problem}
	<Callout tone="danger">{problem}</Callout>
	<div><Button href={resolve('settings/import')}>{t.past}</Button></div>
{:else if signin}
	<Callout tone="info">{t.signIn}</Callout>
{:else if none}
	<p class="hint">{t.none}</p>
{:else if job}
	<ImportProgress {job} />
{:else}
	<Skeleton height="6rem" radius="md" />
{/if}

<style>
	.title {
		margin: 0;
		font-size: var(--text-xl);
		font-weight: 600;
	}
	.hint {
		margin: 0;
		color: var(--text-3);
	}
</style>
