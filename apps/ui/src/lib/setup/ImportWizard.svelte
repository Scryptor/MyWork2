<!-- The transfer from MyWork 2 (spec 6.9): 1) the folder («Выбрать папку…» in the desktop program, a path
     field in a browser); 2) «Проверить» — what was found or why not; 3) «Перенести» (with `replace` after a
     confirmation: the data is replaced, a copy is made first); 4) the stages, asked for once a second; 5) the
     report, on its own screen. A base that is not empty (the first run after `mw-server create-admin`): the
     server answers `replace_required` (SQLite), and «Перенести с заменой» checks again for the replacement,
     with the same confirmation. `resume`: an import under way when the screen opened (a reloaded page, another
     tab) — its progress at once. Once the base is written it may have users of its own: then one of them signs
     in, and the sign-in leads to the report. -->
<script lang="ts">
	import FolderOpen from '@lucide/svelte/icons/folder-open';
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import type { FolderCheck } from '$api/FolderCheck';
	import type { ImportJob } from '$api/ImportJob';
	import { ApiError, errorMessage } from '#lib/api/client.js';
	import { checkLegacyFolder, currentLegacyImport, startLegacyImport } from '#lib/api/import.js';
	import { formatBytes } from '#lib/format/bytes.js';
	import { groupDigits } from '#lib/format/digits.js';
	import { ru } from '#lib/i18n/ru.js';
	import { reportRoute } from '#lib/import/report.js';
	import { signInForReport } from '#lib/import/signin.js';
	import { pickFolder } from '#lib/platform/folder.js';
	import { isTauri } from '#lib/platform/tauri.js';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import ConfirmDialog from '#lib/ui/ConfirmDialog.svelte';
	import KeyValue from '#lib/ui/KeyValue.svelte';
	import TextInput from '#lib/ui/TextInput.svelte';
	import ImportProgress from './ImportProgress.svelte';
	import { followImport, type Followed } from './progress.js';

	let {
		replace = false,
		resume = null,
		onback
	}: {
		/** Over the data of the program (only SQLite): a confirmation first. */
		replace?: boolean;
		resume?: ImportJob | null;
		onback?: () => void;
	} = $props();

	const t = ru.legacy;
	const uid = $props.id();
	const picker = isTauri();

	let path = $state('');
	let pathError = $state<string | undefined>();
	let checking = $state(false);
	let found = $state.raw<FolderCheck | null>(null);
	/** The path `found` is about: the transfer goes from it, whatever the field holds by then. */
	let checkedPath = '';
	let failure = $state<string | null>(null);
	let starting = $state(false);
	let confirmOpen = $state(false);
	/** The import on the screen; null while the folder is chosen. */
	let job = $state.raw<ImportJob | null>(null);
	let followed = $state.raw<Followed | null>(null);
	let notice = $state<string | null>(null);
	let stop: (() => void) | null = null;
	/** The replacement chosen here, after the server said the base is not empty. */
	let chosenReplace = $state(false);
	const replacing = $derived(replace || chosenReplace);

	/** The words of the server's time limit (`ApiError::timed_out`): other 503 have their own. */
	const SERVER_TIMEOUT = 'Сервер не успел обработать запрос, повторите';
	/** No answer to the request (cut by the webview, the connection) or the server's time limit. */
	const cutShort = (err: unknown) =>
		err instanceof ApiError &&
		(err.code === 'network' || (err.code === 'unavailable' && err.message === SERVER_TIMEOUT));

	function follow(from: ImportJob) {
		stop?.();
		job = from;
		followed = { kind: 'running', job: from, offline: false };
		stop = followImport(currentLegacyImport, onstate);
	}

	function onstate(state: Followed) {
		followed = state;
		if (state.kind === 'running') job = state.job;
		else if (state.kind === 'done') void goto(resolve(reportRoute(state.importId)));
		// The server was restarted: how it ended is in the past imports.
		else if (state.kind === 'gone') void goto(resolve(reportRoute('latest')));
	}

	onMount(() => {
		if (resume) follow(resume);
		return () => stop?.();
	});

	function edited() {
		found = null;
		failure = null;
		pathError = undefined;
	}

	/** Another folder: the replacement chosen for the old one is not chosen for it. */
	function pathChanged() {
		chosenReplace = false;
		edited();
	}

	async function pick() {
		const chosen = await pickFolder();
		if (chosen === null) return;
		path = chosen;
		pathChanged();
	}

	async function check(e: SubmitEvent) {
		e.preventDefault();
		if (checking) return;
		const p = path.trim();
		edited();
		if (p === '') {
			pathError = t.folderRequired;
			return;
		}
		await runCheck(p);
	}

	async function runCheck(p: string) {
		checking = true;
		try {
			found = await checkLegacyFolder({ path: p, replace: replacing });
			checkedPath = p;
		} catch (err) {
			// The check reads the whole folder; a webview may cut a request that is silent for a minute.
			failure = cutShort(err) ? t.checkCut : errorMessage(err);
		} finally {
			checking = false;
		}
	}

	/** The base has data: the same folder again, for the replacement. */
	function chooseReplace() {
		chosenReplace = true;
		const p = checkedPath;
		edited();
		void runCheck(p);
	}

	async function start() {
		if (!found?.ok || starting) return;
		starting = true;
		failure = null;
		try {
			follow(await startLegacyImport({ path: checkedPath, replace: replacing }));
		} catch (err) {
			if (err instanceof ApiError && err.status === 409) {
				// Another window started one: this one shows its progress.
				notice = t.running;
				try {
					const running = await currentLegacyImport();
					if (running) follow(running);
				} catch (e) {
					failure = errorMessage(e);
				}
			} else failure = errorMessage(err);
		} finally {
			starting = false;
		}
	}

	function signIn() {
		stop?.();
		signInForReport();
	}

	function again() {
		stop?.();
		job = null;
		followed = null;
		found = null;
		notice = null;
	}

	const foundItems = $derived(
		found?.ok
			? [
					{ label: t.schema, value: found.schema_version ?? '—' },
					{ label: t.orders, value: groupDigits(String(found.orders)) },
					{ label: t.clients, value: groupDigits(String(found.clients)) },
					{
						label: t.files,
						value: t.filesValue(groupDigits(String(found.files)), formatBytes(found.files_bytes))
					}
				]
			: []
	);
