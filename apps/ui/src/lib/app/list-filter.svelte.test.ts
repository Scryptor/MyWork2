import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushSync } from 'svelte';
import { __resetHotkeysForTests } from '#lib/design/hotkeys.svelte.js';
import { FILTER_MS, createListFilter } from './list-filter.svelte.js';

let stop: (() => void) | undefined;
beforeEach(() => vi.useFakeTimers());
afterEach(() => {
	stop?.();
	stop = undefined;
	vi.useRealTimers();
	__resetHotkeysForTests();
});

function make(initial = '') {
	let address = $state(initial);
	const sent: string[] = [];
	let filter!: ReturnType<typeof createListFilter>;
	stop = $effect.root(() => {
		filter = createListFilter({ query: () => address, send: (q) => sent.push(q) });
	});
	flushSync();
	return {
		filter,
		sent,
		setAddress(v: string) {
			address = v;
			flushSync();
		}
	};
}

describe('createListFilter', () => {
	it('starts with the text of the address', () => {
		expect(make('экран').filter.text).toBe('экран');
	});

	it('sends the text once typing pauses, only the last one', () => {
		const { filter, sent } = make();
		filter.text = 'э';
		filter.typed('э');
		vi.advanceTimersByTime(FILTER_MS - 1);
		filter.text = 'эк';
		filter.typed('эк');
		vi.advanceTimersByTime(FILTER_MS - 1);
		expect(sent).toEqual([]);
		vi.advanceTimersByTime(1);
		expect(sent).toEqual(['эк']);
	});

	it('does not send what the address already has, and cancel drops a waiting send', () => {
		const { filter, sent } = make('a');
		filter.typed('a');
		vi.advanceTimersByTime(FILTER_MS);
		filter.typed('b');
		filter.cancel();
		vi.advanceTimersByTime(FILTER_MS);
		expect(sent).toEqual([]);
	});

	it('its own text coming back in the address does not touch the field, another one replaces it', () => {
		const { filter, sent, setAddress } = make();
		filter.text = 'дис';
		filter.typed('дис');
		vi.advanceTimersByTime(FILTER_MS);
		expect(sent).toEqual(['дис']);
		filter.text = 'дисп'; // typed on after the send
		setAddress('дис');
		expect(filter.text).toBe('дисп');
		setAddress('батарея');
		expect(filter.text).toBe('батарея');
	});

	it('a change of the address that keeps the filter text does not undo what is typed', () => {
		let view = $state({ query: '', chip: 'a' });
		let filter!: ReturnType<typeof createListFilter>;
		stop = $effect.root(() => {
			filter = createListFilter({ query: () => view.query, send: () => {} });
		});
		flushSync();
		filter.text = 'печатаю';
		view = { query: '', chip: 'b' };
		flushSync();
		expect(filter.text).toBe('печатаю');
	});

	it('an address change drops the send that waits', () => {
		const { filter, sent, setAddress } = make();
		filter.typed('старое');
		setAddress('другое');
		vi.advanceTimersByTime(FILTER_MS);
		expect(sent).toEqual([]);
		expect(filter.text).toBe('другое');
	});

	it('/ focuses the element given as ref', () => {
		const { filter } = make();
		const input = document.createElement('input');
		document.body.append(input);
		filter.ref = input;
		flushSync();
		window.dispatchEvent(new KeyboardEvent('keydown', { key: '/', code: 'Slash', bubbles: true }));
		vi.advanceTimersByTime(1000);
		expect(document.activeElement).toBe(input);
		input.remove();
	});
});
