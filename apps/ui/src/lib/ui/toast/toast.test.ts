import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { toast } from './toast.svelte.js';

const messages = () => toast.items.map((t) => t.message);

beforeEach(() => {
	vi.useFakeTimers();
});

afterEach(() => {
	toast.resume();
	for (const t of [...toast.items]) toast.dismiss(t.id);
	vi.useRealTimers();
});

describe('toast', () => {
	it('the fourth toast pushes out the oldest: at most three are shown', () => {
		toast.show({ message: 'один' });
		toast.show({ message: 'два' });
		toast.show({ message: 'три' });
		expect(messages()).toEqual(['один', 'два', 'три']);
		toast.show({ message: 'четыре' });
		expect(messages()).toEqual(['два', 'три', 'четыре']);
	});

	it('a toast disappears after 6000 ms', () => {
		toast.show({ message: 'Сохранено', tone: 'success' });
		vi.advanceTimersByTime(5999);
		expect(messages()).toEqual(['Сохранено']);
		vi.advanceTimersByTime(1);
		expect(toast.items).toEqual([]);
	});

	it('honours a custom duration and dismiss by id', () => {
		const short = toast.show({ message: 'коротко', duration: 1000 });
		const other = toast.show({ message: 'другое' });
		vi.advanceTimersByTime(1000);
		expect(messages()).toEqual(['другое']);
		toast.dismiss(other);
		toast.dismiss(short);
		expect(toast.items).toEqual([]);
	});

	it('undoLast runs the undo of the newest undoable toast, removes it and returns true', () => {
		const first = vi.fn();
		const second = vi.fn();
		toast.undoable('№ 1 → Готов', first);
		toast.undoable('№ 2 → Готов', second);
		toast.show({ message: 'Не удалось сохранить', action: { label: 'Повторить', run: vi.fn() } });

		expect(toast.undoLast()).toBe(true);
		expect(second).toHaveBeenCalledTimes(1);
		expect(first).not.toHaveBeenCalled();
		expect(messages()).toEqual(['№ 1 → Готов', 'Не удалось сохранить']);

		expect(toast.undoLast()).toBe(true);
		expect(first).toHaveBeenCalledTimes(1);
		expect(messages()).toEqual(['Не удалось сохранить']);

		expect(toast.undoLast()).toBe(false);
		expect(messages()).toEqual(['Не удалось сохранить']);
	});

	it('undoLast returns false when there is nothing to undo', () => {
		expect(toast.undoLast()).toBe(false);
		toast.show({ message: 'Отправлено на принтер' });
		expect(toast.undoLast()).toBe(false);
	});

	it('an undoable toast carries the «Отменить» action', () => {
		const undo = vi.fn();
		toast.undoable('№ 10481 → Готов', undo);
		const [item] = toast.items;
		expect(item?.action?.label).toBe('Отменить');
		void item?.action?.run();
		expect(undo).toHaveBeenCalledTimes(1);
	});

	it('pause holds every timer; resume continues with the time that was left', () => {
		toast.show({ message: 'раньше' });
		vi.advanceTimersByTime(4000);
		toast.pause();
		toast.show({ message: 'позже' });
		vi.advanceTimersByTime(60_000);
		expect(messages()).toEqual(['раньше', 'позже']);
		toast.resume();
		vi.advanceTimersByTime(1999);
		expect(messages()).toEqual(['раньше', 'позже']);
		vi.advanceTimersByTime(1);
		expect(messages()).toEqual(['позже']);
		vi.advanceTimersByTime(4000);
		expect(toast.items).toEqual([]);
	});
});
