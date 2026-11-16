import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import TextInput from './TextInput.svelte';

describe('TextInput', () => {
	it('takes an aria-label without a visible label', () => {
		render(TextInput, { value: '', ariaLabel: 'Фильтр списка' });
		expect(screen.getByRole('textbox', { name: 'Фильтр списка' })).toBeInTheDocument();
	});
});
