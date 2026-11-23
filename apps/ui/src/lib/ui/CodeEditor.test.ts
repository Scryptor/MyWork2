import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import CodeEditorHarness from './CodeEditorHarness.test-helper.svelte';

const gutterLines = (c: HTMLElement) => [...c.querySelectorAll('.gutter > span')];

describe('CodeEditor', () => {
	it('numbers the lines and follows new ones', async () => {
		const { container } = render(CodeEditorHarness, { value: 'a\nb\nc' });
		expect(gutterLines(container).map((l) => l.textContent)).toEqual(['1', '2', '3']);
		const box = screen.getByRole('textbox', { name: 'Код' });
		await fireEvent.input(box, { target: { value: 'a\nb\nc\n' } });
		expect(gutterLines(container).map((l) => l.textContent)).toEqual(['1', '2', '3', '4']);
	});

	it('shows one line for an empty value', () => {
		const { container } = render(CodeEditorHarness, { value: '' });
		expect(gutterLines(container).map((l) => l.textContent)).toEqual(['1']);
	});

	it('marks the line with an error and lists it', () => {
		const { container } = render(CodeEditorHarness, {
			value: 'a\nb\nc',
			errors: [{ line: 2, column: 3, message: 'unknown variable: x' }]
		});
		const marked = gutterLines(container).filter((l) => l.hasAttribute('data-error'));
		expect(marked.map((l) => l.textContent)).toEqual(['2']);
		expect(screen.getByText('Строка 2, знак 3: unknown variable: x')).toBeVisible();
		const box = screen.getByRole('textbox', { name: 'Код' });
		expect(box).toHaveAttribute('aria-invalid', 'true');
		const list = container.querySelector('ul')!;
		expect(box.getAttribute('aria-describedby')).toContain(list.id);
	});

	it('is valid without errors and keeps the hint in describedby', () => {
		render(CodeEditorHarness, { value: 'a', hint: 'Подсказка' });
		const box = screen.getByRole('textbox', { name: 'Код' });
		expect(box).not.toHaveAttribute('aria-invalid');
		expect(box.getAttribute('aria-describedby')).toMatch(/-hint$/);
	});

	it('turns off the helpers of a prose field', () => {
		render(CodeEditorHarness, { value: '' });
		const box = screen.getByRole('textbox', { name: 'Код' });
		expect(box).toHaveAttribute('spellcheck', 'false');
		expect(box).toHaveAttribute('wrap', 'off');
		expect(box).toHaveAttribute('autocapitalize', 'off');
		expect(box).toHaveAttribute('autocomplete', 'off');
	});
});
