import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { DEFAULT_PRINT_SETTINGS, printSettings } from '#lib/print/settings.svelte.js';

// «Настройки → Печать»: kept on this computer. In a browser it explains that printing goes through a
// PDF and has only what shapes the document; the program has the printers and the copies too. The
// platform is replaced here: the program is not needed to see its screen.
const platform = vi.hoisted(() => ({
	direct: false,
	canPrintDirectly: vi.fn(),
	listPrinters: vi.fn(),
	printFormat: vi.fn()
}));
vi.mock('#lib/print/platform.js', () => ({
	canPrintDirectly: platform.canPrintDirectly,
	listPrinters: platform.listPrinters,
	printFormat: platform.printFormat
}));

import Page from './PrintSettingsHarness.test-helper.svelte';

// What bits-ui calls on a pointer press and jsdom does not have.
beforeAll(() => {
	Element.prototype.hasPointerCapture ??= () => false;
	Element.prototype.releasePointerCapture ??= () => {};
	Element.prototype.scrollIntoView ??= () => {};
});

beforeEach(() => {
	platform.direct = false;
	platform.canPrintDirectly.mockReset();
	platform.canPrintDirectly.mockImplementation(() => platform.direct);
	platform.listPrinters.mockReset();
	platform.printFormat.mockReset();
	platform.printFormat.mockResolvedValue('pdf');
	platform.listPrinters.mockResolvedValue([
		{ name: 'Акты HP', is_default: true },
		{ name: 'Xprinter XP-365B', is_default: false }
	]);
});

afterEach(() => {
	printSettings.set(DEFAULT_PRINT_SETTINGS);
	localStorage.clear();
});

const textbox = (name: string) => screen.getByRole('textbox', { name });
const optionTexts = () =>
	[...document.querySelectorAll('[role="option"]')].map((o) => o.textContent?.trim());
function option(text: string): HTMLElement {
	const found = [...document.querySelectorAll<HTMLElement>('[role="option"]')].find(
		(o) => o.textContent?.trim() === text
	);
	if (!found) throw new Error(`no option «${text}»`);
	return found;
}
/** The list is opened from the keyboard; jsdom does not place it, so it is found with `hidden`. */
async function openList(name: string) {
	const box = screen.getByRole('combobox', { name });
	box.focus();
	await userEvent.keyboard('{Enter}');
	await screen.findAllByRole('option', { hidden: true });
	return box;
}

describe('in a browser', () => {
	it('explains printing through a PDF; no printers and no copies, no question to the program', () => {
		render(Page);
		expect(screen.getByText('Печать из браузера — через PDF')).toBeVisible();
		expect(screen.queryByRole('combobox', { name: 'Принтер актов' })).toBeNull();
		expect(screen.queryByRole('combobox', { name: 'Принтер наклеек' })).toBeNull();
		expect(screen.queryByRole('textbox', { name: 'Копий акта' })).toBeNull();
		expect(platform.listPrinters).not.toHaveBeenCalled();
		// What shapes the PDF of the sticker is here.
		expect(screen.getByRole('radio', { name: '58 × 40 мм' })).toBeChecked();
		expect(textbox('Смещение вправо, мм')).toHaveValue('0');
		expect(screen.getByRole('radio', { name: '203' })).toBeChecked();
	});

	it('a size of the list is saved at once', async () => {
		render(Page);
		await userEvent.click(screen.getByRole('radio', { name: '40 × 30 мм' }));
		expect(printSettings.labelMm).toEqual([40, 30]);
		expect(screen.queryByRole('textbox', { name: 'Ширина, мм' })).toBeNull();
	});

	it('«Свой» size: two fields in mm from the current size; a side outside 15..200 is a mistake and is not kept', async () => {
		render(Page);
		await userEvent.click(screen.getByRole('radio', { name: 'Свой' }));
		expect(textbox('Ширина, мм')).toHaveValue('58');
		expect(textbox('Высота, мм')).toHaveValue('40');
		await userEvent.clear(textbox('Ширина, мм'));
		await userEvent.type(textbox('Ширина, мм'), '70,5');
		expect(printSettings.labelMm).toEqual([70.5, 40]);

		await userEvent.clear(textbox('Высота, мм'));
		await userEvent.type(textbox('Высота, мм'), '300');
		await userEvent.tab();
		expect(textbox('Высота, мм')).toHaveAccessibleDescription(/От 15 до 200 мм/);
		expect(textbox('Высота, мм')).toHaveAttribute('aria-invalid', 'true');
		expect(printSettings.labelMm).toEqual([70.5, 40]);

		await userEvent.clear(textbox('Высота, мм'));
		await userEvent.type(textbox('Высота, мм'), '50');
		expect(textbox('Высота, мм')).not.toHaveAttribute('aria-invalid');
		expect(printSettings.labelMm).toEqual([70.5, 50]);
	});

	it('a saved size not of the list opens as «Свой»', () => {
		printSettings.set({ labelMm: [100, 150] });
		render(Page);
		expect(screen.getByRole('radio', { name: 'Свой' })).toBeChecked();
		expect(textbox('Ширина, мм')).toHaveValue('100');
		expect(textbox('Высота, мм')).toHaveValue('150');
	});

	it('the offset takes −10..10 mm, a comma and a minus; more is a mistake', async () => {
		render(Page);
		await userEvent.clear(textbox('Смещение вниз, мм'));
		await userEvent.type(textbox('Смещение вниз, мм'), '-1,5');
		expect(printSettings.offsetMm).toEqual([0, -1.5]);
		await userEvent.clear(textbox('Смещение вправо, мм'));
		await userEvent.type(textbox('Смещение вправо, мм'), '11');
		await userEvent.tab();
		expect(textbox('Смещение вправо, мм')).toHaveAccessibleDescription(/От −10 до 10 мм/);
		expect(printSettings.offsetMm).toEqual([0, -1.5]);
	});

	it('a word in a number field is «Введите число»', async () => {
		render(Page);
		await userEvent.clear(textbox('Смещение вправо, мм'));
		await userEvent.type(textbox('Смещение вправо, мм'), 'abc');
		await userEvent.tab();
		expect(textbox('Смещение вправо, мм')).toHaveAccessibleDescription(/Введите число/);
		expect(printSettings.offsetMm).toEqual([0, 0]);
	});

	it('the printer’s resolution is one of 203, 300 and 600', async () => {
		render(Page);
		await userEvent.click(screen.getByRole('radio', { name: '300' }));
		expect(printSettings.dpi).toBe(300);
	});
});

