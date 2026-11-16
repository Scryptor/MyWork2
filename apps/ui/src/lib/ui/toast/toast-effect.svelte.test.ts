import { afterEach, describe, expect, it, vi } from 'vitest';
import { flushSync } from 'svelte';
import { toast } from './toast.svelte.js';

let stop: (() => void) | undefined;

afterEach(() => {
	stop?.();
	stop = undefined;
	for (const t of [...toast.items]) toast.dismiss(t.id);
});

/** Runs `body` inside an effect, flushes, and returns how many times the effect ran. */
function inEffect(body: () => void): number {
	let runs = 0;
	stop = $effect.root(() => {
		$effect(() => {
			runs++;
			body();
		});
	});
	flushSync();
	return runs;
}

describe('toast called from an $effect does not make the effect depend on the toasts', () => {
	it('show and undoable: the effect runs once and each toast appears once', () => {
		const runs = inEffect(() => {
			toast.show({ message: 'Сохранено', tone: 'success' });
			toast.undoable('№ 10481 → Готов', () => {});
		});
		expect(runs).toBe(1);
		expect(toast.items.map((t) => t.message)).toEqual(['Сохранено', '№ 10481 → Готов']);
	});

	it('dismiss: the effect runs once and the toast is gone', () => {
		const id = toast.show({ message: 'Отправлено на принтер' });
		const runs = inEffect(() => toast.dismiss(id));
		expect(runs).toBe(1);
		expect(toast.items).toEqual([]);
	});

	it('undoLast: the effect runs once and the undo runs once', () => {
		const undo = vi.fn();
		toast.undoable('№ 10481 → Готов', undo);
		const runs = inEffect(() => toast.undoLast());
		expect(runs).toBe(1);
		expect(undo).toHaveBeenCalledTimes(1);
		expect(toast.items).toEqual([]);
	});
});
