<!-- «Резервные копии»: the daily copy and its settings, the copies in the folder (make now, restore at the next
     start, delete), the answer to the offer of the settings of MyWork 2. The folder is on the disk of the
     server, so the screen works only on this computer (the server says so with 403). Rights: «settings_manage»;
     restoring, deleting and cancelling a restore need «users_manage» too. -->
<script lang="ts">
	import { onDestroy } from 'svelte';
	import type { BackupCopy } from '$api/BackupCopy';
	import type { BackupKind } from '$api/BackupKind';
	import { app } from '#lib/app/context.svelte.js';
	import { formatBytes } from '#lib/format/bytes.js';
	import { formatFull } from '#lib/format/date.js';
	import { countLabel } from '#lib/format/plural.js';
	import { ru } from '#lib/i18n/ru.js';
	import { canControlProgram, openFolder, restartProgram } from '#lib/platform/app.js';
	import { pickFolder } from '#lib/platform/folder.js';
	import { createBackupsScreen } from '#lib/settings/backups.svelte.js';
	import Badge from '#lib/ui/Badge.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import Card from '#lib/ui/Card.svelte';
	import ConfirmDialog from '#lib/ui/ConfirmDialog.svelte';
	import Skeleton from '#lib/ui/Skeleton.svelte';
	import Switch from '#lib/ui/Switch.svelte';
	import TextInput from '#lib/ui/TextInput.svelte';
	import { toast } from '#lib/ui/toast/toast.svelte.js';

	const t = ru.backups;
	const uid = $props.id();
	const screen = createBackupsScreen({ allowed: app.can('settings_manage') });
	/** Restoring, deleting and cancelling a restore need both rights; without them the buttons are not shown. */
	const mayChange = app.can('settings_manage') && app.can('users_manage');
	const inProgram = canControlProgram();

	const o = $derived(screen.overview);
	const tones: Record<BackupKind, 'info' | 'neutral' | 'warning'> = {
		manual: 'info',
		auto: 'neutral',
		before_restore: 'warning'
	};

	let restoring = $state<BackupCopy | null>(null);
	let removing = $state<BackupCopy | null>(null);

	const sizeOf = (c: BackupCopy) =>
		t.size(formatBytes(c.database_bytes + c.files_bytes), countLabel(c.files, t.fileForms));

	async function pick() {
		const folder = await pickFolder();
		if (folder) screen.setFolder(folder);
	}

	async function copyPath() {
		try {
			await navigator.clipboard.writeText(o?.folder ?? '');
			toast.show({ message: t.pathCopied });
		} catch {
			toast.show({ message: t.pathNotCopied, tone: 'danger' });
		}
	}

	async function inProgramDo(run: () => Promise<void>) {
		try {
			await run();
		} catch (e) {
			toast.show({ message: e instanceof Error ? e.message : ru.platform.failed, tone: 'danger' });
		}
	}

	onDestroy(() => screen.dispose());
</script>

<h1 class="title">{ru.settings.backups}</h1>

