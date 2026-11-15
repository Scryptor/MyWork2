import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import StatusBadge from './StatusBadge.svelte';

describe('StatusBadge', () => {
	it('maps a hue to its class and shows the name', () => {
		render(StatusBadge, { name: 'Согласовано', color: 'violet' });
		const el = screen.getByText('Согласовано').closest('.badge');
		expect(el).toHaveClass('st-violet');
	});
	it('falls back to gray for a color literal', () => {
		render(StatusBadge, { name: 'X', color: '#ff0000' });
		expect(screen.getByText('X').closest('.badge')).toHaveClass('st-gray');
	});
});
