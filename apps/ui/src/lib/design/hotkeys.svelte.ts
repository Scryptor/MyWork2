import { isEditableTarget } from './editable.js';
import { layers } from './layers.svelte.js';
import { SCAN_GAP_MS } from './scanner.js';

export { isEditableTarget };

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

interface Registration {
	spec: string;
	handler: (e: KeyboardEvent) => void;
	allowInFields: boolean;
	allowWithOverlay: boolean;
	/** Asked at the keydown: false leaves the key to an older registration. */
	when: () => boolean;
	/** One printable key without modifiers (N, /, ?): it fires once the scan gap has passed. */
	deferred: boolean;
}

let registrations: Registration[] = [];
let listening = false;
/** A one-key hotkey waiting out the scan gap. */
let pending: ReturnType<typeof setTimeout> | undefined;
/** `timeStamp` of the last printable key. */
let lastCharAt = -Infinity;

function isOneKey(spec: string): boolean {
	const s = parse(spec);
	return !s.mod && !s.ctrl && !s.alt && !s.shift && s.key.length === 1;
}

function onKeydown(e: KeyboardEvent) {
	if (e.isComposing || e.keyCode === 229) return;
	const inField = isEditableTarget(e.target);
	// A scanner types too: a printable key within the scan gap of another one is a scan, not a hotkey.
	// The key after a one-key hotkey cancels it; the key before (the end of a scan) drops it. Like the
	// scanner, keys typed in a field do not count.
	let burst = false;
	if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey && !inField) {
		burst = e.timeStamp - lastCharAt <= SCAN_GAP_MS;
		lastCharAt = e.timeStamp;
		clearTimeout(pending);
		pending = undefined;
	}
	// Newest registration wins; fields and layers count as they are at the keydown.
	for (let i = registrations.length - 1; i >= 0; i--) {
		const r = registrations[i];
		if (!r || !matchShortcut(e, r.spec)) continue;
		if (inField && !r.allowInFields) continue;
		if (layers.blocking && !r.allowWithOverlay) continue;
		if (!r.when()) continue;
		e.preventDefault();
		if (!r.deferred) r.handler(e);
		else if (!burst) {
			pending = setTimeout(() => {
				pending = undefined;
				if (registrations.includes(r)) r.handler(e);
			}, SCAN_GAP_MS);
		}
		return;
	}
}

/**
 * A shortcut for the whole page. Outside fields and open popups/modals unless allowed. One printable key
 * without modifiers (N, /, ?) fires SCAN_GAP_MS after the keydown, and not at all inside a scan. While
 * `when` says false the registration is skipped and an older one of the same key gets it.
 */
export function registerHotkey(
	spec: string,
	handler: (e: KeyboardEvent) => void,
	opts?: { allowInFields?: boolean; allowWithOverlay?: boolean; when?: () => boolean }
): () => void {
	if (!listening) {
		window.addEventListener('keydown', onKeydown);
		listening = true;
	}
	const reg: Registration = {
		spec,
		handler,
		allowInFields: opts?.allowInFields ?? false,
		allowWithOverlay: opts?.allowWithOverlay ?? false,
		when: opts?.when ?? (() => true),
		deferred: isOneKey(spec)
	};
	registrations.push(reg);
	return () => {
		registrations = registrations.filter((r) => r !== reg);
	};
}

/**
 * Ctrl/⌘+Enter of the open layer: «Принять», «Выдать», «Сохранить». Works in fields and over other layers;
 * the layer registers it while open and removes it when it closes, the newest one wins.
 */
export function submitShortcut(handler: () => void): () => void {
	return registerHotkey('Mod+Enter', () => handler(), {
		allowInFields: true,
		allowWithOverlay: true
	});
}

/** Tests only: drop every registration and the global listener. */
export function __resetHotkeysForTests(): void {
	registrations = [];
	clearTimeout(pending);
	pending = undefined;
	lastCharAt = -Infinity;
	if (listening) window.removeEventListener('keydown', onKeydown);
	listening = false;
}
