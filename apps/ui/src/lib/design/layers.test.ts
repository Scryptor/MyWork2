import { describe, expect, it, vi } from 'vitest';
import { layers } from './layers.svelte.js';

function escape() {
	window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', code: 'Escape' }));
}

describe('layers', () => {
	it('starts empty', () => {
		expect(layers.topKind).toBeNull();
		expect(layers.blocking).toBe(false);
	});

	it('popup above panel blocks Escape; after it closes, Escape closes the panel once', () => {
		const closeA = vi.fn();
		const closeB = vi.fn();
		const offA = layers.open('panel', closeA);
		const offB = layers.open('popup', closeB);
		expect(layers.topKind).toBe('popup');
		expect(layers.blocking).toBe(true);
		escape();
		expect(closeA).not.toHaveBeenCalled();
		expect(closeB).not.toHaveBeenCalled();
		offB();
		expect(layers.topKind).toBe('panel');
		expect(layers.blocking).toBe(false);
		escape();
		expect(closeA).toHaveBeenCalledTimes(1);
		offA();
		expect(layers.topKind).toBeNull();
	});

	it('modal blocks too', () => {
		const off = layers.open('modal', () => {});
		expect(layers.blocking).toBe(true);
		off();
	});

	it('Escape already handled by a layer above (bits-ui) or typed during IME input leaves the panel open', () => {
		const close = vi.fn();
		const off = layers.open('panel', close);
		const handled = new KeyboardEvent('keydown', { key: 'Escape', cancelable: true });
		handled.preventDefault();
		window.dispatchEvent(handled);
		window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', isComposing: true }));
		window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', keyCode: 229 }));
		expect(close).not.toHaveBeenCalled();
		escape();
		expect(close).toHaveBeenCalledTimes(1);
		off();
	});

	it('an Esc a tooltip took (bits-ui prevents it) still closes the panel under the tooltip', () => {
		const close = vi.fn();
		const off = layers.open('panel', close);
		layers.escapeFromTooltip(new KeyboardEvent('keydown', { key: 'Escape' }));
		expect(close).toHaveBeenCalledTimes(1);
		layers.escapeFromTooltip(new KeyboardEvent('keydown', { key: 'Escape', isComposing: true }));
		expect(close).toHaveBeenCalledTimes(1);
		off();

		// Over a popup or a modal bits-ui keeps the Esc for itself: nothing more to close here.
		const below = vi.fn();
		const offPanel = layers.open('panel', below);
		const offModal = layers.open('modal', () => {});
		layers.escapeFromTooltip(new KeyboardEvent('keydown', { key: 'Escape' }));
		expect(below).not.toHaveBeenCalled();
		offModal();
		offPanel();
	});

	it('a paste or a drop belongs to the topmost modal or panel; popups above them do not count', () => {
		const page = document.createElement('div');
		const sheet = document.createElement('div');
		const inSheet = document.createElement('span');
		sheet.append(inSheet);
		document.body.append(page, sheet);
		// No layer: the page takes it.
		expect(layers.takesInput(page)).toBe(true);
		const offSheet = layers.open(
			'modal',
			() => {},
			() => sheet
		);
		expect(layers.takesInput(page)).toBe(false);
		expect(layers.takesInput(inSheet)).toBe(true);
		const offPopup = layers.open('popup', () => {});
		expect(layers.takesInput(inSheet)).toBe(true);
		// A modal that does not tell its element keeps the input from everything under it.
		const offDialog = layers.open('modal', () => {});
		expect(layers.takesInput(inSheet)).toBe(false);
		offDialog();
		offPopup();
		offSheet();
		expect(layers.takesInput(page)).toBe(true);
		page.remove();
		sheet.remove();
	});

	it('popups of a modal go with it: its content (and the popups) may outlive it by a frame', () => {
		const offSheet = layers.open('modal', () => {});
		const offA = layers.open('popup', () => {});
		const offB = layers.open('popup', () => {});
		offSheet();
		// The sheet is closed: N works at once, not when its content is gone.
		expect(layers.topKind).toBeNull();
		expect(layers.blocking).toBe(false);
		// The popups' own teardown comes later and changes nothing.
		offA();
		offB();
		expect(layers.topKind).toBeNull();

		// A popup over a modal over a panel: the panel stays.
		const offPanel = layers.open('panel', () => {});
		const offDialog = layers.open('modal', () => {});
		const offMenu = layers.open('popup', () => {});
		offDialog();
		expect(layers.topKind).toBe('panel');
		offMenu();
		offPanel();
	});

	it('a popup on the page or beside a panel, and a panel opened as a modal closes, stay', () => {
		const offPanel = layers.open('panel', () => {});
		const offMenu = layers.open('popup', () => {});
		offPanel();
		// The page beside a panel takes input: its menu is not the panel's.
		expect(layers.topKind).toBe('popup');
		offMenu();

		// Ctrl K → Enter: the palette closes as the order panel opens.
		const offPalette = layers.open('modal', () => {});
		const offOrder = layers.open('panel', () => {});
		offPalette();
		expect(layers.topKind).toBe('panel');
		offOrder();
		expect(layers.topKind).toBeNull();
	});
});
