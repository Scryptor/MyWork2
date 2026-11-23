// The template editor's model (plan 7.1): the template shown, its undo history and the block chosen,
// saved by itself over the shared record editor — one queue of writes, the version, the conflict. The
// history is the truth of the screen: every step, an undo too, goes to the server as the whole model.
import type { Block } from '$api/Block';
import type { CodeDiagnostic } from '$api/CodeDiagnostic';
import type { DocumentKind } from '$api/DocumentKind';
import type { DocumentTemplate } from '$api/DocumentTemplate';
import type { ErrorField } from '$api/ErrorField';
import type { Template } from '$api/Template';
import { ApiError } from '#lib/api/client.js';
import { getTemplate, saveTemplate } from '#lib/api/document-templates.js';
import {
	createEntityEditor,
	type Conflict,
	type SaveStatus
} from '#lib/app/entity-editor.svelte.js';
import { ru } from '#lib/i18n/ru.js';
import { formatMm } from '#lib/print/settings.svelte.js';
import { sameValue } from '#lib/orders/merge.js';
import { toast } from '#lib/ui/toast/toast.svelte.js';
import { createHistory } from './history.svelte.js';
import { blockAt, type BlockPath } from './ops.js';

const t = ru.templates;

export interface TemplateEditorApi {
	getTemplate: (kind: DocumentKind) => Promise<DocumentTemplate>;
	saveTemplate: (
		kind: DocumentKind,
		template: Template,
		version: number
	) => Promise<DocumentTemplate>;
}

export interface TemplateEditorOptions {
	api?: TemplateEditorApi;
	delayMs?: number;
}

/** What is chosen in the list of blocks: the page or a block. */
export type Selection = BlockPath | 'page';

export interface TemplateEditor {
	readonly kind: DocumentKind;
	/** The template as the screen shows it: the current step of the history. */
	readonly template: Template;
	readonly standard: Template;
	readonly isStandard: boolean;
	/** The chosen item; a block that is gone (an undo took it) leaves «Страница». */
	readonly selected: Selection;
	select(s: Selection): void;
	/**
	 * A new model: a step of the history and a write. `mergeKey` (typing in one field) joins the steps
	 * within a second and lets the write wait for a pause; a model equal to the shown one changes nothing.
	 */
	edit(next: Template, mergeKey?: string): void;
	undo(): void;
	redo(): void;
	readonly canUndo: boolean;
	readonly canRedo: boolean;
	/** = `edit(standard)`: an ordinary step, it can be undone. */
	resetToStandard(): void;
	/**
	 * The errors of own Typst code in the newest preview, set by the preview: those of a refused model
	 * (a 422), none after any other answer. Replaced as a whole, never changed in place.
	 */
	diagnostics: readonly CodeDiagnostic[];
	/** For the save indicator; «Отбросить мои правки» there shows the saved template again. */
	readonly status: SaveStatus;
	/** Own model and the one saved elsewhere meanwhile; null without a conflict. */
	readonly conflict: Conflict<Template> | null;
	/** «Оставить моё»: own model goes over the newer version. */
	keepMine(): void;
	/** «Взять новое»: the template saved elsewhere is shown, a step that can be undone. */
	takeTheirs(): void;
	/** Own changes not on the server: waiting, being sent, failed, or disputed. */
	readonly unsaved: boolean;
	flush(): Promise<boolean>;
	leave(ask: () => Promise<boolean>): Promise<boolean>;
	dispose(): void;
}

const DEFAULT_API: TemplateEditorApi = { getTemplate, saveTemplate };

/** A save no retry can help: no right to edit; a model the server does not take until it is fixed. */
function lostText(e: ApiError | null): string | null {
	if (e?.code === 'forbidden') return e.message;
	if (e?.code === 'validation') return t.invalid;
	return null;
}

