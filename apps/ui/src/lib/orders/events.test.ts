import { describe, expect, it, vi } from 'vitest';
import { order } from '#lib/app/fixtures.test-helper.js';
import { emitOrderChanged, onOrderChanged } from './events.js';

describe('order events', () => {
	it('every listener hears a change until it unsubscribes', () => {
		const a = vi.fn();
		const b = vi.fn();
		const offA = onOrderChanged(a);
		const offB = onOrderChanged(b);
		emitOrderChanged(order(1));
		offA();
		emitOrderChanged(order(2));
		offB();
		emitOrderChanged(order(3));
		expect(a.mock.calls.map(([o]) => o.id)).toEqual([1]);
		expect(b.mock.calls.map(([o]) => o.id)).toEqual([1, 2]);
	});

	it('says whether the order was just created (intake) or changed', () => {
		const heard = vi.fn();
		const off = onOrderChanged(heard);
		emitOrderChanged(order(1), 'created');
		emitOrderChanged(order(2));
		off();
		expect(heard.mock.calls.map(([o, change]) => [o.id, change])).toEqual([
			[1, 'created'],
			[2, 'updated']
		]);
	});

	it('a listener that unsubscribes while being called does not stop the others', () => {
		const seen: number[] = [];
		const off = onOrderChanged(() => {
			seen.push(1);
			off();
		});
		const offOther = onOrderChanged(() => seen.push(2));
		emitOrderChanged(order(1));
		emitOrderChanged(order(2));
		offOther();
		expect(seen).toEqual([1, 2, 2]);
	});
});
