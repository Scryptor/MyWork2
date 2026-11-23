<!-- The firm's logo of «Документы и реквизиты»: the picture, and with the right to change the settings «Загрузить…»
     and «Удалить» (after a confirmation). The server takes PNG, JPEG and WebP and says in Russian why not. -->
<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import type { FirmLogo } from '$api/FirmLogo';
	import { getLogoInfo, logoUrl, releaseLogoUrl, removeLogo, uploadLogo } from '#lib/api/logo.js';
	import { errorMessage } from '#lib/api/client.js';
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import ConfirmDialog from '#lib/ui/ConfirmDialog.svelte';
	import Spinner from '#lib/ui/Spinner.svelte';

	let { canEdit }: { canEdit: boolean } = $props();

	const t = ru.settings;
	/** What is known of the logo; undefined while it is read. */
	let info = $state<FirmLogo | null | undefined>(undefined);
	let url = $state<string | null>(null);
	let busy = $state(false);
	let error = $state<string | undefined>();
	let confirming = $state(false);
	let input = $state<HTMLInputElement | null>(null);
	/** False once the screen is gone: an address that comes after it is let go at once. */
	let alive = true;

	async function load() {
		try {
			const next = await getLogoInfo();
			const address = next === null ? null : await logoUrl();
			if (!alive) {
				if (address !== null) releaseLogoUrl(address);
				return;
			}
			info = next;
			url = address;
			// The logo went between the two requests.
			if (next !== null && address === null) info = null;
		} catch (e) {
			if (alive) error = errorMessage(e);
		}
	}

	onMount(() => {
		void load();
	});
	onDestroy(() => {
		alive = false;
		// Its own address only: one made since for another screen stays.
		if (url !== null) releaseLogoUrl(url);
	});

	async function choose(event: Event) {
		const target = event.currentTarget as HTMLInputElement;
		const file = target.files?.[0];
		target.value = '';
		if (!file) return;
		busy = true;
		error = undefined;
		try {
			await uploadLogo(file);
			await load();
		} catch (e) {
			error = errorMessage(e);
		} finally {
			busy = false;
		}
	}

	async function remove() {
		error = undefined;
		try {
			await removeLogo();
			releaseLogoUrl();
			url = null;
			info = null;
		} catch (e) {
			error = errorMessage(e);
		}
	}
</script>

<div class="logo">
	<h2 class="subtitle">{t.logo}</h2>
	<p class="note">{t.logoNote}</p>
	{#if info === undefined}
		<Spinner />
	{:else if info !== null && url !== null}
		<div class="picture"><img src={url} alt={t.logoAlt} /></div>
		<p class="note">{t.logoSize(info.width, info.height)}</p>
	{:else}
		<p class="none">{t.logoNone}</p>
	{/if}
	{#if error}<Callout tone="danger">{error}</Callout>{/if}
	{#if canEdit}
		<div class="buttons">
			<input
				bind:this={input}
				type="file"
				hidden
				aria-label={t.logoFile}
				accept="image/png,image/jpeg,image/webp"
				onchange={choose}
			/>
			<Button size="sm" {busy} onclick={() => input?.click()}
				>{info ? t.logoReplace : t.logoUpload}</Button
			>
			{#if info}
				<Button size="sm" variant="danger" disabled={busy} onclick={() => (confirming = true)}
					>{t.logoRemove}</Button
				>
			{/if}
		</div>
	{/if}
</div>

<ConfirmDialog
	bind:open={confirming}
	title={t.logoRemoveTitle}
	description={t.logoRemoveText}
	confirmLabel={t.logoRemove}
	onconfirm={remove}
/>

<style>
	.logo {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}
	.subtitle {
		margin: 0;
		font-size: var(--text-lg);
		font-weight: 600;
	}
	.note,
	.none {
		margin: 0;
		color: var(--text-3);
		font-size: var(--text-sm);
	}
	.picture {
		align-self: flex-start;
		padding: var(--space-2);
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
		background: var(--surface-2);
	}
	.picture img {
		display: block;
		max-width: 100%;
		max-height: 8rem;
	}
	.buttons {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
	}
</style>
