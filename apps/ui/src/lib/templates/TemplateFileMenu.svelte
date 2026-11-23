<!-- «Файл» in the top line of the editor: «Сохранить в файл…» gives the saved template as a `.mwtemplate`
     file, «Загрузить из файла…» takes one the user picks. A file that is taken is an edit that can be
     undone; one that is not leaves the template as it is and says why. -->
<script lang="ts">
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import Download from '@lucide/svelte/icons/download';
	import FileIcon from '@lucide/svelte/icons/file';
	import Upload from '@lucide/svelte/icons/upload';
	import { errorMessage } from '#lib/api/client.js';
	import { exportTemplateFile } from '#lib/api/document-templates.js';
	import { ru } from '#lib/i18n/ru.js';
	import { downloadBlob } from '#lib/platform/download.js';
	import Button from '#lib/ui/Button.svelte';
	import DropdownMenu, { type MenuItem } from '#lib/ui/DropdownMenu.svelte';
	import { toast } from '#lib/ui/toast/toast.svelte.js';
	import type { TemplateEditor } from './editor.svelte.js';
	import { readTemplateFile } from './file.js';

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

	const items: MenuItem[] = [
		{ label: t.saveFile, icon: Download, onselect: () => void save() },
		{ label: t.loadFile, icon: Upload, onselect: () => input?.click() }
	];
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
