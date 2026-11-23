import { describe, expect, it, vi } from 'vitest';
import { createLeaveGuard, type LeaveNavigation } from './leave-guard.svelte.js';

// Leaving a screen with own changes not saved: the way out waits for the save and then goes on; closing
// the tab gets the browser's own question; the sign-in page is not held.

interface Ed {
	unsaved: boolean;
}

function nav(to: string | null, o: Partial<LeaveNavigation> = {}) {
	return {
		cancel: vi.fn(),
		willUnload: false,
		type: 'link',
		delta: 0,
		to: to === null ? null : { url: new URL(to, 'http://localhost') },
		...o
	} as LeaveNavigation & { cancel: ReturnType<typeof vi.fn> };
}

function setup(
	saved: (ed: Ed) => Promise<boolean>,
	ed: Ed | null = { unsaved: true },
	skip?: (nav: LeaveNavigation, ed: Ed) => boolean
) {
	const go = vi.fn();
	const goBack = vi.fn();
	const guard = createLeaveGuard<Ed>({
		editor: () => ed,
		unsaved: (e) => e.unsaved,
		saved,
		skip,
		goto: go,
		go: goBack
	});
	return { guard, go, goBack };
}

describe('createLeaveGuard', () => {
	it('lets a screen without unsaved changes go', () => {
		const { guard } = setup(async () => true, { unsaved: false });
		const n = nav('/orders');
		guard(n);
		expect(n.cancel).not.toHaveBeenCalled();
		const none = setup(async () => true, null);
		none.guard(n);
		expect(n.cancel).not.toHaveBeenCalled();
	});

	it('does not hold the sign-in page', () => {
		const { guard } = setup(async () => true);
		const n = nav('/login');
		guard(n);
		expect(n.cancel).not.toHaveBeenCalled();
	});

	it('closing the tab is cancelled for the browser to ask, nothing is saved', () => {
		const saved = vi.fn(async () => true);
		const { guard, go } = setup(saved);
		const n = nav(null, { willUnload: true });
		guard(n);
		expect(n.cancel).toHaveBeenCalled();
		expect(saved).not.toHaveBeenCalled();
		expect(go).not.toHaveBeenCalled();
	});

	it('a link waits for the save, then goes on to its address', async () => {
		const { guard, go } = setup(async () => true);
		const n = nav('/orders?x=1');
		guard(n);
		expect(n.cancel).toHaveBeenCalled();
		await Promise.resolve();
		await Promise.resolve();
		expect(go).toHaveBeenCalledWith(new URL('/orders?x=1', 'http://localhost'));
	});

	it('stays when the save did not succeed', async () => {
		const { guard, go } = setup(async () => false);
		guard(nav('/orders'));
		await Promise.resolve();
		await Promise.resolve();
		expect(go).not.toHaveBeenCalled();
	});

	it('the browser «Назад» goes back by its delta', async () => {
		const { guard, go, goBack } = setup(async () => true);
		guard(nav('/orders', { type: 'popstate', delta: -2 }));
		await Promise.resolve();
		await Promise.resolve();
		expect(goBack).toHaveBeenCalledWith(-2);
		expect(go).not.toHaveBeenCalled();
	});

	it('a second way out while the first waits replaces it: one save, the last way', async () => {
		let release: (ok: boolean) => void = () => {};
		const saved = vi.fn(() => new Promise<boolean>((r) => (release = r)));
		const { guard, go } = setup(saved);
		guard(nav('/a'));
		guard(nav('/b'));
		expect(saved).toHaveBeenCalledTimes(1);
		release(true);
		await Promise.resolve();
		await Promise.resolve();
		expect(go).toHaveBeenCalledTimes(1);
		expect(go).toHaveBeenCalledWith(new URL('/b', 'http://localhost'));
	});

	it('does not go on when the editor was replaced while the save waited', async () => {
		let release: (ok: boolean) => void = () => {};
		let ed: Ed | null = { unsaved: true };
		const go = vi.fn();
		const guard = createLeaveGuard<Ed>({
			editor: () => ed,
			unsaved: (e) => e.unsaved,
			saved: () => new Promise<boolean>((r) => (release = r)),
			goto: go,
			go: vi.fn()
		});
		guard(nav('/a'));
		ed = { unsaved: true };
		release(true);
		await Promise.resolve();
		await Promise.resolve();
		expect(go).not.toHaveBeenCalled();
	});

	it('a way out the screen says keeps it as it is: not held, not saved', () => {
		const saved = vi.fn(async () => true);
		const skip = vi.fn((n: LeaveNavigation) => n.to?.url.pathname === '/same');
		const { guard, go } = setup(saved, { unsaved: true }, skip);
		const stay = nav('/same');
		guard(stay);
		expect(stay.cancel).not.toHaveBeenCalled();
		expect(saved).not.toHaveBeenCalled();
		expect(go).not.toHaveBeenCalled();
		// Any other way is still held.
		const away = nav('/other');
		guard(away);
		expect(away.cancel).toHaveBeenCalled();
		expect(saved).toHaveBeenCalledTimes(1);
		// Without unsaved changes the predicate is not even asked.
		const calls = skip.mock.calls.length;
		setup(saved, { unsaved: false }, skip).guard(nav('/same'));
		expect(skip.mock.calls.length).toBe(calls);
	});
});
