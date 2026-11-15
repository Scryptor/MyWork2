import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import Avatar from './Avatar.svelte';

describe('Avatar', () => {
	it('shows initials', () => {
		render(Avatar, { name: 'ИВАНОВ ИВАН' });
		expect(screen.getByText('ИИ')).toBeInTheDocument();
	});
});
