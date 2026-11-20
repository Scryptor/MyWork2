import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeAll, describe, expect, it, vi } from 'vitest';
import type { MenuItem } from './DropdownMenu.svelte';
import Harness from './DropdownMenuHarness.test-helper.svelte';

// What bits-ui calls on a pointer press and jsdom does not have.
beforeAll(() => {
	Element.prototype.hasPointerCapture ??= () => false;
	Element.prototype.releasePointerCapture ??= () => {};
	Element.prototype.scrollIntoView ??= () => {};
});

const item = (label: string): MenuItem => ({ label, onselect: vi.fn() });
const ITEMS: MenuItem[] = [
	item('Ждёт запчасть'),
	{ heading: 'Выданные по статусам' },
	item('Выдан без ремонта'),
	item('Выдан по гарантии'),
	{ separator: true },
	item('Отменён')
];

/** Opened from the keyboard; jsdom does not place the menu, so it is found with `hidden`. */
async function open() {
	render(Harness, { items: ITEMS });
	screen.getByRole('button', { name: 'Ещё' }).focus();
	await userEvent.keyboard('{Enter}');
	return screen.findByRole('menu', { hidden: true });
}
const labels = (root: HTMLElement) =>
	[...root.querySelectorAll('[role="menuitem"]')].map((i) => i.textContent?.trim());

describe('DropdownMenu headings', () => {
	it('a heading names the group of the items after it, up to a separator', async () => {
		const menu = await open();
		const groups = [...menu.querySelectorAll<HTMLElement>('[role="group"]')];
		expect(groups).toHaveLength(1);
		const group = groups[0]!;
		const heading = document.getElementById(group.getAttribute('aria-labelledby') ?? '');
		expect(heading).toHaveTextContent('Выданные по статусам');
		expect(group).toContainElement(heading);
		expect(labels(group)).toEqual(['Выдан без ремонта', 'Выдан по гарантии']);
		expect(labels(menu)).toEqual([
			'Ждёт запчасть',
			'Выдан без ремонта',
			'Выдан по гарантии',
			'Отменён'
		]);
		expect(heading).not.toHaveAttribute('role', 'menuitem');
	});

	it('arrow keys go over the heading: from the item before it to the first one of its group', async () => {
		await open();
		await waitFor(() => expect(document.activeElement).toHaveTextContent('Ждёт запчасть'));
		await userEvent.keyboard('{ArrowDown}');
		expect(document.activeElement).toHaveTextContent('Выдан без ремонта');
		await userEvent.keyboard('{ArrowUp}');
		expect(document.activeElement).toHaveTextContent('Ждёт запчасть');
	});
});