</script>

{#if job === null}
	<form class="wizard" novalidate onsubmit={check}>
		<TextInput
			label={t.folder}
			hint={t.folderHint}
			bind:value={path}
			error={pathError}
			oninput={pathChanged}
			autocomplete="off"
			autofocus
		/>
		<div class="actions">
			{#if onback}<Button onclick={onback}>{ru.setup.back}</Button>{/if}
			{#if picker}<Button icon={FolderOpen} onclick={() => void pick()}>{t.pick}</Button>{/if}
			<Button type="submit" variant={found?.ok ? 'secondary' : 'primary'} busy={checking}
				>{t.check}</Button
			>
		</div>
	</form>
	{#if failure}<Callout tone="danger">{failure}</Callout>{/if}
	{#if found && !found.ok && found.replace_required}
		<Callout tone="warning" title={t.hasData}>
			{found.problem}
			{#snippet actions()}
				<Button busy={checking} onclick={chooseReplace}>{t.replaceOffer}</Button>
			{/snippet}
		</Callout>
	{:else if found && !found.ok}
		<Callout tone="danger" title={t.cannot}>{found.problem}</Callout>
	{:else if found}
		<section class="found" aria-labelledby="{uid}-found">
			<h2 id="{uid}-found">{t.found}</h2>
			<KeyValue items={foundItems} />
			<div class="actions">
				<Button
					variant="primary"
					busy={starting}
					onclick={() => (replacing ? (confirmOpen = true) : void start())}>{t.start}</Button
				>
			</div>
		</section>
	{/if}
{:else}
	<div class="wizard">
		{#if notice}<Callout tone="info">{notice}</Callout>{/if}
		<ImportProgress
			{job}
			written={followed?.kind === 'signin'}
			failed={followed?.kind === 'failed'}
		/>
		{#if followed?.kind === 'running' && followed.offline}<p class="muted">{t.waiting}</p>{/if}
		{#if followed?.kind === 'failed'}
			<Callout tone="danger" title={t.failed}>{followed.message}</Callout>
			<div class="actions">
				{#if followed.importId !== null}
					<Button href={resolve(reportRoute(followed.importId))}>{t.openReport}</Button>
				{/if}
				<Button variant="primary" onclick={again}>{t.tryAgain}</Button>
			</div>
		{:else if followed?.kind === 'signin'}
			<Callout tone="info">{t.signIn}</Callout>
			<div class="actions">
				<Button variant="primary" onclick={signIn}>{t.signInButton}</Button>
			</div>
		{:else if followed?.kind === 'forbidden'}
			<Callout tone="danger">{followed.message}</Callout>
		{/if}
	</div>
{/if}

{#if replacing}
	<ConfirmDialog
		bind:open={confirmOpen}
		title={t.replaceTitle}
		description={t.replaceText(found?.backups_dir ?? null)}
		confirmLabel={t.replace}
		onconfirm={start}
	/>
{/if}

<style>
	.wizard,
	.found {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}
	.found h2 {
		margin: 0;
		font-size: var(--text-lg);
		font-weight: 600;
	}
	.actions {
		display: flex;
		flex-wrap: wrap;
		justify-content: flex-end;
		gap: var(--space-2);
	}
	.muted {
		margin: 0;
		color: var(--text-3);
		font-size: var(--text-md);
	}
</style>