export function createTemplateEditor(
	initial: DocumentTemplate,
	opts: TemplateEditorOptions = {}
): TemplateEditor {
	const api = opts.api ?? DEFAULT_API;
	const kind = initial.kind;
	const history = createHistory<Template>(initial.template, { limit: 100, mergeMs: 1000 });
	let chosen = $state.raw<Selection>('page');
	let diagnostics = $state.raw<readonly CodeDiagnostic[]>([]);

	/** The answer to the last write of this editor; every other record that arrives was read from the server. */
	let ownAnswer: DocumentTemplate | null = null;

	const ed = createEntityEditor<DocumentTemplate, 'template'>({
		initial,
		fields: ['template'],
		load: () => api.getTemplate(kind),
		save: async (patch, version) => {
			const r = await api.saveTemplate(kind, patch.template!, version);
			ownAnswer = r;
			return r;
		},
		emit: catchUp,
		lostText,
		delayMs: opts.delayMs
	});

	const owns = () => 'template' in ed.autosave.pending;

	/** The model to the server; one equal to the saved template is nothing to send. */
	function send(next: Template, typing: boolean) {
		if (sameValue(next, ed.record.template)) {
			// Back to what the other computer saved: the dispute is over, theirs is ours.
			if (ed.conflicts.has('template')) ed.take('template');
			else {
				ed.autosave.discard('template');
				// A failed write of what is gone is no failure any more: with nothing left to send the
				// retry only clears the error, «Не сохранено» and the reason with it.
				ed.autosave.retry();
			}
			return;
		}
		if (typing) ed.autosave.edit('template', next);
		else ed.autosave.commit('template', next);
	}

	/**
	 * An answer of the server. To a write of this editor: if the model went back to the saved one (an
	 * undo) while the write was on its way, it has landed after all and the model shown goes again; an
	 * answer to a write still own (its value as the server keeps it) changes nothing, the next edit goes
	 * over it. A record read again after a conflict is taken as it is: what is shown follows it, nothing
	 * is written over a version saved elsewhere.
	 */
	function catchUp(r: DocumentTemplate) {
		if (ed.conflicts.size > 0 || owns()) return;
		if (r !== ownAnswer) follow(r.template);
		else if (!sameValue(history.current, r.template))
			ed.autosave.commit('template', history.current);
	}

	/** The model becomes the saved one (taken from elsewhere, own changes dropped): a step to undo. */
	function follow(saved: Template) {
		if (!sameValue(history.current, saved)) history.push(saved);
	}

	function edit(next: Template, mergeKey?: string) {
		if (sameValue(next, history.current)) return;
		history.push(next, mergeKey);
		send(next, mergeKey !== undefined);
	}

	const status: SaveStatus = {
		get saveState() {
			return ed.saveState;
		},
		get lost() {
			return ed.lost;
		},
		get autosave() {
			return ed.autosave;
		},
		drop() {
			ed.drop();
			follow(ed.record.template);
		}
	};

	return {
		kind,
		get template() {
			return history.current;
		},
		get standard() {
			return ed.record.standard;
		},
		get isStandard() {
			return sameValue(history.current, ed.record.standard);
		},
		get selected() {
			return chosen !== 'page' && blockAt(history.current, chosen) === undefined ? 'page' : chosen;
		},
		select(s) {
			chosen = s;
		},
		edit,
		undo() {
			const prev = history.undo();
			if (prev !== undefined) send(prev, false);
		},
		redo() {
			const next = history.redo();
			if (next !== undefined) send(next, false);
		},
		get canUndo() {
			return history.canUndo;
		},
		get canRedo() {
			return history.canRedo;
		},
		resetToStandard() {
			edit(ed.record.standard);
		},
		get diagnostics() {
			return diagnostics;
		},
		set diagnostics(d) {
			diagnostics = d;
		},
		status,
		get conflict() {
			return ed.conflicts.get('template') ?? null;
		},
		keepMine() {
			ed.keep('template');
		},
		takeTheirs() {
			ed.take('template');
			follow(ed.record.template);
		},
		get unsaved() {
			return ed.unsaved;
		},
		flush: () => ed.flush(),
		leave: (ask) => ed.leave(ask),
		dispose: () => ed.dispose()
	};
}

/** Why own changes of the template are not saved: the failed save, or a conflict waiting for a choice. */
export function unsavedText(ed: TemplateEditor): string {
	const reason =
		ed.conflict !== null
			? t.conflictTitle
			: (ed.status.lost ?? ed.status.autosave.error?.message ?? ru.api.failed);
	return t.unsaved(reason);
}

