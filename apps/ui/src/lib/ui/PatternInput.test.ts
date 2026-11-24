import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import Harness from './PatternInputHarness.test-helper.svelte';

const out = () => screen.getByTestId('out').textContent;
const point = (n: number) => screen.getByRole('button', { name: `Точка ${n}` });

async function press(...points: number[]) {
	for (const n of points) await userEvent.click(point(n));
}

describe('PatternInput', () => {
	it('nine points; clicks make the pattern, a step over a point takes it too', async () => {
		render(Harness);
		expect(screen.getAllByRole('button', { name: /^Точка \d$/ })).toHaveLength(9);
		await press(1, 3, 6, 9);
		expect(out()).toBe('12369');
		expect(screen.getByText('1 · 2 · 3 · 6 · 9')).toBeInTheDocument();
		expect(screen.getByText('Ключ готов')).toBeInTheDocument();
	});

	it('the chosen points are pressed and show their place in the pattern', async () => {
		render(Harness);
		await press(5, 1);
		expect(point(5)).toHaveAttribute('aria-pressed', 'true');
		expect(point(1)).toHaveAttribute('aria-pressed', 'true');
		expect(point(9)).toHaveAttribute('aria-pressed', 'false');
		expect(point(5)).toHaveTextContent('1');
		expect(point(1)).toHaveTextContent('2');
		expect(point(9)).toHaveTextContent('');
	});

	it('until four points it says how many are left', async () => {
		render(Harness);
		await press(1);
		expect(screen.getByText('Ещё 3 точки')).toBeInTheDocument();
		await press(5, 9);
		expect(screen.getByText('Ещё 1 точка')).toBeInTheDocument();
		expect(screen.queryByText('Ключ готов')).toBeNull();
	});

	it('«Отменить шаг» takes the last point back, «Сбросить» empties the pattern', async () => {
		render(Harness);
		await press(1, 3, 6, 9);
		await userEvent.click(screen.getByRole('button', { name: 'Отменить шаг' }));
		expect(out()).toBe('1236');
		expect(point(9)).toHaveAttribute('aria-pressed', 'false');
		await userEvent.click(screen.getByRole('button', { name: 'Сбросить' }));
		expect(out()).toBe('');
		expect(screen.getByRole('button', { name: 'Отменить шаг' })).toBeDisabled();
		expect(screen.getByRole('button', { name: 'Сбросить' })).toBeDisabled();
	});

	it('shows a pattern it is given', () => {
		render(Harness, { value: '1596' });
		expect(screen.getByText('1 · 5 · 9 · 6')).toBeInTheDocument();
		expect(point(6)).toHaveTextContent('4');
	});

	it('disabled: nothing can be pressed', async () => {
		render(Harness, { value: '1596', disabled: true });
		expect(point(2)).toBeDisabled();
		expect(screen.getByRole('button', { name: 'Сбросить' })).toBeDisabled();
		expect(out()).toBe('1596');
	});
});
