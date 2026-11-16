import Ellipsis from '@lucide/svelte/icons/ellipsis';
import Gauge from '@lucide/svelte/icons/gauge';
import Plus from '@lucide/svelte/icons/plus';
import ScanLine from '@lucide/svelte/icons/scan-line';
import Users from '@lucide/svelte/icons/users';
import Wrench from '@lucide/svelte/icons/wrench';
import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import BottomNav from './BottomNav.svelte';
import type { NavItem } from './nav.js';

const ORDERS: NavItem = { id: 'orders', label: 'Заказы', icon: Wrench, href: '#orders' };
const CLIENTS: NavItem = { id: 'clients', label: 'Клиенты', icon: Users, href: '#clients' };
const SCAN: NavItem = { id: 'scan', label: 'Скан', icon: ScanLine, href: '#scan' };
const MORE: NavItem = { id: 'more', label: 'Ещё', icon: Ellipsis, href: '#more' };
const REPORTS: NavItem = { id: 'reports', label: 'Отчёты', icon: Gauge, href: '#reports' };

function labels(): string[] {
	const nav = screen.getByRole('navigation', { name: 'Основная навигация' });
	return [...nav.children].map((el) => el.textContent?.trim() ?? '');
}

describe('BottomNav', () => {
	it('puts the main action «＋» in the middle (spec 5.6)', async () => {
		const onclick = vi.fn();
		render(BottomNav, {
			items: [ORDERS, CLIENTS, SCAN, MORE],
			active: 'orders',
			primary: { label: 'Принять', icon: Plus, onclick }
		});
		expect(labels()).toEqual(['Заказы', 'Клиенты', 'Принять', 'Скан', 'Ещё']);
		expect(screen.getByRole('link', { name: 'Заказы' })).toHaveAttribute('aria-current', 'page');
		await userEvent.click(screen.getByRole('button', { name: 'Принять' }));
		expect(onclick).toHaveBeenCalledOnce();
	});

	it('an odd number of items: the bigger half goes first', () => {
		render(BottomNav, {
			items: [ORDERS, CLIENTS, REPORTS],
			active: 'clients',
			primary: { label: 'Принять', icon: Plus, onclick: vi.fn() }
		});
		expect(labels()).toEqual(['Заказы', 'Клиенты', 'Принять', 'Отчёты']);
	});

	it('an item without href is a button that calls onnavigate', async () => {
		const onnavigate = vi.fn();
		render(BottomNav, {
			items: [ORDERS, { id: 'more', label: 'Ещё', icon: Ellipsis }],
			active: 'orders',
			onnavigate
		});
		expect(screen.queryByRole('link', { name: 'Ещё' })).toBeNull();
		await userEvent.click(screen.getByRole('button', { name: 'Ещё' }));
		expect(onnavigate).toHaveBeenCalledWith('more');
	});

	it('without a main action only the items', () => {
		render(BottomNav, { items: [ORDERS, CLIENTS], active: 'orders' });
		expect(labels()).toEqual(['Заказы', 'Клиенты']);
	});
});
