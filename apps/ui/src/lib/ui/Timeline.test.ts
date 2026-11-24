import Warehouse from '@lucide/svelte/icons/warehouse';
import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import Timeline from './Timeline.svelte';

const now = { year: 2026, month: 11, day: 15, hour: 12, minute: 0, second: 0 };

describe('Timeline', () => {
	it('shows relative time', () => {
		render(Timeline, { items: [{ id: 1, at: '2026-11-15T10:30:00', text: 'Принят' }], now });
		expect(screen.getByText('сегодня 10:30')).toBeInTheDocument();
	});
	it('shows emptyText for no items', () => {
		render(Timeline, { items: [], now, emptyText: 'Пока пусто' });
		expect(screen.getByText('Пока пусто')).toBeInTheDocument();
	});
	it('shows the icon of an entry before its text, hidden from readers', () => {
		const { container } = render(Timeline, {
			items: [{ id: 1, at: '2026-11-15T10:30:00', text: 'Списано со склада', icon: Warehouse }],
			now
		});
		expect(screen.getByText('Списано со склада')).toBeInTheDocument();
		expect(container.querySelector('svg[aria-hidden="true"]')).not.toBeNull();
	});
});
