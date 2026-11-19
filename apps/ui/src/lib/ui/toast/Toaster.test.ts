import { render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { afterEach, describe, expect, it } from 'vitest';
import Toaster from './ToasterHarness.test-helper.svelte';
import { toast } from './toast.svelte.js';

afterEach(() => {
	for (const t of [...toast.items]) toast.dismiss(t.id);
});

const press = (el: Element) => {
	const e = new MouseEvent('mousedown', { bubbles: true, cancelable: true });
	el.dispatchEvent(e);
	return e;
};

describe('Toaster', () => {
	it('a press on a toast keeps the focus where it was; a press beside the toasts is the page’s (text selection)', async () => {
		render(Toaster);
		toast.show({ message: 'Сохранено' });
		await tick();
		const region = screen.getByRole('status', { name: 'Уведомления' });
		expect(press(screen.getByText('Сохранено')).defaultPrevented).toBe(true);
		expect(press(region).defaultPrevented).toBe(false);
	});
});
