import { render, screen } from '@testing-library/svelte';
import { createRawSnippet } from 'svelte';
import { describe, expect, it } from 'vitest';
import Field from './Field.svelte';

const control = createRawSnippet<
	[{ id: string; describedBy: string | undefined; invalid: boolean }]
>((a) => ({
	render: () =>
		`<input id="${a().id}" ${a().describedBy ? `aria-describedby="${a().describedBy}"` : ''} ${a().invalid ? 'aria-invalid="true"' : ''} />`
}));

describe('Field', () => {
	it('links hint and error through aria-describedby', () => {
		render(Field, {
			id: 'x',
			label: 'Имя',
			hint: 'Как в паспорте',
			error: 'Обязательно',
			children: control
		});
		const input = screen.getByLabelText(/Имя/);
		const ids = (input.getAttribute('aria-describedby') ?? '').split(' ');
		expect(ids).toEqual(['x-hint', 'x-error']);
		expect(document.getElementById('x-hint')).toHaveTextContent('Как в паспорте');
		expect(document.getElementById('x-error')).toHaveTextContent('Обязательно');
	});
	it('error marks the control invalid', () => {
		render(Field, { id: 'x', label: 'Имя', error: 'Плохо', children: control });
		expect(screen.getByLabelText(/Имя/)).toHaveAttribute('aria-invalid', 'true');
	});
	it('no hint or error: no aria-describedby, not invalid', () => {
		render(Field, { id: 'x', label: 'Имя', children: control });
		const input = screen.getByLabelText(/Имя/);
		expect(input).not.toHaveAttribute('aria-describedby');
		expect(input).not.toHaveAttribute('aria-invalid');
	});
	it('required adds an asterisk to the label', () => {
		render(Field, { id: 'x', label: 'Имя', required: true, children: control });
		expect(screen.getByText('Имя').closest('label')).toHaveTextContent('Имя *');
	});
});