{#if screen.blocked}
	<Callout tone={app.can('settings_manage') ? 'warning' : 'info'}>{screen.blocked}</Callout>
{:else if o === null}
	<Skeleton height="6rem" radius="md" />
{:else}
	{#if o.last_restore}
		{@const last = o.last_restore}
		<Callout tone={last.ok ? 'success' : 'danger'}>
			{last.ok
				? t.restoredOk(formatFull(last.copy.created_at))
				: t.restoredFailed(last.error ?? '')}
			{#snippet actions()}
				{#if mayChange}
					<Button size="sm" variant="ghost" onclick={() => screen.dismissRestore()}>{t.hide}</Button
					>
				{/if}
			{/snippet}
		</Callout>
	{/if}

	{#if o.pending}
		<Callout tone="warning">
			{t.pending(formatFull(o.pending.copy.created_at))}
			{#if !inProgram}<p class="note">{t.restartServer}</p>{/if}
			{#snippet actions()}
				{#if inProgram}
					<Button size="sm" variant="primary" onclick={() => inProgramDo(restartProgram)}>
						{t.restartNow}
					</Button>
				{/if}
				{#if mayChange}
					<Button size="sm" onclick={() => screen.dismissRestore()}>{t.cancelRestore}</Button>
				{/if}
			{/snippet}
		</Callout>
	{/if}

	{#if o.legacy}
		<Callout tone="info">
			{t.legacy(o.legacy.auto, o.legacy.folders)}
			{#snippet actions()}
				<Button
					size="sm"
					variant="primary"
					disabled={screen.working}
					onclick={() => screen.chooseLegacy(true)}>{t.legacyApply}</Button
				>
				<Button size="sm" disabled={screen.working} onclick={() => screen.chooseLegacy(false)}>
					{t.legacyDecline}
				</Button>
			{/snippet}
		</Callout>
	{/if}

	<section aria-labelledby="{uid}-auto">
		<Card>
			<div class="fields">
				<h2 id="{uid}-auto">{t.autoCard}</h2>
				<Switch
					label={t.auto}
					help={ru.help.backups.auto}
					bind:checked={() => screen.auto(), (v) => screen.setAuto(v)}
				/>
				<TextInput
					label={t.keep}
					inputmode="numeric"
					error={screen.fieldError('keep')}
					bind:value={() => screen.keepText(), (v) => screen.typeKeep(v)}
					onblur={() => screen.leave('keep')}
				/>
				{#if inProgram}
					<div class="folder">
						<span class="label">{t.folder}</span>
						<span class="path">{o.folder}</span>
						{#if screen.fieldError('folder')}
							<span class="error" role="alert">{screen.fieldError('folder')}</span>
						{/if}
					</div>
				{:else}
					<TextInput
						label={t.folder}
						help={ru.help.backups.folder}
						error={screen.fieldError('folder')}
						bind:value={() => screen.folderText(), (v) => screen.typeFolder(v)}
						onblur={() => screen.leave('folder')}
					/>
				{/if}
				{#if o.folder_problem}<Callout tone="warning">{o.folder_problem}</Callout>{/if}
				<div class="buttons">
					{#if inProgram}
						<Button onclick={pick}>{t.pick}</Button>
						<Button onclick={() => inProgramDo(() => openFolder(o.folder))}>{t.open}</Button>
					{:else}
						<Button onclick={copyPath}>{t.copyPath}</Button>
					{/if}
					{#if o.settings.folder !== null}
						<Button onclick={() => screen.setFolder(null)}>{t.byDefault}</Button>
					{/if}
				</div>
				{#if screen.saveError}
					<p class="failed" role="status">
						{ru.save.failed}: {screen.saveError.message}
						<Button size="sm" variant="ghost" onclick={() => screen.retrySave()}>
							{ru.common.retry}
						</Button>
					</p>
				{/if}
			</div>
		</Card>
	</section>

	<section aria-labelledby="{uid}-copies">
		<Card>
			<div class="fields">
				<h2 id="{uid}-copies">{t.copiesCard}</h2>
				<div class="buttons">
					<Button variant="primary" busy={screen.making} onclick={() => screen.makeCopy()}>
						{t.makeNow}
					</Button>
				</div>
				{#if o.job?.error}<Callout tone="danger">{o.job.error}</Callout>{/if}
				{#if o.job?.note}<Callout tone="warning">{o.job.note}</Callout>{/if}
				{#if o.copies.length === 0}
					<p class="hint">{t.none}</p>
				{:else}
					<ul class="copies" aria-label={t.copiesCard}>
						{#each o.copies as c (c.name)}
							<li class="copy">
								<span class="when">{formatFull(c.created_at)}</span>
								<Badge tone={tones[c.kind]}>{t.kinds[c.kind]}</Badge>
								<span class="size">{sizeOf(c)}</span>
								{#if mayChange}
									<span class="row-actions">
										<Button
											size="sm"
											aria-label={t.restoreLabel(formatFull(c.created_at))}
											onclick={() => (restoring = c)}>{t.restore}</Button
										>
										<Button
											size="sm"
											variant="ghost"
											aria-label={t.removeLabel(formatFull(c.created_at))}
											onclick={() => (removing = c)}>{t.remove}</Button
										>
									</span>
								{/if}
							</li>
						{/each}
					</ul>
				{/if}
			</div>
		</Card>
	</section>
{/if}

{#if restoring}
	{@const target = restoring}
	<ConfirmDialog
		bind:open={() => restoring !== null, (v) => !v && (restoring = null)}
		tone="primary"
		title={t.restoreTitle}
		description={t.restoreText(formatFull(target.created_at))}
		confirmLabel={t.restoreConfirm}
		onconfirm={() => screen.restore(target.name)}
	/>
{/if}
{#if removing}
	{@const target = removing}
	<ConfirmDialog
		bind:open={() => removing !== null, (v) => !v && (removing = null)}
		title={t.removeTitle}
		description={t.removeText(formatFull(target.created_at))}
		confirmLabel={t.removeConfirm}
		onconfirm={() => screen.remove(target.name)}
	/>
{/if}

<style>
	.title {
		margin: 0;
		font-size: var(--text-xl);
		font-weight: 600;
	}
	h2 {
		margin: 0;
		font-size: var(--text-lg);
		font-weight: 600;
	}
	.fields {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}
	.note {
		margin: var(--space-1) 0 0;
	}
	.folder {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
	}
	.label {
		font-size: var(--text-md);
		font-weight: 500;
	}
	.path {
		overflow-wrap: anywhere;
		color: var(--text-2);
		font-size: var(--text-md);
	}
	.error,
	.failed {
		margin: 0;
		color: var(--danger);
		font-size: var(--text-md);
	}
	.buttons {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
	}
	.hint {
		margin: 0;
		color: var(--text-3);
		font-size: var(--text-md);
	}
	.copies {
		display: flex;
		flex-direction: column;
		margin: 0;
		padding: 0;
		list-style: none;
	}
	.copy {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2) var(--space-3);
		padding: var(--space-2) 0;
		border-top: 1px solid var(--border);
		font-size: var(--text-md);
	}
	.when {
		font-weight: 600;
	}
	.size {
		color: var(--text-3);
	}
	.row-actions {
		display: inline-flex;
		gap: var(--space-2);
		margin-inline-start: auto;
	}
</style>
