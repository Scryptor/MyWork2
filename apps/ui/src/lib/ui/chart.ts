import type { EChartsCoreOption } from 'echarts/core';

export type ChartSeries = {
	name: string;
	values: number[];
	kind: 'bar' | 'line';
	tone: 1 | 2 | 3;
};

export type ChartInput = {
	categories: string[];
	series: ChartSeries[];
	horizontal?: boolean;
	format: (value: number) => string;
};

export type ChartColors = Record<1 | 2 | 3, string>;
export type ChartText = { color: string; muted: string; grid: string };

function escapeHtml(s: string): string {
	return s
		.replaceAll('&', '&amp;')
		.replaceAll('<', '&lt;')
		.replaceAll('>', '&gt;')
		.replaceAll('"', '&quot;')
		.replaceAll("'", '&#39;');
}

type TooltipItem = {
	axisValueLabel?: string;
	seriesName?: string;
	value?: unknown;
	marker?: string;
};

/**
 * The ECharts option for a bar/line chart. Pure: colours come in as arguments (the caller reads the
 * `--chart-*` tokens), so the option holds no colour literal of its own.
 */
export function chartOption(
	input: ChartInput,
	colors: ChartColors,
	text: ChartText
): EChartsCoreOption {
	const { categories, series, horizontal = false, format } = input;
	const category = {
		type: 'category' as const,
		data: categories,
		axisLine: { lineStyle: { color: text.grid } },
		axisTick: { show: false },
		axisLabel: { color: text.muted },
		...(horizontal ? { inverse: true } : {})
	};
	const value = {
		type: 'value' as const,
		axisLabel: { color: text.muted, formatter: (v: number) => format(v) },
		splitLine: { lineStyle: { color: text.grid } }
	};
	return {
		animation: false,
		textStyle: { color: text.color },
		grid: { left: 8, right: 16, top: series.length > 1 ? 40 : 16, bottom: 8, containLabel: true },
		legend: {
			show: series.length > 1,
			top: 0,
			textStyle: { color: text.color }
		},
		tooltip: {
			trigger: 'axis',
			axisPointer: { type: horizontal ? 'shadow' : 'line' },
			textStyle: { color: text.color },
			formatter: (params: TooltipItem | TooltipItem[]) => {
				const items = Array.isArray(params) ? params : [params];
				if (items.length === 0) return '';
				const head = `<div>${escapeHtml(String(items[0]!.axisValueLabel ?? ''))}</div>`;
				const rows = items.map(
					(p) =>
						`<div>${p.marker ?? ''}${escapeHtml(String(p.seriesName ?? ''))}: <b>${escapeHtml(format(Number(p.value)))}</b></div>`
				);
				return head + rows.join('');
			}
		},
		xAxis: horizontal ? value : category,
		yAxis: horizontal ? category : value,
		series: series.map((s) => {
			const color = colors[s.tone];
			return s.kind === 'bar'
				? { type: 'bar' as const, name: s.name, data: s.values, itemStyle: { color } }
				: {
						type: 'line' as const,
						name: s.name,
						data: s.values,
						itemStyle: { color },
						lineStyle: { color },
						symbolSize: 6
					};
		})
	};
}
