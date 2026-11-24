import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeAll, describe, expect, it, vi } from 'vitest';
import Harness from './MultiComboboxHarness.test-helper.svelte';

// What bits-ui calls on a pointer press and jsdom does not have.
beforeAll(() => {
	Element.prototype.hasPointerCapture ??= () => false;
	Element.prototype.releasePointerCapture ??= () => {};
	Element.prototype.scrollIntoView ??= () => {};
});

/**
 * jsdom does not place the list, so it stays `visibility: hidden`: the queries take `hidden: true`, and the
 * options are read by their text (a hidden one has no accessible name).
 */
const hidden = { hidden: true } as const;
const field = () => screen.getByRole('combobox', { name: 'Комплектность' });
const out = () => JSON.parse(screen.getByTestId('out').textContent ?? '[]') as string[];
const shown = () => screen.queryAllByRole('option', hidden).map((o) => o.textContent?.trim() ?? '');

const OPTIONS = ['Чехол', 'Коробка', 'Документы к коробке', 'Шнур'];

describe('MultiCombobox', () => {
	it('the chosen values are tags that can be taken away', async () => {
		render(Harness, { options: OPTIONS, selected: ['Чехол', 'Коробка'] });
		await userEvent.click(screen.getByRole('button', { name: 'Убрать: Чехол' }));
		expect(out()).toEqual(['Коробка']);
		expect(screen.queryByRole('button', { name: 'Убрать: Чехол' })).toBeNull();
		expect(screen.getByRole('button', { name: 'Убрать: Коробка' })).toBeTruthy();
	});

	it('typing leaves the matching options only, in any case, and not the chosen ones', async () => {
		render(Harness, { options: OPTIONS, selected: ['Документы к коробке'] });
		await userEvent.type(field(), 'КОР');
		expect(field()).toHaveAttribute('aria-expanded', 'true');
		expect(shown()).toEqual(['Коробка']);
	});

	it('the list opens with every option not chosen, in the given order', async () => {
		render(Harness, { options: OPTIONS, selected: ['Шнур'] });
		await userEvent.click(field());
		expect(shown()).toEqual(['Чехол', 'Коробка', 'Документы к коробке']);
	});

	it('↓ and Enter pick the highlighted option; the field empties and the list stays', async () => {
		render(Harness, { options: OPTIONS });
		field().focus();
		await userEvent.keyboard('{ArrowDown}{ArrowDown}{Enter}');
		expect(out()).toEqual(['Коробка']);
		expect(field()).toHaveValue('');
		expect(shown()).toEqual(['Чехол', 'Документы к коробке', 'Шнур']);
	});

	it('a typed option is picked with Enter, the text goes from the field', async () => {
		const oncreate = vi.fn();
		render(Harness, { options: OPTIONS, allowCustom: true, oncreate });
		await userEvent.type(field(), 'шнур{Enter}');
		expect(out()).toEqual(['Шнур']);
		expect(field()).toHaveValue('');
		expect(oncreate).not.toHaveBeenCalled();
	});

	it('Enter with own text adds it when allowed and tells oncreate', async () => {
		const oncreate = vi.fn();
		render(Harness, { options: OPTIONS, allowCustom: true, oncreate });
		await userEvent.type(field(), '  Плёнка   на экране ');
		expect(shown()).toEqual(['Добавить «Плёнка на экране»']);
		await userEvent.keyboard('{Enter}');
		expect(out()).toEqual(['Плёнка на экране']);
		expect(oncreate).toHaveBeenCalledExactlyOnceWith('Плёнка на экране');
		expect(field()).toHaveValue('');
	});

	it('oncreate is told when the options take in the chosen values at once, as in the intake', async () => {
		const oncreate = vi.fn();
		render(Harness, { options: OPTIONS, allowCustom: true, oncreate, withChosen: true });
		await userEvent.type(field(), 'Плёнка{Enter}');
		expect(out()).toEqual(['Плёнка']);
		expect(oncreate).toHaveBeenCalledExactlyOnceWith('Плёнка');
		await userEvent.type(field(), 'чех{Enter}');
		expect(out()).toEqual(['Плёнка', 'Чехол']);
		expect(oncreate).toHaveBeenCalledOnce();
	});

	it('own text that is part of an option: the option first, «Добавить» after it', async () => {
		const oncreate = vi.fn();
		render(Harness, { options: OPTIONS, allowCustom: true, oncreate });
		await userEvent.type(field(), 'короб');
		expect(shown()).toEqual(['Коробка', 'Документы к коробке', 'Добавить «короб»']);
		await userEvent.keyboard('{ArrowDown}{ArrowDown}{Enter}');
		expect(out()).toEqual(['короб']);
		expect(oncreate).toHaveBeenCalledExactlyOnceWith('короб');
	});

	it('without allowCustom own text is not added', async () => {
		const oncreate = vi.fn();
		render(Harness, { options: OPTIONS, oncreate });
		await userEvent.type(field(), 'Плёнка');
		expect(shown()).toEqual([]);
		expect(screen.getByText('Ничего не найдено')).toBeTruthy();
		await userEvent.keyboard('{Enter}');
		expect(out()).toEqual([]);
		expect(oncreate).not.toHaveBeenCalled();
	});

	it('Backspace in an empty field takes the last tag away', async () => {
		render(Harness, { options: OPTIONS, selected: ['Чехол', 'Коробка'] });
		await userEvent.type(field(), 'ш{Backspace}');
		expect(out()).toEqual(['Чехол', 'Коробка']);
		await userEvent.keyboard('{Backspace}');
		expect(out()).toEqual(['Чехол']);
	});

	it('Ctrl/⌘+Enter (accept the form) picks nothing, highlighted or typed', async () => {
		const oncreate = vi.fn();
		render(Harness, { options: OPTIONS, allowCustom: true, oncreate });
		await userEvent.type(field(), 'кор');
		await userEvent.keyboard('{Control>}{Enter}{/Control}{Meta>}{Enter}{/Meta}');
		expect(out()).toEqual([]);
		await userEvent.clear(field());
		await userEvent.type(field(), 'Плёнка');
		await userEvent.keyboard('{Control>}{Enter}{/Control}');
		expect(out()).toEqual([]);
		expect(oncreate).not.toHaveBeenCalled();
	});

	it('a value repeated in the chosen ones (an old draft) is one tag, the next edit drops the repeat', async () => {
		render(Harness, { options: OPTIONS, selected: ['Чехол', 'Чехол', 'Шнур'] });
		expect(screen.getAllByRole('button', { name: 'Убрать: Чехол' })).toHaveLength(1);
		await userEvent.type(field(), 'коробка{Enter}');
		expect(out()).toEqual(['Чехол', 'Шнур', 'Коробка']);
	});

	it('Escape closes the list', async () => {
		render(Harness, { options: OPTIONS });
		await userEvent.click(field());
		expect(field()).toHaveAttribute('aria-expanded', 'true');
		expect(shown()).toHaveLength(4);
		await userEvent.keyboard('{Escape}');
		expect(field()).toHaveAttribute('aria-expanded', 'false');
		await waitFor(() => expect(shown()).toEqual([]));
		expect(out()).toEqual([]);
	});

	it('shows at most the limit', async () => {
		const many = Array.from({ length: 300 }, (_, i) => `Вариант ${i + 1}`);
		render(Harness, { options: many });
		await userEvent.click(field());
		expect(shown()).toHaveLength(50);
		expect(shown()[0]).toBe('Вариант 1');
		// The rest is found by typing.
		await userEvent.type(field(), 'вариант 300');
		expect(shown()).toEqual(['Вариант 300']);
	});
});
