import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
	__resetHotkeysForTests,
	formatShortcut,
	isEditableTarget,
	matchShortcut,
	registerHotkey,
	submitShortcut
} from './hotkeys.svelte.js';
import { layers } from './layers.svelte.js';
import { SCAN_GAP_MS } from './scanner.js';

const ev = (init: Partial<KeyboardEvent>) => init as KeyboardEvent;

afterEach(() => {
	__resetHotkeysForTests();
	document.body.innerHTML = '';
});

describe('formatShortcut', () => {
	it('formats for PC and Mac', () => {
		expect(formatShortcut('Mod+K', { mac: false })).toBe('Ctrl K');
		expect(formatShortcut('Mod+K', { mac: true })).toBe('⌘K');
		expect(formatShortcut('Mod+Enter', { mac: false })).toBe('Ctrl Enter');
		expect(formatShortcut('Mod+Enter', { mac: true })).toBe('⌘↩');
		expect(formatShortcut('Alt+ArrowUp', { mac: false })).toBe('Alt ↑');
		expect(formatShortcut('Alt+ArrowUp', { mac: true })).toBe('⌥↑');
		expect(formatShortcut('Escape', { mac: false })).toBe('Esc');
		expect(formatShortcut('N', { mac: false })).toBe('N');
	});
	it('writes the size keys with + and −', () => {
		expect(formatShortcut('Mod+Plus', { mac: false })).toBe('Ctrl +');
		expect(formatShortcut('Mod+Minus', { mac: true })).toBe('⌘−');
		expect(formatShortcut('Mod+0', { mac: false })).toBe('Ctrl 0');
	});
});

describe('size keys', () => {
	const pc = { mac: false };
	it('Plus: Equal at any Shift, NumpadAdd, only with the modifier', () => {
		expect(matchShortcut(ev({ code: 'Equal', key: '=', ctrlKey: true }), 'Mod+Plus', pc)).toBe(
			true
		);
		expect(
			matchShortcut(ev({ code: 'Equal', key: '+', ctrlKey: true, shiftKey: true }), 'Mod+Plus', pc)
		).toBe(true);
		expect(matchShortcut(ev({ code: 'NumpadAdd', key: '+', ctrlKey: true }), 'Mod+Plus', pc)).toBe(
			true
		);
		expect(
			matchShortcut(ev({ code: 'Equal', key: '=', metaKey: true }), 'Mod+Plus', { mac: true })
		).toBe(true);
		expect(matchShortcut(ev({ code: 'Equal', key: '=' }), 'Mod+Plus', pc)).toBe(false);
	});
	it('Minus: Minus at any Shift, like Plus', () => {
		expect(
			matchShortcut(ev({ code: 'Minus', key: '_', ctrlKey: true, shiftKey: true }), 'Mod+Minus', pc)
		).toBe(true);
		expect(
			matchShortcut(
				ev({ code: 'NumpadSubtract', key: '-', metaKey: true, shiftKey: true }),
				'Mod+Minus',
				{ mac: true }
			)
		).toBe(true);
	});
	it('Minus: Minus, NumpadSubtract', () => {
		expect(matchShortcut(ev({ code: 'Minus', key: '-', ctrlKey: true }), 'Mod+Minus', pc)).toBe(
			true
		);
		expect(
			matchShortcut(ev({ code: 'NumpadSubtract', key: '-', ctrlKey: true }), 'Mod+Minus', pc)
		).toBe(true);
		expect(matchShortcut(ev({ code: 'Minus', key: '-' }), 'Mod+Minus', pc)).toBe(false);
	});
	it('0: Digit0, Numpad0', () => {
		expect(matchShortcut(ev({ code: 'Digit0', key: '0', ctrlKey: true }), 'Mod+0', pc)).toBe(true);
		expect(matchShortcut(ev({ code: 'Numpad0', key: '0', ctrlKey: true }), 'Mod+0', pc)).toBe(true);
		expect(matchShortcut(ev({ code: 'Digit0', key: '0' }), 'Mod+0', pc)).toBe(false);
	});
});

