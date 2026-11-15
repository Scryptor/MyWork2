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
});
