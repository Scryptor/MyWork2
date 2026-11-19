import { render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import Harness from './SortableListHarness.test-helper.svelte';

const titles = () => screen.getAllByRole('listitem').map((li) => li.textContent?.trim());

afterEach(() => vi.useRealTimers());

describe('SortableList', () => {
	it('ArrowDown on a handle moves the row at once, keeps focus; onreorder comes after a pause', async () => {
		vi.useFakeTimers();
		const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
		const onorder = vi.fn();
		render(Harness, { onorder });
		screen.getByRole('button', { name: 'Переместить „Второй“' }).focus();
		await user.keyboard('{ArrowDown}');
		expect(titles()).toEqual(['Первый', 'Третий', 'Второй']);
		expect(onorder).not.toHaveBeenCalled();
		await vi.advanceTimersByTimeAsync(399);
		expect(onorder).not.toHaveBeenCalled();
		await vi.advanceTimersByTimeAsync(1);
		expect(onorder).toHaveBeenCalledTimes(1);
		expect(onorder).toHaveBeenCalledWith([1, 3, 2]);
		expect(screen.getByRole('button', { name: 'Переместить „Второй“' })).toHaveFocus();
		expect(screen.getByText('„Второй“ — место 3 из 3')).toBeInTheDocument();
		expect(screen.getByText('„Второй“ — место 3 из 3').closest('[aria-live]')).toHaveAttribute(
			'aria-live',
			'polite'
		);
	});

	it('several arrows in a row give one onreorder with the final order, and the place is announced after it', async () => {
		vi.useFakeTimers();
		const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
		const onorder = vi.fn();
		render(Harness, { onorder });
		screen.getByRole('button', { name: 'Переместить „Первый“' }).focus();
		await user.keyboard('{ArrowDown}');
		await vi.advanceTimersByTimeAsync(300);
		await user.keyboard('{ArrowDown}');
		await vi.advanceTimersByTimeAsync(300); // the pause starts anew with every key
		expect(onorder).not.toHaveBeenCalled();
		expect(screen.queryByText(/место/)).toBeNull();
		await vi.advanceTimersByTimeAsync(100);
		expect(onorder).toHaveBeenCalledTimes(1);
		expect(onorder).toHaveBeenCalledWith([2, 3, 1]);
		expect(screen.getByText('„Первый“ — место 3 из 3')).toBeInTheDocument();
	});

	it('focus leaving the handle sends the order at once, not after the pause', async () => {
		vi.useFakeTimers();
		const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
		const onorder = vi.fn();
		render(Harness, { onorder });
		screen.getByRole('button', { name: 'Переместить „Первый“' }).focus();
		await user.keyboard('{ArrowDown}');
		expect(onorder).not.toHaveBeenCalled();
		await user.tab();
		expect(onorder).toHaveBeenCalledTimes(1);
		expect(onorder).toHaveBeenCalledWith([2, 1, 3]);
		await vi.advanceTimersByTimeAsync(1000);
		expect(onorder).toHaveBeenCalledTimes(1);
	});

	it('an arrow back to where it began sends nothing', async () => {
		vi.useFakeTimers();
		const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
		const onorder = vi.fn();
		render(Harness, { onorder });
		screen.getByRole('button', { name: 'Переместить „Второй“' }).focus();
		await user.keyboard('{ArrowDown}{ArrowUp}');
		await vi.advanceTimersByTimeAsync(1000);
		expect(onorder).not.toHaveBeenCalled();
	});

	it('ArrowUp on the first row does nothing', async () => {
		const onorder = vi.fn();
		render(Harness, { onorder });
		screen.getByRole('button', { name: 'Переместить „Первый“' }).focus();
		await userEvent.keyboard('{ArrowUp}');
		expect(onorder).not.toHaveBeenCalled();
	});

	it('disabled has no handles', () => {
		render(Harness, { disabled: true });
		expect(screen.queryAllByRole('button')).toHaveLength(0);
		expect(screen.getByText('Второй')).toBeInTheDocument();
	});

	it('unmounting in the middle of a drag removes the window listeners', async () => {
		const add = vi.spyOn(window, 'addEventListener');
		const remove = vi.spyOn(window, 'removeEventListener');
		const { unmount } = render(Harness, {});
		const handle = screen.getByRole('button', { name: 'Переместить „Первый“' });
		handle.dispatchEvent(
			new PointerEvent('pointerdown', { button: 0, bubbles: true, clientY: 10, pointerId: 1 })
		);
		const names = ['pointermove', 'pointerup', 'pointercancel'];
		for (const n of names) expect(add.mock.calls.some((c) => c[0] === n)).toBe(true);
		unmount();
		for (const n of names) {
			const added = add.mock.calls.find((c) => c[0] === n)![1];
			expect(remove.mock.calls.some((c) => c[0] === n && c[1] === added)).toBe(true);
		}
		add.mockRestore();
		remove.mockRestore();
	});

	describe('with the mouse', () => {
		/** jsdom has no layout: row `i` is 40px high, from `i * 40`. */
		function layout() {
			vi.spyOn(Element.prototype, 'getBoundingClientRect').mockImplementation(function (
				this: Element
			) {
				const i =
					this.parentElement?.tagName === 'UL' ? [...this.parentElement.children].indexOf(this) : 0;
				return new DOMRect(0, i * 40, 300, 40);
			});
		}
		const pointer = (target: EventTarget, type: string, clientY: number) =>
			target.dispatchEvent(
				new PointerEvent(type, {
					button: 0,
					bubbles: true,
					cancelable: true,
					clientY,
					pointerId: 1
				})
			);
		const handle = (name: string) => screen.getByRole('button', { name: `Переместить „${name}“` });
		afterEach(() => vi.restoreAllMocks());

		it('dropping a row lower sends the new order and announces the place', async () => {
			layout();
			const onorder = vi.fn();
			render(Harness, { onorder });
			pointer(handle('Первый'), 'pointerdown', 20);
			pointer(window, 'pointermove', 110); // the middle of the third row is at 100
			pointer(window, 'pointerup', 110);
			await tick();
			expect(onorder).toHaveBeenCalledTimes(1);
			expect(onorder).toHaveBeenCalledWith([2, 3, 1]);
			expect(screen.getByText('„Первый“ — место 3 из 3')).toBeInTheDocument();
		});

		it('dropping it where it was sends nothing', () => {
			layout();
			const onorder = vi.fn();
			render(Harness, { onorder });
			pointer(handle('Второй'), 'pointerdown', 60);
			pointer(window, 'pointermove', 110);
			pointer(window, 'pointermove', 60);
			pointer(window, 'pointerup', 60);
			expect(onorder).not.toHaveBeenCalled();
		});

		it('Esc during the drag cancels it: the row goes back and the drop sends nothing', async () => {
			layout();
			const onorder = vi.fn();
			render(Harness, { onorder });
			pointer(handle('Первый'), 'pointerdown', 20);
			pointer(window, 'pointermove', 110);
			await tick();
			const li = handle('Первый').closest('li')!;
			expect(li.style.transform).toBe('translateY(90px)');
			const esc = new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true });
			window.dispatchEvent(esc);
			expect(esc.defaultPrevented).toBe(true);
			await tick();
			expect(li.style.transform).toBe('');
			pointer(window, 'pointerup', 110);
			expect(onorder).not.toHaveBeenCalled();
		});

		it('Esc without a drag is left alone', () => {
			render(Harness, {});
			const esc = new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true });
			window.dispatchEvent(esc);
			expect(esc.defaultPrevented).toBe(false);
		});

		it('a cancelled pointer (pointercancel) sends nothing', () => {
			layout();
			const onorder = vi.fn();
			render(Harness, { onorder });
			pointer(handle('Первый'), 'pointerdown', 20);
			pointer(window, 'pointermove', 110);
			pointer(window, 'pointercancel', 110);
			expect(onorder).not.toHaveBeenCalled();
		});
	});
});
