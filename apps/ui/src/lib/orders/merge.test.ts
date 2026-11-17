import { describe, expect, it } from 'vitest';
import { mergeRemote } from './merge.js';

describe('mergeRemote', () => {
	it('splits remote changes into taken and conflicting fields', () => {
		const r = mergeRemote({ a: 1, b: 1, c: 1 }, { a: 2, c: 5 }, { a: 3, b: 2, c: 5 });
		expect(r).toEqual({ take: { b: 2 }, conflicts: ['a'], changedByOthers: ['b'] });
	});

	it('nothing changed elsewhere: own edits are not conflicts', () => {
		const r = mergeRemote<{ a: string; b: number | null }>(
			{ a: 'x', b: null },
			{ a: 'y', b: 100 },
			{ a: 'x', b: null }
		);
		expect(r).toEqual({ take: {}, conflicts: [], changedByOthers: [] });
	});

	it('compares values, not references', () => {
		const term = { unit: 'days', value: 30 };
		const r = mergeRemote(
			{ term, warranty: '30 дней' },
			{},
			{ term: { unit: 'days', value: 30 }, warranty: '30 дней' }
		);
		expect(r.changedByOthers).toEqual([]);
		const changed = mergeRemote({ term }, {}, { term: { unit: 'months', value: 1 } });
		expect(changed.take).toEqual({ term: { unit: 'months', value: 1 } });
	});

	it('an own edit back to the old value conflicts with a new one from elsewhere', () => {
		const r = mergeRemote({ a: 'old' }, { a: 'old' }, { a: 'new' });
		expect(r.conflicts).toEqual(['a']);
	});
});
