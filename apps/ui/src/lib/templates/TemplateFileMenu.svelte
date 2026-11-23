<!-- «Файл» in the top line of the editor: «Сохранить в файл…» gives the saved template as a `.mwtemplate`
     file, «Загрузить из файла…» takes one the user picks. A file that is taken is an edit that can be
     undone; one that is not leaves the template as it is and says why. -->
<script lang="ts">
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import Download from '@lucide/svelte/icons/download';
	import FileIcon from '@lucide/svelte/icons/file';
	import History from '@lucide/svelte/icons/history';
	import Upload from '@lucide/svelte/icons/upload';
	import { errorMessage } from '#lib/api/client.js';
	import { exportTemplateFile, legacyTemplate } from '#lib/api/document-templates.js';
	import { ru } from '#lib/i18n/ru.js';
	import { downloadBlob } from '#lib/platform/download.js';
	import Button from '#lib/ui/Button.svelte';
	import DropdownMenu, { type MenuItem } from '#lib/ui/DropdownMenu.svelte';
	import { toast } from '#lib/ui/toast/toast.svelte.js';
	import type { TemplateEditor } from './editor.svelte.js';
	import LegacyTemplateDialog from './LegacyTemplateDialog.svelte';
	import { readTemplateFile } from './file.js';
	import { insertTextBlock } from './ops.js';

	let {
		editor,
		onmisfit
	}: {
		editor: TemplateEditor;
		/** The texts of what the file lacks for this document; null when the file was taken. */
		onmisfit: (messages: string[] | null) => void;
	} = $props();

	const t = ru.templates;
	let input = $state<HTMLInputElement | null>(null);

	async function save() {
		try {
			// The file is what the server has: the edits still waiting are sent first.
			if (!(await editor.flush())) return;
			const { blob, filename } = await exportTemplateFile(editor.kind);
			downloadBlob(filename, blob);
			toast.show({ message: t.fileSaved(filename), tone: 'success' });
		} catch (e) {
			toast.show({ message: errorMessage(e), tone: 'danger' });
		}
	}

	async function load(e: Event & { currentTarget: HTMLInputElement }) {
		const file = e.currentTarget.files?.[0];
		// The same file may be picked again.
		e.currentTarget.value = '';
		if (!file) return;
		try {
			const read = await readTemplateFile(editor.kind, await file.text());
			if (read.ok) {
				onmisfit(null);
				editor.edit(read.template);
				toast.show({ message: t.fileLoaded, tone: 'success' });
			} else if (read.problem === 'misfit') {
				onmisfit(read.messages);
			} else {
				onmisfit(null);
				toast.show({
					message: read.problem === 'newer' ? t.newerFile : t.notTemplateFile,
					tone: 'danger'
				});
			}
		} catch (err) {
			toast.show({ message: errorMessage(err), tone: 'danger' });
		}
	}

	// The text of the old act of this kind, asked once when the editor opens for it; the item is there
	// only when the import left a text.
	let legacy = $state.raw<string | null>(null);
	let legacyOpen = $state(false);
	$effect(() => {
		const kind = editor.kind;
		let current = true;
		legacy = null;
		legacyTemplate(kind).then(
			(found) => {
				if (current) legacy = found?.text ?? null;
			},
			() => {
				// Without the answer there is no item: the rest of the menu works as it is.
			}
		);
		return () => {
			current = false;
		};
	});

	/** One step of the history: after the chosen block, or at the end. */
	function insertLegacy(text: string) {
		const { template, at } = insertTextBlock(editor.template, editor.selected, text);
		editor.edit(template);
		editor.select(at);
		toast.show({ message: t.legacyInserted, tone: 'success' });
	}

	const items = $derived<MenuItem[]>([
		{ label: t.saveFile, icon: Download, onselect: () => void save() },
		{ label: t.loadFile, icon: Upload, onselect: () => input?.click() },
		...(legacy === null
			? []
			: [{ label: t.legacyMenu, icon: History, onselect: () => (legacyOpen = true) }])
	]);
</script>

<DropdownMenu {items} label={t.file}>
	{#snippet trigger(props)}
		<Button {...props} icon={FileIcon} iconEnd={ChevronDown}>{t.file}</Button>
	{/snippet}
</DropdownMenu>
<input
	bind:this={input}
	type="file"
	accept=".mwtemplate,application/json"
	hidden
	tabindex="-1"
	aria-hidden="true"
	onchange={load}
/>
{#if legacy !== null}
	<LegacyTemplateDialog bind:open={legacyOpen} text={legacy} oninsert={insertLegacy} />
{/if}
