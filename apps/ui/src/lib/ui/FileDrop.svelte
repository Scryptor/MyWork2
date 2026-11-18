<!-- A zone for files: drop them here, paste images from the clipboard, or «Выбрать файлы». -->
<script lang="ts">
	import Upload from '@lucide/svelte/icons/upload';
	import type { Snippet } from 'svelte';
	import { isEditableTarget } from '#lib/design/hotkeys.svelte.js';
	import { layers } from '#lib/design/layers.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import { acceptsFile } from './accept.js';
	import Button from './Button.svelte';
	import Kbd from './Kbd.svelte';

	let {
		accept = 'image/*',
		multiple = true,
		label = ru.media.files,
		showChoose = true,
		disabled = false,
		onfiles,
		children
	}: {
		accept?: string;
		multiple?: boolean;
		/** Accessible name of the zone. */
		label?: string;
		/** The «Выбрать файлы» button of the zone itself; off when the parent calls `open()`. */
		showChoose?: boolean;
		/** Takes no files: no drop, no paste, the button is off. */
		disabled?: boolean;
		onfiles: (files: File[]) => void;
		children?: Snippet;
	} = $props();

	let input = $state<HTMLInputElement | null>(null);
	let zone = $state<HTMLElement | null>(null);
	/** dragenter and dragleave also come from the zone's children: count them. */
	let depth = $state(0);

	/** Opens the file chooser, e.g. from a «＋» tile inside the zone. */
	export function open(): void {
		if (!disabled) input?.click();
	}

	/** Reports the files that fit `accept` (only the first one without `multiple`); false if none did. */
	function take(list: ArrayLike<File> | null | undefined): boolean {
		const fit = Array.from(list ?? []).filter((f) => acceptsFile(accept, f));
		const files = multiple ? fit : fit.slice(0, 1);
		if (files.length === 0) return false;
		onfiles(files);
		return true;
	}

	const carriesFiles = (e: DragEvent) => Array.from(e.dataTransfer?.types ?? []).includes('Files');
	/** A zone under a sheet or a dialog leaves paste and drop to the layer on top. */
	const takes = () => !disabled && zone !== null && layers.takesInput(zone);

	// A drop that is not taken is still prevented: the browser would open the file in place of the page.
	function ondragenter(e: DragEvent) {
		if (!carriesFiles(e)) return;
		e.preventDefault();
		if (takes()) depth++;
	}
	function ondragover(e: DragEvent) {
		if (!carriesFiles(e)) return;
		e.preventDefault();
		if (e.dataTransfer) e.dataTransfer.dropEffect = takes() ? 'copy' : 'none';
	}
	function ondragleave(e: DragEvent) {
		if (carriesFiles(e)) depth = Math.max(0, depth - 1);
	}
	function ondrop(e: DragEvent) {
		e.preventDefault();
		depth = 0;
		if (takes()) take(e.dataTransfer?.files);
	}

	// A paste anywhere on the page takes images from the clipboard; a paste with text into a field
	// stays a text paste.
	$effect(() => {
		function onpaste(e: ClipboardEvent) {
			const data = e.clipboardData;
			if (e.defaultPrevented || !data || data.files.length === 0 || !takes()) return;
			if (isEditableTarget(e.target) && Array.from(data.types).includes('text/plain')) return;
			if (take(data.files)) e.preventDefault();
		}
		window.addEventListener('paste', onpaste);
		return () => window.removeEventListener('paste', onpaste);
	});
</script>

<div
	bind:this={zone}
	class="drop"
	class:over={depth > 0}
	data-over={depth > 0 ? '' : undefined}
	role="group"
	aria-label={label}
	{ondragenter}
	{ondragover}
	{ondragleave}
	{ondrop}
>
	{#if children}
		{@render children()}
	{:else}
		<p class="hint">{ru.media.dropHint} <Kbd shortcut="Mod+V" /></p>
	{/if}
	{#if showChoose}
		<div class="actions">
			<Button size="sm" icon={Upload} {disabled} onclick={open}>{ru.media.chooseFiles}</Button>
		</div>
	{/if}
	<input
		bind:this={input}
		type="file"
		hidden
		{accept}
		{multiple}
		onchange={(e) => {
			take(e.currentTarget.files);
			e.currentTarget.value = '';
		}}
	/>
</div>

<style>
	.drop {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		box-sizing: border-box;
		min-width: 0;
		padding: var(--space-3);
		border: 1px dashed var(--border-strong);
		border-radius: var(--radius-lg);
		background: var(--surface);
		transition:
			background var(--duration-fast),
			border-color var(--duration-fast);
	}
	.drop.over {
		border-style: solid;
		border-color: var(--accent);
		background: var(--accent-soft);
	}
	.hint {
		margin: 0;
		color: var(--text-3);
		font-size: var(--text-sm);
	}
	.actions {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
	}
</style>
