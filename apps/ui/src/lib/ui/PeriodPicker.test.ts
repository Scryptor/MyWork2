import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import PeriodPicker from './PeriodPicker.svelte';

describe('PeriodPicker', () => {
	it('a quick period reports its kind without dates', async () => {
		const onchange = vi.fn();
		render(PeriodPicker, { kind: 'month', from: null, to: null, onchange });
		await userEvent.click(screen.getByRole('radio', { name: 'Прошлый месяц' }));
		expect(onchange).toHaveBeenCalledWith('last_month', null, null);
	});

	it('marks the current kind as checked for the screen reader', () => {
		render(PeriodPicker, { kind: 'week', from: null, to: null, onchange: vi.fn() });
		const group = screen.getByRole('radiogroup');
		expect(within(group).getAllByRole('radio')).toHaveLength(7);
		expect(within(group).getByRole('radio', { name: 'Неделя' })).toBeChecked();
		expect(within(group).getByRole('radio', { name: 'Год' })).not.toBeChecked();
	});

	it('hides the date fields for a quick period', () => {
		render(PeriodPicker, { kind: 'year', from: null, to: null, onchange: vi.fn() });
		expect(screen.queryByLabelText('С')).toBeNull();
	});

	it('«Свой период» shows the two fields', async () => {
		const onchange = vi.fn();
		render(PeriodPicker, { kind: 'custom', from: '2026-11-01', to: '2026-11-25', onchange });
		expect(screen.getByLabelText('С')).toHaveValue('01.11.2026');
		expect(screen.getByLabelText('По')).toHaveValue('25.11.2026');
		await userEvent.click(screen.getByRole('radio', { name: 'Год' }));
		expect(onchange).toHaveBeenCalledWith('year', null, null);
	});

	it('choosing «Свой период» reports the current dates', async () => {
		const onchange = vi.fn();
		render(PeriodPicker, { kind: 'month', from: '2026-11-01', to: '2026-11-25', onchange });
		await userEvent.click(screen.getByRole('radio', { name: 'Свой период' }));
		expect(onchange).toHaveBeenCalledWith('custom', '2026-11-01', '2026-11-25');
	});

	it('flags an end earlier than the start', () => {
		render(PeriodPicker, {
			kind: 'custom',
			from: '2026-11-20',
			to: '2026-11-10',
			onchange: vi.fn()
		});
		expect(screen.getByText('Начало периода позже конца')).toBeInTheDocument();
	});

	it('editing a date reports custom with both dates', async () => {
		const onchange = vi.fn();
		render(PeriodPicker, { kind: 'custom', from: '2026-11-01', to: '2026-11-25', onchange });
		const to = screen.getByLabelText('По');
		await userEvent.clear(to);
		await userEvent.type(to, '20.11.2026');
		await userEvent.tab();
		expect(onchange).toHaveBeenLastCalledWith('custom', '2026-11-01', '2026-11-20');
	});
});
