import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { createRawSnippet } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import Button from './Button.svelte';
import IconButton from './IconButtonHarness.test-helper.svelte';
import Phone from '@lucide/svelte/icons/phone';

const label = createRawSnippet(() => ({ render: () => '<span>Сохранить</span>' }));

describe('Button', () => {
	it('busy sets aria-busy, swallows clicks and shows a spinner', async () => {
		const onclick = vi.fn();
		render(Button, { busy: true, onclick, children: label });
		const btn = screen.getByRole('button');
		expect(btn).toHaveAttribute('aria-busy', 'true');
		await userEvent.click(btn);
		expect(onclick).not.toHaveBeenCalled();
		expect(btn.querySelector('[role="status"]')).not.toBeNull();
	});
	it('disabled sets the attribute', () => {
		render(Button, { disabled: true, children: label });
		expect(screen.getByRole('button')).toBeDisabled();
	});
	it('shortcut renders kbd', () => {
		const { container } = render(Button, { shortcut: 'N', children: label });
		expect(container.querySelector('kbd')).not.toBeNull();
	});
	it('href renders a link', () => {
		render(Button, { href: '/x', children: label });
		expect(screen.getByRole('link')).toHaveAttribute('href', '/x');
	});
});

describe('IconButton', () => {
	it('has aria-label equal to label', () => {
		render(IconButton, { label: 'Позвонить', icon: Phone });
		expect(screen.getByRole('button', { name: 'Позвонить' })).toHaveAttribute(
			'aria-label',
			'Позвонить'
		);
	});
});

describe('Button rest attributes', () => {
	it('passes other attributes to the <button> so it can be a trigger target', () => {
		render(Button, { children: label, 'aria-haspopup': 'menu', 'data-state': 'closed' });
		const btn = screen.getByRole('button');
		expect(btn).toHaveAttribute('aria-haspopup', 'menu');
		expect(btn).toHaveAttribute('data-state', 'closed');
	});
	it('iconEnd renders after the text', () => {
		const { container } = render(Button, { children: label, iconEnd: Phone });
		expect(container.querySelector('svg')).not.toBeNull();
	});
});