describe('in the program', () => {
	beforeEach(() => {
		platform.direct = true;
	});

	it('the printers of this computer, «Принтер по умолчанию» first; no word about the browser', async () => {
		render(Page);
		expect(screen.queryByText('Печать из браузера — через PDF')).toBeNull();
		await waitFor(() => expect(platform.listPrinters).toHaveBeenCalledOnce());
		const box = screen.getByRole('combobox', { name: 'Принтер актов' });
		expect(box).toHaveTextContent('Принтер по умолчанию');
		await waitFor(async () => {
			await openList('Принтер актов');
			expect(optionTexts()).toEqual([
				'Принтер по умолчанию',
				'Акты HP (по умолчанию в системе)',
				'Xprinter XP-365B'
			]);
		});
		await userEvent.click(option('Xprinter XP-365B'));
		expect(printSettings.actPrinter).toBe('Xprinter XP-365B');
		expect(printSettings.labelPrinter).toBeNull();
	});

	it('«Принтер по умолчанию» puts the choice back to the system’s', async () => {
		printSettings.set({ labelPrinter: 'Xprinter XP-365B' });
		render(Page);
		const box = screen.getByRole('combobox', { name: 'Принтер наклеек' });
		await waitFor(() => expect(box).toHaveTextContent('Xprinter XP-365B'));
		await openList('Принтер наклеек');
		await userEvent.click(option('Принтер по умолчанию'));
		expect(printSettings.labelPrinter).toBeNull();
	});

	it('a chosen printer that is gone stays chosen, marked so', async () => {
		printSettings.set({ actPrinter: 'Старый Canon' });
		render(Page);
		const box = screen.getByRole('combobox', { name: 'Принтер актов' });
		await waitFor(() => expect(box).toHaveTextContent('Старый Canon (не найден)'));
		expect(printSettings.actPrinter).toBe('Старый Canon');
	});

	it('the list not read: the error of the program over the printers, the choice as it was', async () => {
		platform.listPrinters.mockRejectedValue(new Error('lpstat не ответил'));
		printSettings.set({ actPrinter: 'Акты HP' });
		render(Page);
		expect(
			await screen.findByText('Не удалось получить список принтеров: lpstat не ответил')
		).toBeVisible();
		expect(screen.getByRole('combobox', { name: 'Принтер актов' })).toHaveTextContent(/^Акты HP$/);
		expect(printSettings.actPrinter).toBe('Акты HP');
	});

	it('copies of the act and of the sticker, 1..99', async () => {
		render(Page);
		await userEvent.clear(textbox('Копий наклейки'));
		await userEvent.type(textbox('Копий наклейки'), '3');
		expect(printSettings.labelCopies).toBe(3);
		await userEvent.clear(textbox('Копий акта'));
		await userEvent.type(textbox('Копий акта'), '100');
		await userEvent.tab();
		expect(textbox('Копий акта')).toHaveAccessibleDescription(/От 1 до 99/);
		expect(printSettings.actCopies).toBe(1);
		await userEvent.clear(textbox('Копий акта'));
		await userEvent.type(textbox('Копий акта'), '1,5');
		await userEvent.tab();
		expect(textbox('Копий акта')).toHaveAccessibleDescription(/Введите целое число/);
		expect(printSettings.actCopies).toBe(1);
	});
});
