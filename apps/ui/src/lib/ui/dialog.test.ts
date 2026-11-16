import { tick } from 'svelte';
import { afterEach, describe, expect, it } from 'vitest';
import { dialogFocus } from './dialog.js';

/** The page: <main tabindex="-1"> with two buttons, as on /design and in AppShell. */
function page() {
	document.body.innerHTML = `<main tabindex="-1"><button id="a">A</button><button id="b"><span id="b-icon">B</span></button></main>`;
	const get = (id: string) => document.getElementById(id) as HTMLElement;
	return { main: document.querySelector('main')!, a: get('a'), b: get('b'), icon: get('b-icon') };
}

const press = (el: Element) =>
	el.dispatchEvent(new Event('pointerdown', { bubbles: true, cancelable: true }));
const key = (k: string) =>
	document.activeElement!.dispatchEvent(new KeyboardEvent('keydown', { key: k, bubbles: true }));

/** What bits-ui passes to onCloseAutoFocus. */
const closeEvent = () => new CustomEvent('focusScope.onCloseAutoFocus', { cancelable: true });

afterEach(() => {
	document.body.innerHTML = '';
});

describe('dialogFocus', () => {
	it('back to the focused opener (the keyboard, a click in Chromium)', () => {
		const { a } = page();
		const f = dialogFocus();
		a.focus();
		key('Enter');
		f.opened();
		a.blur();
		const e = closeEvent();
		f.closed(e);
		expect(e.defaultPrevented).toBe(true);
		expect(document.activeElement).toBe(a);
	});

	it('back to the pressed button when a click left focus around it (WebKit)', () => {
		const { main, b, icon } = page();
		const f = dialogFocus();
		press(icon);
		main.focus(); // WebKit focuses the nearest focusable ancestor, not the button
		f.opened();
		f.closed(closeEvent());
		expect(document.activeElement).toBe(b);
	});

	it('to <main> when the opener is gone', () => {
		const { main, b } = page();
		const f = dialogFocus();
		press(b);
		f.opened();
		b.remove();
		f.closed(closeEvent());
		expect(document.activeElement).toBe(main);
	});

	it('to <main> when the opener goes in the same update as the dialog', async () => {
		const { main, a } = page();
		const f = dialogFocus();
		a.focus();
		f.opened();
		f.closed(closeEvent());
		expect(document.activeElement).toBe(a);
		a.remove();
		await tick();
		expect(document.activeElement).toBe(main);
	});

	it('to <main> when nothing had focus: <body> is no opener', () => {
		const { main } = page();
		const f = dialogFocus();
		f.opened();
		f.closed(closeEvent());
		expect(document.activeElement).toBe(main);
	});

	it('a key since the last click: the focused element opened the dialog, not the clicked one', () => {
		const { a, b } = page();
		const f = dialogFocus();
		press(a);
		b.focus();
		key('k');
		f.opened();
		f.closed(closeEvent());
		expect(document.activeElement).toBe(b);
	});

	it('a click that moved focus elsewhere (a menu returns focus to its trigger): the focused one wins', () => {
		const { a, b } = page();
		const f = dialogFocus();
		press(a);
		b.focus();
		f.opened();
		f.closed(closeEvent());
		expect(document.activeElement).toBe(b);
	});

	it('without <main> and without an opener, bits-ui keeps its default', () => {
		document.body.innerHTML = '<button id="x">X</button>';
		const f = dialogFocus();
		f.opened();
		const e = closeEvent();
		f.closed(e);
		expect(e.defaultPrevented).toBe(false);
	});
});
