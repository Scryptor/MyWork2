<!-- The editor of a document's template (plan 7.1, decision 11): out of the settings' layout, which has no
     room for three columns. Without «settings_manage» it only says who may change templates. Leaving with
     own changes not saved waits for them, as an order's card does; a conflict still undecided is asked
     about. -->
<script lang="ts">
	import { untrack } from 'svelte';
	import { beforeNavigate, goto } from '$app/navigation';
	import { app } from '#lib/app/context.svelte.js';
	import { isSignIn } from '#lib/app/login.js';
	import { ru } from '#lib/i18n/ru.js';
	import { documentName } from '#lib/print/print.svelte.js';
	import {
		closeTemplateEditor,
		createTemplateEditor,
		unsavedText,
		type TemplateEditor
	} from '#lib/templates/editor.svelte.js';
	import Editor from '#lib/templates/TemplateEditor.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import ConfirmDialog from '#lib/ui/ConfirmDialog.svelte';
	import { toast } from '#lib/ui/toast/toast.svelte.js';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	const t = ru.templates;
	const canEdit = $derived(app.can('settings_manage'));

	let editor = $state.raw<TemplateEditor | null>(null);
	// One editor per template shown: another document (or the screen left) saves the old one's changes.
	$effect(() => {
		if (!canEdit) return;
		const initial = data.template;
		const ed = untrack(() => createTemplateEditor(initial));
		editor = ed;
		return () => {
			if (editor === ed) editor = null;
			closeTemplateEditor(ed);
		};
	});

	// «Шаблон изменили на другом компьютере» left undecided: own changes go only when asked.
	let asking = $state(false);
	let answer: ((leave: boolean) => void) | null = null;
	function ask(): Promise<boolean> {
		answer?.(false);
		asking = true;
		return new Promise((resolve) => (answer = resolve));
	}
	function reply(leave: boolean) {
		const a = answer;
		answer = null;
		asking = false;
		a?.(leave);
	}

	/** Own changes saved before the editor is left; false: stay (the toast or the question says why). */
	async function saved(ed: TemplateEditor): Promise<boolean> {
		if (!ed.unsaved) return true;
		const ok = await ed.leave(ask);
		if (!ok && editor === ed && ed.conflict === null)
			toast.show({ message: unsavedText(ed), tone: 'danger' });
		return ok;
	}

	// Any way out (a link, the document chosen above, the browser's «Назад») waits for the save, then goes
	// on. Closing or reloading the tab gets the browser's own question; the sign-in page is not held.
	let resume: (() => void) | null = null;
	beforeNavigate((nav) => {
		const ed = editor;
		if (!ed?.unsaved || (nav.to && isSignIn(nav.to.url))) return;
		nav.cancel();
		if (nav.willUnload || !nav.to) return;
		const { url } = nav.to;
		const delta = nav.type === 'popstate' ? nav.delta : 0;
		const waiting = resume !== null;
		resume =
			delta !== 0
				? () => history.go(delta)
				: // eslint-disable-next-line svelte/no-navigation-without-resolve -- where SvelteKit was going
					() => void goto(url);
		if (waiting) return;
		void saved(ed).then((ok) => {
			const go = resume;
			resume = null;
			if (ok && editor === ed) go?.();
		});
	});
</script>

<svelte:head><title>{t.title} · {documentName(data.template.kind)}</title></svelte:head>

{#if !canEdit}
	<div class="locked">
		<h1>{t.title}</h1>
		<Callout tone="info">{t.noRight(ru.permissions.settings_manage.label)}</Callout>
	</div>
{:else if editor}
	{#key editor}
		<Editor {editor} />
	{/key}
{/if}

<ConfirmDialog
	bind:open={() => asking, (v) => !v && reply(false)}
	title={t.conflictTitle}
	description={t.disputeText}
	cancelLabel={ru.order.disputeStay}
	confirmLabel={ru.order.disputeLeave}
	onconfirm={() => reply(true)}
/>

<style>
	.locked {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
		max-width: 40rem;
		padding: var(--space-4) var(--space-6);
	}
	h1 {
		margin: 0;
		font-size: var(--text-xl);
		font-weight: 600;
	}
</style>
