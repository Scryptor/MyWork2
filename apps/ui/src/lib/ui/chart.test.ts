import { describe, expect, it } from 'vitest';
import { chartOption } from './chart.js';

const colors = { 1: '#aa0001', 2: '#aa0002', 3: '#aa0003' } as const;
const text = { color: '#bb0001', muted: '#bb0002', grid: '#bb0003' };
const format = (v: number) => `${v} ₽`;
const input = {
	categories: ['Янв', 'Фев'],
	series: [
		{ name: 'Выручка', values: [10, 20], kind: 'bar' as const, tone: 1 as const },
		{ name: 'Прибыль', values: [3, 5], kind: 'line' as const, tone: 3 as const }
	],
	format
};

type Opt = {
	series: {
		type: string;
		name: string;
		data: number[];
		itemStyle?: { color: string };
		lineStyle?: { color: string };
	}[];
	xAxis: { type: string; data?: string[] };
	yAxis: { type: string; data?: string[] };
	tooltip: { formatter: (p: unknown) => string };
};
const build = (extra: { horizontal?: boolean } = {}) =>
	chartOption({ ...input, ...extra }, colors, text) as unknown as Opt;

describe('chartOption', () => {
	it('bar and line series keep their type and take the tone colour', () => {
		const o = build();
		expect(o.series.map((s) => s.type)).toEqual(['bar', 'line']);
		expect(o.series[0]!.itemStyle?.color).toBe(colors[1]);
		expect(o.series[1]!.itemStyle?.color).toBe(colors[3]);
		expect(o.series[1]!.lineStyle?.color).toBe(colors[3]);
		expect(o.series[0]!.data).toEqual([10, 20]);
	});

	it('horizontal puts categories on the Y axis', () => {
		const v = build();
		expect(v.xAxis.type).toBe('category');
		expect(v.xAxis.data).toEqual(['Янв', 'Фев']);
		const h = build({ horizontal: true });
		expect(h.yAxis.type).toBe('category');
		expect(h.yAxis.data).toEqual(['Янв', 'Фев']);
		expect(h.xAxis.type).toBe('value');
	});

	it('the tooltip shows values through format', () => {
		const html = build().tooltip.formatter([
			{ axisValueLabel: 'Янв', seriesName: 'Выручка', value: 10, marker: '' },
			{ axisValueLabel: 'Янв', seriesName: 'Прибыль', value: 3, marker: '' }
		]);
		expect(html).toContain('10 ₽');
		expect(html).toContain('3 ₽');
		expect(html).toContain('Выручка');
	});

	it('the tooltip escapes names', () => {
		const html = build().tooltip.formatter([
			{ axisValueLabel: '<u>', seriesName: '<i>', value: 1, marker: '' }
		]);
		expect(html).not.toContain('<u>');
		expect(html).not.toContain('<i>');
	});

	it('has no colour literal other than the ones passed in', () => {
		const json = JSON.stringify(chartOption(input, colors, text));
		const found = json.match(/#[0-9a-fA-F]{3,8}\b|rgba?\(|hsla?\(/g) ?? [];
		const allowed = new Set([...Object.values(colors), ...Object.values(text)]);
		for (const f of found) expect(allowed.has(f)).toBe(true);
		expect(json).toContain(colors[1]);
		expect(json).toContain(text.grid);
	});
});
