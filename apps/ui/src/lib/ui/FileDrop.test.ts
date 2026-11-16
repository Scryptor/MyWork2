import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { flushSync } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import FileDrop from './FileDrop.svelte';

const image = new File(['x'], 'photo.png', { type: 'image/png' });
const second = new File(['y'], 'IMG_0002.jpg', { type: 'image/jpeg' });
const pdf = new File(['z'], 'act.pdf', { type: 'application/pdf' });

/** jsdom has no DataTransfer: an event with the fields FileDrop reads, then the DOM is updated. */
function fire(target: EventTarget, type: string, fields: Record<string, unknown>): Event {
	const e = new Event(type, { bubbles: true, cancelable: true });
	for (const [key, value] of Object.entries(fields)) Object.defineProperty(e, key, { value });
	target.dispatchEvent(e);
	flushSync();
	return e;
}
const transfer = (files: File[], types = ['Files']) => ({ files, types, dropEffect: 'none' });

afterEach(() => {
	vi.restoreAllMocks();
	document.body.innerHTML = '';
});

describe('FileDrop', () => {
	it('«Выбрать файлы» opens the hidden file input; chosen files that fit accept are reported', async () => {
		const onfiles = vi.fn();
		const { container } = render(FileDrop, { onfiles });
		const input = container.querySelector<HTMLInputElement>('input[type="file"]')!;
		expect(input).not.toBeVisible();
		expect(input).toHaveAttribute('accept', 'image/*');
		expect(input.multiple).toBe(true);
		const click = vi.spyOn(input, 'click').mockImplementation(() => {});

		await userEvent.click(screen.getByRole('button', { name: 'Выбрать файлы' }));
		expect(click).toHaveBeenCalledOnce();

		Object.defineProperty(input, 'files', { value: [image, pdf, second], configurable: true });
		input.dispatchEvent(new Event('change', { bubbles: true }));
		expect(onfiles).toHaveBeenCalledWith([image, second]);
	});

	it('dragging files over lights the zone up; a drop reports the files that fit', () => {
		const onfiles = vi.fn();
		const { container } = render(FileDrop, { onfiles });
		const zone = container.firstElementChild!;
		const hint = zone.querySelector('p')!;

		const enter = fire(zone, 'dragenter', { dataTransfer: transfer([image]) });
		expect(enter.defaultPrevented).toBe(true);
		expect(zone).toHaveAttribute('data-over');
		fire(hint, 'dragenter', { dataTransfer: transfer([image]) });
		fire(hint, 'dragleave', { dataTransfer: transfer([image]) });
		expect(zone).toHaveAttribute('data-over');
		fire(zone, 'dragleave', { dataTransfer: transfer([image]) });
		expect(zone).not.toHaveAttribute('data-over');

		fire(zone, 'dragenter', { dataTransfer: transfer([image, pdf]) });
		const over = fire(zone, 'dragover', { dataTransfer: transfer([image, pdf]) });
		expect(over.defaultPrevented).toBe(true);
		const drop = fire(zone, 'drop', { dataTransfer: transfer([image, pdf]) });
		expect(drop.defaultPrevented).toBe(true);
		expect(zone).not.toHaveAttribute('data-over');
		expect(onfiles).toHaveBeenCalledWith([image]);

		fire(zone, 'drop', { dataTransfer: transfer([pdf]) });
		expect(onfiles).toHaveBeenCalledOnce();
	});

	it('text being dragged does not light the zone up', () => {
		const { container } = render(FileDrop, { onfiles: vi.fn() });
		const zone = container.firstElementChild!;
		fire(zone, 'dragenter', { dataTransfer: transfer([], ['text/plain']) });
		expect(zone).not.toHaveAttribute('data-over');
	});

	it('a pasted image is taken; a paste with text into a field stays a text paste', () => {
		const onfiles = vi.fn();
		render(FileDrop, { onfiles });
		const paste = fire(document.body, 'paste', { clipboardData: transfer([image]) });
		expect(onfiles).toHaveBeenCalledWith([image]);
		expect(paste.defaultPrevented).toBe(true);

		const field = document.createElement('input');
		document.body.append(field);
		const text = fire(field, 'paste', {
			clipboardData: transfer([image], ['text/plain', 'Files'])
		});
		expect(text.defaultPrevented).toBe(false);
		expect(onfiles).toHaveBeenCalledOnce();

		fire(document.body, 'paste', { clipboardData: transfer([pdf]) });
		expect(onfiles).toHaveBeenCalledOnce();
	});

	it('multiple=false takes the first file only; accept can be changed', () => {
		const onfiles = vi.fn();
		const { container } = render(FileDrop, { onfiles, multiple: false, accept: '.pdf,image/*' });
		const zone = container.firstElementChild!;
		fire(zone, 'drop', { dataTransfer: transfer([pdf, image]) });
		expect(onfiles).toHaveBeenCalledWith([pdf]);
		expect(container.querySelector('input[type="file"]')).not.toHaveAttribute('multiple');
	});
});
