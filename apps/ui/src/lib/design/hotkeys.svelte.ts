import { layers } from './layers.svelte.js';

export function isMac(): boolean {
	return typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.platform);
}

interface Spec {
	mod: boolean;
	ctrl: boolean;
	alt: boolean;
	shift: boolean;
	key: string;
}

function parse(spec: string): Spec {
	const parts = spec.split('+');
	const key = parts.pop() ?? '';
	const has = (n: string) => parts.includes(n);
	return { mod: has('Mod'), ctrl: has('Ctrl'), alt: has('Alt'), shift: has('Shift'), key };
}

const PC_NAMES: Record<string, string> = {
	Escape: 'Esc',
	ArrowUp: '↑',
	ArrowDown: '↓',
	ArrowLeft: '←',
	ArrowRight: '→'
};
const MAC_NAMES: Record<string, string> = { ...PC_NAMES, Enter: '↩', Backspace: '⌫' };

export function formatShortcut(spec: string, opts?: { mac?: boolean }): string {
	const mac = opts?.mac ?? isMac();
	const s = parse(spec);
	const out: string[] = [];
	if (s.ctrl) out.push(mac ? '⌃' : 'Ctrl');
	if (s.mod) out.push(mac ? '⌘' : 'Ctrl');
	if (s.alt) out.push(mac ? '⌥' : 'Alt');
	if (s.shift) out.push(mac ? '⇧' : 'Shift');
	out.push((mac ? MAC_NAMES : PC_NAMES)[s.key] ?? s.key);
	return out.join(mac ? '' : ' ');
}

export function matchShortcut(e: KeyboardEvent, spec: string, opts?: { mac?: boolean }): boolean {
	const mac = opts?.mac ?? isMac();
	const s = parse(spec);
	const wantMeta = s.mod && mac;
	const wantCtrl = s.ctrl || (s.mod && !mac);
	if (!!e.metaKey !== wantMeta || !!e.ctrlKey !== wantCtrl || !!e.altKey !== s.alt) return false;

	if (s.key === '?') return e.key === '?'; // Shift is part of the character
	if (!!e.shiftKey !== s.shift) return false;
	if (s.key === '/') return e.key === '/' || e.code === 'Slash';
	if (/^[A-Za-z]$/.test(s.key)) return e.code === `Key${s.key.toUpperCase()}`;
	if (/^\d$/.test(s.key)) return e.code === `Digit${s.key}`;
	return e.key === s.key;
}

const NON_TEXT_INPUTS = new Set([
	'button',
	'checkbox',
	'color',
	'file',
	'image',
	'radio',
	'range',
	'reset',
	'submit'
]);

export function isEditableTarget(target: EventTarget | null): boolean {
	if (!(target instanceof Element)) return false;
	if (target instanceof HTMLInputElement) return !NON_TEXT_INPUTS.has(target.type);
	if (target instanceof HTMLTextAreaElement || target instanceof HTMLSelectElement) return true;
	const editable = target.closest('[contenteditable]');
	return editable !== null && editable.getAttribute('contenteditable') !== 'false';
}

interface Registration {
	spec: string;
	handler: (e: KeyboardEvent) => void;
	allowInFields: boolean;
	allowWithOverlay: boolean;
}

let registrations: Registration[] = [];
let listening = false;

function onKeydown(e: KeyboardEvent) {
	if (e.isComposing || e.keyCode === 229) return;
	const inField = isEditableTarget(e.target);
	// Newest registration wins.
	for (let i = registrations.length - 1; i >= 0; i--) {
		const r = registrations[i];
		if (!r || !matchShortcut(e, r.spec)) continue;
		if (inField && !r.allowInFields) continue;
		if (layers.blocking && !r.allowWithOverlay) continue;
		e.preventDefault();
		r.handler(e);
		return;
	}
}

export function registerHotkey(
	spec: string,
	handler: (e: KeyboardEvent) => void,
	opts?: { allowInFields?: boolean; allowWithOverlay?: boolean }
): () => void {
	if (!listening) {
		window.addEventListener('keydown', onKeydown);
		listening = true;
	}
	const reg: Registration = {
		spec,
		handler,
		allowInFields: opts?.allowInFields ?? false,
		allowWithOverlay: opts?.allowWithOverlay ?? false
	};
	registrations.push(reg);
	return () => {
		registrations = registrations.filter((r) => r !== reg);
	};
}

/** Tests only: drop every registration and the global listener. */
export function __resetHotkeysForTests(): void {
	registrations = [];
	if (listening) window.removeEventListener('keydown', onKeydown);
	listening = false;
}
