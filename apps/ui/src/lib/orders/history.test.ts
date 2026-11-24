import type { AuditEntry } from '$api/AuditEntry';
import { describe, expect, it } from 'vitest';
import {
	historyCursor,
	historyItems,
	mergeHistory,
	statusBeforeIssue,
	statusTimeline,
	warrantyCases
} from './history.js';

function entry(id: number, action: string, details: AuditEntry['details'] = null): AuditEntry {
	return {
		id,
		at: `2026-11-1${id % 10}T10:00:00`,
		user_id: 1,
		user_name: 'Администратор',
		action,
		order_id: 7,
		client_id: 3,
		stock_item_id: null,
		summary: `Запись ${id}`,
		details
	};
}

describe('statusBeforeIssue', () => {
	// Newest first, as the server gives them.
	const entries = [
		entry(5, 'order.updated'),
		entry(4, 'order.status_changed', { from: 6, to: 8 }),
		entry(3, 'order.status_changed', { from: 2, to: 6 }),
		entry(2, 'order.status_changed', { from: 1, to: 8 }),
		entry(1, 'order.created')
	];

	it('is the status the newest change to the current one came from', () => {
		expect(statusBeforeIssue(entries, 8, [1, 2, 6])).toBe(6);
	});

	it('skips a status that cannot be chosen now', () => {
		expect(statusBeforeIssue(entries, 8, [1, 2])).toBeNull();
	});

	it('without such a change there is none', () => {
		expect(statusBeforeIssue([entry(1, 'order.created')], 8, [1])).toBeNull();
		expect(statusBeforeIssue([entry(2, 'order.status_changed', 'broken')], 8, [1])).toBeNull();
	});
});

describe('historyItems', () => {
	it('turns entries into timeline lines with who did it', () => {
		expect(historyItems([entry(3, 'order.status_changed')])).toEqual([
			{ id: 3, at: '2026-11-13T10:00:00', who: 'Администратор', text: 'Запись 3' }
		]);
	});
});

describe('statusTimeline', () => {
	it('keeps the creation, status changes, imported statuses and warranty cases', () => {
		const entries = [
			entry(6, 'order.warranty_case', {
				complaint: 'Снова не включается',
				at: '2026-11-16T09:00:00'
			}),
			entry(5, 'order.updated'),
			entry(4, 'order.status_changed', { from: 6, to: 8 }),
			entry(3, 'legacy.status'),
			entry(2, 'legacy.history'),
			entry(1, 'order.created')
		];
		expect(statusTimeline(entries).map((i) => i.id)).toEqual([6, 4, 3, 1]);
	});
});

describe('warrantyCases', () => {
	it('takes the date and the complaint of each case', () => {
		const entries = [
			entry(6, 'order.warranty_case', {
				complaint: 'Снова не включается',
				at: '2026-11-16T09:00:00'
			}),
			entry(5, 'order.status_changed', { from: 6, to: 8 }),
			entry(4, 'order.warranty_case', { complaint: 'Шумит' }),
			entry(3, 'order.warranty_case', 'broken')
		];
		expect(warrantyCases(entries)).toEqual([
			{ id: 6, at: '2026-11-16T09:00:00', complaint: 'Снова не включается' },
			{ id: 4, at: '2026-11-14T10:00:00', complaint: 'Шумит' },
			{ id: 3, at: '2026-11-13T10:00:00', complaint: 'Запись 3' }
		]);
	});
});

describe('mergeHistory', () => {
	it('puts new entries on top of those shown, each once, newest first', () => {
		const shown = [entry(3, 'a'), entry(2, 'a'), entry(1, 'a')];
		const fresh = [entry(5, 'a'), entry(4, 'a'), entry(3, 'a')];
		expect(mergeHistory(shown, fresh).map((e) => e.id)).toEqual([5, 4, 3, 2, 1]);
		expect(mergeHistory(shown, [entry(1, 'b')]).find((e) => e.id === 1)?.action).toBe('b');
	});
});

describe('historyCursor', () => {
	it('goes on after the last entry of the page as the server gave it, whatever its time', () => {
		// 9 is at «2026-11-19», later than 8 («…18»), yet the last of the page: the cursor is after it.
		const page = [entry(8, 'a'), entry(9, 'a')];
		expect(historyCursor(page)).toEqual({ before_at: '2026-11-19T10:00:00', before_id: 9 });
		expect(historyCursor([])).toBeUndefined();
	});
});
