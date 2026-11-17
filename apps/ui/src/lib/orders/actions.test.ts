import type { Order } from '$api/Order';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '#lib/api/client.js';
import { order } from '#lib/app/fixtures.test-helper.js';
import { toast } from '#lib/ui/toast/toast.svelte.js';
import { closeEditor, unsavedText } from './actions.svelte.js';
import type { Conflict, OrderEditor, OrderField } from './editor.svelte.js';

/** An editor whose flush ends as given: saved, failed with an error, or with a disputed field. */
function editor(o: Order, end: 'saved' | 'error' | 'conflict') {
	const conflicts = new Map<OrderField, Conflict>(
		end === 'conflict' ? [['work_done', { mine: 'Моё', theirs: 'Чужое' }]] : []
	);
	const error = end === 'error' ? new ApiError(0, 'network', 'Нет связи с сервером', null) : null;
	return {
		order: o,
		conflicts,
		autosave: { error },
		flush: vi.fn(async () => end === 'saved'),
		dispose: vi.fn()
	} as unknown as OrderEditor & { dispose: ReturnType<typeof vi.fn> };
}

const settle = async () => {
	for (let i = 0; i < 10; i++) await Promise.resolve();
};

afterEach(() => {
	for (const t of toast.items) toast.dismiss(t.id);
});

describe('closeEditor', () => {
	it('saves what is left and stops quietly', async () => {
		const ed = editor(order(5), 'saved');
		closeEditor(ed);
		await settle();
		expect(ed.flush).toHaveBeenCalled();
		expect(ed.dispose).toHaveBeenCalled();
		expect(toast.items).toEqual([]);
	});

	it('a failed save is told', async () => {
		closeEditor(editor(order(5), 'error'));
		await settle();
		expect(toast.items.map((t) => t.message)).toEqual([
			'Изменения № 5 не сохранены: Нет связи с сервером'
		]);
	});

	it('a conflict left undecided is told too: the disputed values are not saved', async () => {
		const ed = editor(order(5), 'conflict');
		closeEditor(ed);
		await settle();
		expect(toast.items.map((t) => [t.message, t.tone])).toEqual([
			['Изменения № 5 не сохранены: Заказ изменили на другом компьютере', 'danger']
		]);
		expect(unsavedText(ed)).toBe('Изменения № 5 не сохранены: Заказ изменили на другом компьютере');
		expect(ed.dispose).toHaveBeenCalled();
	});
});
