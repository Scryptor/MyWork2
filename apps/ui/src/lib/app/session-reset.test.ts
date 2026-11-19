import type { AppSettings } from '$api/AppSettings';
import { describe, expect, it, vi } from 'vitest';
import { app } from '#lib/app/context.svelte.js';
import { clearAttachmentCache } from '#lib/api/attachments.js';
import { createEntityEditor } from '#lib/app/entity-editor.svelte.js';
import { summary } from '#lib/app/summary.svelte.js';
import { undo } from '#lib/app/undo.svelte.js';
import { forgetDraft, INTAKE_DRAFT_KEY } from '#lib/intake/draft.js';
import { orderPanel } from '#lib/orders/panel.svelte.js';
import { savedViews } from '#lib/orders/saved-views.svelte.js';
import { toast } from '#lib/ui/toast/toast.svelte.js';
import {
	discardQueues,
	sessionEpoch,
	hasUnsaved,
	onSessionReset,
	resetSession,
	settleQueues,
	trackQueue
} from './session-reset.js';
import { POINTS, STATUSES, user } from './fixtures.test-helper.js';

describe('queues', () => {
	it('settleQueues sends what waits and says nothing is left', async () => {
		let left = true;
		const flush = vi.fn(async () => ((left = false), true));
		const off = trackQueue({ unsaved: () => left, flush, discard: () => (left = false) });
		expect(hasUnsaved()).toBe(true);
		expect(await settleQueues(1000)).toBe(true);
		expect(flush).toHaveBeenCalledTimes(1);
		off();
	});

	it('a queue that cannot send leaves unsaved changes', async () => {
		const discard = vi.fn();
		const off = trackQueue({ unsaved: () => true, flush: async () => false, discard });
		expect(await settleQueues(1000)).toBe(false);
		discardQueues();
		expect(discard).toHaveBeenCalled();
		off();
	});

	it('a queue whose send failed asks, though nothing waits in it any more (a refused deletion)', async () => {
		const off = trackQueue({ unsaved: () => false, flush: async () => false, discard: () => {} });
		expect(hasUnsaved()).toBe(false);
		expect(await settleQueues(1000)).toBe(false);
		off();
	});

	it('a queue that hangs is given up after the timeout', async () => {
		vi.useFakeTimers();
		const off = trackQueue({
			unsaved: () => true,
			flush: () => new Promise<boolean>(() => {}),
			discard: () => {}
		});
		const settled = settleQueues(5000);
		await vi.advanceTimersByTimeAsync(5000);
		expect(await settled).toBe(false);
		off();
		vi.useRealTimers();
	});

	it('an editor registers its queue until it is disposed', async () => {
		const save = vi.fn(async (patch: { name?: string }, version: number) => ({
			id: 1,
			version: version + 1,
			name: patch.name ?? ''
		}));
		const ed = createEntityEditor({
			initial: { id: 1, version: 1, name: 'a' },
			fields: ['name'],
			load: async () => ({ id: 1, version: 1, name: 'a' }),
			save
		});
		ed.autosave.edit('name', 'b');
		expect(hasUnsaved()).toBe(true);
		// Signing out does not wait for the 600 ms of the pause: the edit goes at once.
		expect(await settleQueues(1000)).toBe(true);
		expect(save).toHaveBeenCalledWith({ name: 'b' }, 1);
		expect(hasUnsaved()).toBe(false);

		ed.autosave.edit('name', 'c');
		resetSession();
		expect(hasUnsaved()).toBe(false);
		await new Promise((r) => setTimeout(r, 700));
		// The edit of the user who signed out never goes out.
		expect(save).toHaveBeenCalledTimes(1);
		ed.dispose();
	});
});

describe('resetSession', () => {
	it('every singleton of the session goes back to its start', () => {
		app.set({
			server: {
				app: 'mw',
				version: '0.1.0',
				api_version: 1,
				auth_required: true,
				has_users: true,
				setup_required: false,
				server_time: '2026-11-15T10:24:00'
			},
			me: user(),
			statuses: STATUSES,
			points: POINTS,
			settings: {} as AppSettings
		});
		orderPanel.id = 5;
		const sent: string[] = [];
		const stop = onSessionReset(() => void sent.push('x'));
		undo.push('№ 1', async () => {});
		toast.show({ message: 'старое' });
		clearAttachmentCache();
		expect(toast.items.length).toBeGreaterThan(0);

		resetSession();

		expect(orderPanel.id).toBeNull();
		expect(toast.items).toEqual([]);
		expect(summary.current).toBeNull();
		expect(() => app.me).toThrow();
		expect(savedViews.list).toEqual([]);
		expect(sent).toEqual(['x']);
		stop();
	});
});

describe('the intake draft', () => {
	it('an expired session (the reset of the login page) keeps it; signing out removes it', () => {
		localStorage.setItem(INTAKE_DRAFT_KEY, '{"owner":5}');
		resetSession();
		expect(localStorage.getItem(INTAKE_DRAFT_KEY)).toBe('{"owner":5}');
		forgetDraft();
		expect(localStorage.getItem(INTAKE_DRAFT_KEY)).toBeNull();
	});
});

describe('queued actions', () => {
	it('an action still waiting when the session ends sends no request', async () => {
		let release: () => void = () => {};
		const slow = new Promise<void>((resolve) => (release = resolve));
		type R = { id: number; version: number; name: string };
		const ed = createEntityEditor<R, 'name'>({
			initial: { id: 1, version: 1, name: 'a' },
			fields: ['name'],
			load: async () => ({ id: 1, version: 1, name: 'a' }),
			save: async (p, v) => {
				await slow;
				return { id: 1, version: v + 1, name: p.name ?? '' };
			}
		});
		ed.autosave.commit('name', 'b');
		const action = vi.fn(async (v: number) => ({ id: 1, version: v + 1, name: 'x' }));
		const before = sessionEpoch();
		let settled = false;
		void ed.run(action).then(() => (settled = true));
		// The action waits behind the save; the user signs out meanwhile.
		resetSession();
		expect(sessionEpoch()).toBe(before + 1);
		release();
		await new Promise((r) => setTimeout(r, 20));
		expect(action).not.toHaveBeenCalled();
		expect(settled).toBe(false);
		ed.dispose();
	});

	it('an action of the same session still runs', async () => {
		type R = { id: number; version: number; name: string };
		const ed = createEntityEditor<R, 'name'>({
			initial: { id: 1, version: 1, name: 'a' },
			fields: ['name'],
			load: async () => ({ id: 1, version: 1, name: 'a' }),
			save: async (p, v) => ({ id: 1, version: v + 1, name: p.name ?? '' })
		});
		const action = vi.fn(async (v: number) => ({ id: 1, version: v + 1, name: 'x' }));
		await ed.run(action);
		expect(action).toHaveBeenCalledOnce();
		ed.dispose();
	});
});