/** An editor no longer shown: what is left is saved, a failure is told, then it stops. */
export function closeTemplateEditor(ed: TemplateEditor): void {
	void ed
		.flush()
		.then((ok) => {
			if (!ok) toast.show({ message: unsavedText(ed), tone: 'danger' });
		})
		.finally(() => ed.dispose());
}

/** The longest start of a text in the list of blocks. */
const LABEL_TEXT = 40;

/** The first line of a text, cut at a word with «…» when it is long. */
function start(text: string): string {
	const line = text.replace(/\s+/g, ' ').trim();
	if (line.length <= LABEL_TEXT) return line;
	const cut = line.slice(0, LABEL_TEXT);
	const word = cut.lastIndexOf(' ');
	return `${(word > 0 ? cut.slice(0, word) : cut).replace(/[\s,.;:—-]+$/, '')}…`;
}

/** A row of the list of blocks: the type and what tells it from others, «Текст · Акт приёма №…». */
export function blockLabel(block: Block): string {
	const b = block.body;
	const type = t.blockTypes[b.type];
	let detail = '';
	if (b.type === 'text') detail = start(b.text);
	else if (b.type === 'fields') detail = start(b.rows.map((r) => r.label).join(', '));
	else if (b.type === 'signatures') detail = start([b.left, b.right].filter(Boolean).join(' / '));
	else if (b.type === 'spacer') detail = `${formatMm(b.mm)} ${t.mmUnit}`;
	else if (b.type === 'typst_code')
		detail = start(
			b.code.split('\n').find((l) => l.trim() !== '' && !l.trim().startsWith('//')) ?? ''
		);
	else if (b.type === 'columns') detail = String(b.columns.length);
	return detail ? `${type} · ${detail}` : type;
}

/** The place of a field error of the server (`blocks.2.columns.1.0.text`) as a path of the model. */
function pathOf(field: string): BlockPath | null {
	const m = /^blocks\.(\d+)(?:\.columns\.(\d+)\.(\d+))?/.exec(field);
	if (!m) return null;
	return m[2] === undefined ? [Number(m[1])] : [Number(m[1]), Number(m[2]), Number(m[3])];
}

/** What Typst says of an error of own code, with its hints: «message — hint — hint». */
export const diagnosticText = (d: CodeDiagnostic): string => [d.message, ...d.hints].join(' — ');

/** An error of own code over the preview: «Typst-код · … · строка 2, знак 5: message — hint». */
function codeErrorLine(d: CodeDiagnostic, template: Template): string {
	if (d.place === null) return diagnosticText(d);
	const block = blockAt(template, d.place.block);
	return t.codeError(
		block ? blockLabel(block) : t.blockTypes.typst_code,
		d.place.line,
		d.place.column,
		diagnosticText(d)
	);
}

/**
 * The mistakes of a model the server did not take (a 422), each with what it is about. The errors of
 * own code come first, each with its line; the fields that only mark their blocks are not repeated.
 */
export function templateErrors(e: unknown, template: Template): string[] {
	if (!(e instanceof ApiError) || e.code !== 'validation') return [];
	const diagnostics = e.body?.diagnostics ?? [];
	const fields = (e.body?.fields ?? []).filter(
		(f) => diagnostics.length === 0 || f.code !== 'typst_error'
	);
	const code = diagnostics.map((d) => codeErrorLine(d, template));
	if (code.length === 0 && fields.length === 0) return [e.message];
	return [...code, ...fieldErrors(e, fields, template)];
}

/** The field errors of a refused model, each with the block or the setting it is about. */
function fieldErrors(e: ApiError, fields: readonly ErrorField[], template: Template): string[] {
	return fields.map((f) => {
		const message = f.message || e.message;
		const path = pathOf(f.field);
		const block = path && blockAt(template, path);
		const where = block
			? blockLabel(block)
			: f.field.startsWith('page')
				? t.page
				: f.field === 'layout'
					? t.layout
					: f.field === 'font_size_pt'
						? t.fontSize
						: null;
		return where ? `${where}: ${message}` : message;
	});
}