describe('matchShortcut', () => {
	it('letters match by code (any layout)', () => {
		expect(matchShortcut(ev({ key: 'т', code: 'KeyN' }), 'N')).toBe(true);
		expect(matchShortcut(ev({ key: 'n', code: 'KeyN', ctrlKey: true }), 'N')).toBe(false);
	});
	it('Mod is Ctrl or Cmd', () => {
		expect(matchShortcut(ev({ code: 'KeyK', ctrlKey: true }), 'Mod+K', { mac: false })).toBe(true);
		expect(matchShortcut(ev({ code: 'KeyK', metaKey: true }), 'Mod+K', { mac: true })).toBe(true);
		expect(matchShortcut(ev({ code: 'KeyK', metaKey: true }), 'Mod+K', { mac: false })).toBe(false);
	});
	it('slash and question mark', () => {
		expect(matchShortcut(ev({ key: '/', code: 'Slash' }), '/')).toBe(true);
		expect(matchShortcut(ev({ key: '.', code: 'Slash' }), '/')).toBe(true);
		expect(matchShortcut(ev({ key: '?', code: 'Digit7', shiftKey: true }), '?')).toBe(true);
	});
});

describe('isEditableTarget', () => {
	it('detects editable elements', () => {
		document.body.innerHTML = `<input type="text" id="t"><textarea id="a"></textarea>
			<div contenteditable="true" id="c"><span id="s"></span></div>
			<input type="checkbox" id="k"><button id="b"></button>`;
		const q = (id: string) => document.getElementById(id);
		expect(isEditableTarget(q('t'))).toBe(true);
		expect(isEditableTarget(q('a'))).toBe(true);
		expect(isEditableTarget(q('c'))).toBe(true);
		expect(isEditableTarget(q('s'))).toBe(true);
		expect(isEditableTarget(q('k'))).toBe(false);
		expect(isEditableTarget(q('b'))).toBe(false);
		expect(isEditableTarget(null)).toBe(false);
	});
});

const CODES: Record<string, string> = { '/': 'Slash', '?': 'Slash', Enter: 'Enter' };
const codeOf = (key: string) =>
	/^[a-z]$/i.test(key)
		? `Key${key.toUpperCase()}`
		: /^\d$/.test(key)
			? `Digit${key}`
			: (CODES[key] ?? key);

/** A keydown of `key` (letters and digits get their `code`) on `target`, the page by default. */
function press(key: string, target: EventTarget = document.body, init: KeyboardEventInit = {}) {
	const e = new KeyboardEvent('keydown', {
		key,
		code: codeOf(key),
		bubbles: true,
		cancelable: true,
		...init
	});
	target.dispatchEvent(e);
	return e;
}

