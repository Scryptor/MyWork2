import { afterEach, describe, expect, it, vi } from 'vitest';
import {
	__resetHotkeysForTests,
	formatShortcut,
	isEditableTarget,
	matchShortcut,
	registerHotkey
} from './hotkeys.svelte.js';
import { layers } from './layers.svelte.js';

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

describe('registerHotkey', () => {
	const press = (target: EventTarget, init: KeyboardEventInit = {}) =>
		target.dispatchEvent(
			new KeyboardEvent('keydown', { key: 'n', code: 'KeyN', bubbles: true, ...init })
		);

	it('calls the handler and prevents default', () => {
		const h = vi.fn();
		registerHotkey('N', h);
		const e = new KeyboardEvent('keydown', {
			key: 'n',
			code: 'KeyN',
			bubbles: true,
			cancelable: true
		});
		document.body.dispatchEvent(e);
		expect(h).toHaveBeenCalledTimes(1);
		expect(e.defaultPrevented).toBe(true);
	});

	it('ignores events from fields unless allowed', () => {
		document.body.innerHTML = '<input id="i">';
		const input = document.getElementById('i')!;
		const h = vi.fn();
		const off = registerHotkey('N', h);
		press(input);
		expect(h).not.toHaveBeenCalled();
		off();
		const g = vi.fn();
		registerHotkey('N', g, { allowInFields: true });
		press(input);
		expect(g).toHaveBeenCalledTimes(1);
	});

	it('ignores IME composition', () => {
		const h = vi.fn();
		registerHotkey('N', h);
		press(document.body, { isComposing: true });
		expect(h).not.toHaveBeenCalled();
	});

	it('stands down while a popup or modal is open', () => {
		const h = vi.fn();
		const g = vi.fn();
		registerHotkey('N', g, { allowWithOverlay: true });
		registerHotkey('N', h);
		const off = layers.open('modal', () => {});
		press(document.body);
		expect(h).not.toHaveBeenCalled();
		expect(g).toHaveBeenCalledTimes(1);
		off();
	});

	it('unsubscribe removes the handler', () => {
		const h = vi.fn();
		const off = registerHotkey('N', h);
		off();
		press(document.body);
		expect(h).not.toHaveBeenCalled();
	});
});
