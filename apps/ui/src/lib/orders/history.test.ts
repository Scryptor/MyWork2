import type { AuditEntry } from '$api/AuditEntry';
import { describe, expect, it } from 'vitest';
import { historyItems, statusBeforeIssue } from './history.js';

function entry(id: number, action: string, details: AuditEntry['details'] = null): AuditEntry {
	return {
		id,
		at: `2026-11-1${id % 10}T10:00:00`,
		user_id: 1,
		user_name: 'Администратор',
		action,
		order_id: 7,
		client_id: 3,
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
