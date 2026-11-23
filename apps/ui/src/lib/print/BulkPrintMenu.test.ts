import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

// «Печать ▾» of the selection bar: the program prints, a browser only saves a PDF.
const platform = vi.hoisted(() => ({ canPrintDirectly: vi.fn<() => boolean>(() => false) }));
vi.mock('#lib/print/platform.js', () => platform);
vi.mock('$app/navigation', () => ({ goto: vi.fn() }));
vi.mock('$app/paths', () => ({ resolve: (p: string) => `/${p}` }));

import BulkPrintMenu from './BulkPrintMenu.svelte';

/** Opened from the keyboard; jsdom does not place the menu, so its parts are found with `hidden`. */
async function open() {
	screen.getByRole('button', { name: 'Печать' }).focus();
	await userEvent.keyboard('{Enter}');
	await screen.findByRole('menu', { hidden: true });
}
/** Chosen from the keyboard, as a click would. */
async function choose(item: HTMLElement) {
	item.focus();
	await userEvent.keyboard('{Enter}');
}
/** The group of a heading (jsdom does not place the menu, so the name is read from `aria-labelledby`). */
const groups = () =>
	[...document.querySelectorAll<HTMLElement>('[role="group"]')].map((g) => ({
		name: document.getElementById(g.getAttribute('aria-labelledby') ?? '')?.textContent?.trim(),
		el: g
	}));
const group = (name: string) => groups().find((g) => g.name === name)!.el;
const itemIn = (name: string, label: string) =>
	[...group(name).querySelectorAll<HTMLElement>('[role="menuitem"]')].find(
		(i) => i.textContent?.trim() === label
	)!;
const props = () => ({ onprint: vi.fn(), onsave: vi.fn(), onstop: vi.fn() });

describe('BulkPrintMenu', () => {
	it('while a batch runs: «Печать: 3 из 12» and «Остановить» instead of the menu; stop calls back', async () => {
		const p = props();
		render(BulkPrintMenu, { count: 12, progress: { done: 3, total: 12 }, ...p });
		expect(screen.getByText('Печать: 3 из 12')).toBeInTheDocument();
		expect(screen.queryByRole('button', { name: 'Печать' })).toBeNull();
		await userEvent.click(screen.getByRole('button', { name: 'Остановить' }));
		expect(p.onstop).toHaveBeenCalledOnce();
	});

	it('over the limit: the items are disabled and the hint is shown', async () => {
		const p = props();
		render(BulkPrintMenu, { count: 101, ...p });
		expect(screen.getByText('Не больше 100 заказов за раз')).toBeInTheDocument();
		await open();
		const items = screen.getAllByRole('menuitem', { hidden: true });
		expect(items).toHaveLength(5);
		for (const item of items) expect(item).toHaveAttribute('data-disabled');
		await choose(items[4]!);
		expect(p.onsave).not.toHaveBeenCalled();
	});

	it('at the limit: no hint, an item saves; a browser has only «Сохранить PDF»', async () => {
		const p = props();
		render(BulkPrintMenu, { count: 100, ...p });
		expect(screen.queryByText('Не больше 100 заказов за раз')).toBeNull();
		await open();
		expect(groups().map((g) => g.name)).toEqual(['Сохранить PDF']);
		await choose(itemIn('Сохранить PDF', 'Наклейка'));
		expect(p.onsave).toHaveBeenCalledWith('sticker');
	});

	it('in the program both sections are there; «Печать» prints', async () => {
		platform.canPrintDirectly.mockReturnValue(true);
		const p = props();
		render(BulkPrintMenu, { count: 3, ...p });
		await open();
		expect(groups().map((g) => g.name)).toEqual(['Печать', 'Сохранить PDF']);
		expect(screen.getAllByRole('menuitem', { hidden: true })).toHaveLength(10);
		await choose(itemIn('Печать', 'Акт приёма'));
		expect(p.onprint).toHaveBeenCalledWith('intake_act');
	});
});
