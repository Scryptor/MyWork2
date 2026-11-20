import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeAll, describe, expect, it } from 'vitest';
import Select from './Select.svelte';

// What bits-ui calls on a pointer press and jsdom does not have.
beforeAll(() => {
	Element.prototype.hasPointerCapture ??= () => false;
	Element.prototype.releasePointerCapture ??= () => {};
	Element.prototype.scrollIntoView ??= () => {};
});

/**
 * Opened from the keyboard. jsdom does not place the list, so it stays `visibility: hidden`: the queries
 * take `hidden: true`, and the options are found by their text (a hidden one has no accessible name).
 */
async function open(box: HTMLElement) {
	box.focus();
	await userEvent.keyboard('{Enter}');
}
const hidden = { hidden: true } as const;
const texts = (options: HTMLElement[]) => options.map((o) => o.textContent?.trim());
/** A group by the text of the heading that names it (`aria-labelledby`). */
function group(name: string): HTMLElement {
	const found = [...document.querySelectorAll<HTMLElement>('[role="group"]')].find(
		(g) => document.getElementById(g.getAttribute('aria-labelledby') ?? '')?.textContent === name
	);
	if (!found) throw new Error(`no group «${name}»`);
	return found;
}
function option(text: string, within: HTMLElement = document.body): HTMLElement {
	const found = [...within.querySelectorAll<HTMLElement>('[role="option"]')].find(
		(o) => o.textContent?.trim() === text
	);
	if (!found) throw new Error(`no option «${text}»`);
	return found;
}

const GROUPS = [
	{
		label: 'Россия',
		options: [
			{ value: 'Europe/Moscow', label: '(UTC+03:00) Москва' },
			{ value: 'Europe/Samara', label: '(UTC+04:00) Самара' }
		]
	},
	{
		label: 'Европа',
		options: [{ value: 'Europe/Berlin', label: '(UTC+01:00) Берлин' }]
	}
];

describe('Select', () => {
	it('a plain list as before', async () => {
		let value: string | null = 'a';
		render(Select<string>, {
			label: 'Буква',
			items: [
				{ value: 'a', label: 'А' },
				{ value: 'b', label: 'Б' }
			],
			get value() {
				return value;
			},
			set value(v) {
				value = v;
			}
		});
		const box = screen.getByRole('combobox', { name: 'Буква' });
		expect(box).toHaveTextContent('А');
		await open(box);
		await screen.findAllByRole('option', hidden);
		await userEvent.click(option('Б'));
		expect(value).toBe('b');
		expect(screen.queryByRole('group', hidden)).not.toBeInTheDocument();
	});

	it('groups: a heading over the options of each, the value shown by its label', async () => {
		let value: string | null = 'Europe/Moscow';
		render(Select<string>, {
			label: 'Часовой пояс',
			items: GROUPS,
			get value() {
				return value;
			},
			set value(v) {
				value = v;
			}
		});
		const box = screen.getByRole('combobox', { name: 'Часовой пояс' });
		expect(box).toHaveTextContent('(UTC+03:00) Москва');
		await open(box);
		await screen.findAllByRole('group', hidden);
		const russia = group('Россия');
		expect(texts(within(russia).getAllByRole('option', hidden))).toEqual([
			'(UTC+03:00) Москва',
			'(UTC+04:00) Самара'
		]);
		const europe = group('Европа');
		await userEvent.click(option('(UTC+01:00) Берлин', europe));
		expect(value).toBe('Europe/Berlin');
	});
});