describe('registerHotkey', () => {
	beforeEach(() => {
		vi.useFakeTimers();
	});
	afterEach(() => {
		vi.useRealTimers();
	});

	it('the scan gap is the scanner’s 35 ms', () => {
		expect(SCAN_GAP_MS).toBe(35);
	});

	it('fires a single-letter hotkey after the scan gap', () => {
		const h = vi.fn();
		registerHotkey('n', h);
		press('n');
		expect(h).not.toHaveBeenCalled();
		vi.advanceTimersByTime(36);
		expect(h).toHaveBeenCalledTimes(1);
	});

	it('drops a single-letter hotkey when another key follows within the gap (a scan)', () => {
		const h = vi.fn();
		registerHotkey('/', h);
		press('/');
		vi.advanceTimersByTime(10);
		press('1');
		vi.advanceTimersByTime(100);
		expect(h).not.toHaveBeenCalled();
	});

	it('drops a single-letter hotkey that ends a burst (the last character of a scan)', () => {
		const h = vi.fn();
		registerHotkey('N', h);
		for (const ch of '12345') {
			press(ch);
			vi.advanceTimersByTime(10);
		}
		press('n');
		press('Enter');
		vi.advanceTimersByTime(100);
		expect(h).not.toHaveBeenCalled();

		// A pause, then N alone: a person.
		press('n');
		vi.advanceTimersByTime(36);
		expect(h).toHaveBeenCalledTimes(1);
	});

	it('keys typed in a field are not a scan: the hotkey right after them fires', () => {
		document.body.innerHTML = '<input id="i">';
		const input = document.getElementById('i')!;
		const h = vi.fn();
		registerHotkey('N', h);
		press('n', input);
		press('n');
		vi.advanceTimersByTime(36);
		expect(h).toHaveBeenCalledTimes(1);
	});

	it('a deferred key is prevented at once: «/» is not typed anywhere', () => {
		const h = vi.fn();
		registerHotkey('/', h);
		const e = press('/');
		expect(e.defaultPrevented).toBe(true);
		expect(h).not.toHaveBeenCalled();
		vi.advanceTimersByTime(36);
		expect(h).toHaveBeenCalledTimes(1);
	});

	it('? and Shift are one key: Shift before ? does not cancel it', () => {
		const h = vi.fn();
		registerHotkey('?', h);
		press('Shift', document.body, { code: 'ShiftLeft', shiftKey: true });
		press('?', document.body, { shiftKey: true });
		vi.advanceTimersByTime(36);
		expect(h).toHaveBeenCalledTimes(1);
	});

	it('a deferred key whose handler is removed within the gap does not fire', () => {
		const h = vi.fn();
		const off = registerHotkey('N', h);
		press('n');
		off();
		vi.advanceTimersByTime(36);
		expect(h).not.toHaveBeenCalled();
	});

	it('keys with modifiers fire at once', () => {
		const h = vi.fn();
		registerHotkey('Mod+K', h, { allowInFields: true });
		press('k', document.body, { ctrlKey: true });
		expect(h).toHaveBeenCalledTimes(1);
	});

	it('mod+Enter fires at once, also inside fields', () => {
		document.body.innerHTML = '<input id="i">';
		const input = document.getElementById('i')!;
		const h = vi.fn();
		const off = submitShortcut(h);
		const e = press('Enter', input, { ctrlKey: true });
		expect(h).toHaveBeenCalledTimes(1);
		expect(e.defaultPrevented).toBe(true);
		press('Enter', input);
		expect(h).toHaveBeenCalledTimes(1);
		off();
		press('Enter', input, { ctrlKey: true });
		expect(h).toHaveBeenCalledTimes(1);
	});

	it('the submit shortcut of an open dialog works over it; the newest one wins', () => {
		const outer = vi.fn();
		const inner = vi.fn();
		submitShortcut(outer);
		const closeModal = layers.open('modal', () => {});
		const off = submitShortcut(inner);
		press('Enter', document.body, { ctrlKey: true });
		expect(inner).toHaveBeenCalledTimes(1);
		expect(outer).not.toHaveBeenCalled();
		off();
		closeModal();
		press('Enter', document.body, { ctrlKey: true });
		expect(outer).toHaveBeenCalledTimes(1);
	});

	it('a registration that does not apply now leaves the key to an older one', () => {
		const panel = vi.fn();
		const card = vi.fn();
		let panelOpen = true;
		registerHotkey('Mod+Enter', panel, { allowInFields: true });
		// The card registers again after the panel: it gives way while the panel is open.
		registerHotkey('Mod+Enter', card, { allowInFields: true, when: () => !panelOpen });
		press('Enter', document.body, { ctrlKey: true });
		expect(panel).toHaveBeenCalledTimes(1);
		expect(card).not.toHaveBeenCalled();
		panelOpen = false;
		press('Enter', document.body, { ctrlKey: true });
		expect(card).toHaveBeenCalledTimes(1);
		expect(panel).toHaveBeenCalledTimes(1);
	});

	it('calls the handler and prevents default', () => {
		const h = vi.fn();
		registerHotkey('N', h);
		const e = press('n');
		vi.advanceTimersByTime(36);
		expect(h).toHaveBeenCalledTimes(1);
		expect(e.defaultPrevented).toBe(true);
	});

	it('ignores events from fields unless allowed', () => {
		document.body.innerHTML = '<input id="i">';
		const input = document.getElementById('i')!;
		const h = vi.fn();
		const off = registerHotkey('N', h);
		const typed = press('n', input);
		vi.advanceTimersByTime(36);
		expect(h).not.toHaveBeenCalled();
		expect(typed.defaultPrevented).toBe(false);
		off();
		const g = vi.fn();
		registerHotkey('N', g, { allowInFields: true });
		press('n', input);
		vi.advanceTimersByTime(36);
		expect(g).toHaveBeenCalledTimes(1);
	});

	it('ignores IME composition', () => {
		const h = vi.fn();
		registerHotkey('N', h);
		press('n', document.body, { isComposing: true });
		vi.advanceTimersByTime(36);
		expect(h).not.toHaveBeenCalled();
	});

	it('stands down while a popup or modal is open, as it was at the keydown', () => {
		const h = vi.fn();
		const g = vi.fn();
		registerHotkey('N', g, { allowWithOverlay: true });
		registerHotkey('N', h);
		const off = layers.open('modal', () => {});
		press('n');
		off();
		vi.advanceTimersByTime(36);
		expect(h).not.toHaveBeenCalled();
		expect(g).toHaveBeenCalledTimes(1);
	});

	it('unsubscribe removes the handler', () => {
		const h = vi.fn();
		const off = registerHotkey('N', h);
		off();
		press('n');
		vi.advanceTimersByTime(36);
		expect(h).not.toHaveBeenCalled();
	});
});
