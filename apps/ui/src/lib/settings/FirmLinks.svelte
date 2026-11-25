<!-- The list of the firm's links printed as QR codes (plan 8.3): the built-in ones can be switched off and
     renamed, an own one can also be deleted. The list goes to the server whole, as it is edited. -->
<script lang="ts">
	import Plus from '@lucide/svelte/icons/plus';
	import Trash2 from '@lucide/svelte/icons/trash-2';
	import type { FirmLink } from '$api/FirmLink';
	import { app } from '#lib/app/context.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import Switch from '#lib/ui/Switch.svelte';
	import TextInput from '#lib/ui/TextInput.svelte';
	import { isBuiltin, MAX_LINKS, newLinkId, newLinkName } from './firm-links.js';
	import type { SettingsEditor } from './settings-editor.svelte.js';

	let { editor }: { editor: SettingsEditor } = $props();

	const t = ru.settings;
	const locked = $derived(!editor.canEdit);
	const links = $derived(editor.value('firm_links', app.settings.firm_links) ?? []);

	/** Changes one link: a text goes after a pause, a switch at once. */
	function change(i: number, patch: Partial<FirmLink>, now: boolean) {
		const next = links.map((l, k) => (k === i ? { ...l, ...patch } : l));
		if (now) editor.commit('firm_links', next);
		else editor.edit('firm_links', next);
	}
	function add() {
		const next = [
			...links,
			{
				id: newLinkId(links),
				name: newLinkName(links, t.linkNewName),
				url: '',
				enabled: true
			}
		];
		editor.commit('firm_links', next);
	}
	function remove(i: number) {
		editor.commit(
			'firm_links',
			links.filter((_, k) => k !== i)
		);
	}
</script>

<ul class="links" aria-label={t.qrLinks}>
	{#each links as link, i (link.id)}
		<li role="group" aria-label={link.name}>
			<div class="head">
				<Switch
					label={t.linkEnabled}
					disabled={locked}
					bind:checked={() => link.enabled, (v) => change(i, { enabled: v }, true)}
				/>
				{#if !isBuiltin(link.id) && !locked}
					<IconButton size="sm" label={t.linkRemove} icon={Trash2} onclick={() => remove(i)} />
				{/if}
			</div>
			<div class="pair">
				<TextInput
					label={t.linkName}
					disabled={locked}
					error={editor.fieldError(`firm_links.${i}.name`)}
					bind:value={() => link.name, (v) => change(i, { name: v }, false)}
					onblur={() => void editor.flush()}
				/>
				<TextInput
					label={t.linkUrl}
					placeholder={t.linkUrlPlaceholder}
					inputmode="url"
					disabled={locked}
					error={editor.fieldError(`firm_links.${i}.url`)}
					bind:value={() => link.url, (v) => change(i, { url: v }, false)}
					onblur={() => void editor.flush()}
				/>
			</div>
		</li>
	{/each}
</ul>
{#if editor.fieldError('firm_links')}
	<p class="error" role="alert">{editor.fieldError('firm_links')}</p>
{/if}
{#if !locked}
	<div>
		<Button size="sm" icon={Plus} disabled={links.length >= MAX_LINKS} onclick={add}
			>{t.linkAdd}</Button
		>
	</div>
{/if}

<style>
	.links {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
		margin: 0;
		padding: 0;
		list-style: none;
	}
	.links li {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		padding-top: var(--space-3);
		border-top: 1px solid var(--border);
	}
	.links li:first-child {
		padding-top: 0;
		border-top: 0;
	}
	.head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
	}
	.pair {
		display: grid;
		grid-template-columns: 1fr 2fr;
		gap: var(--space-3);
	}
	.error {
		margin: 0;
		color: var(--danger);
		font-size: var(--text-sm);
	}
</style>
