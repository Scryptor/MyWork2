<script lang="ts">
	import { BarChart, LineChart } from 'echarts/charts';
	import { GridComponent, LegendComponent, TooltipComponent } from 'echarts/components';
	import { init, use, type EChartsType } from 'echarts/core';
	import { SVGRenderer } from 'echarts/renderers';
	import { prefs } from '#lib/design/prefs.svelte.js';
	import { chartOption, type ChartSeries } from './chart.js';

	use([BarChart, LineChart, GridComponent, TooltipComponent, LegendComponent, SVGRenderer]);

	let {
		label,
		categories,
		series,
		horizontal = false,
		format,
		height = 280
	}: {
		/** What the chart shows: read aloud in place of the picture. */
		label: string;
		categories: string[];
		series: ChartSeries[];
		horizontal?: boolean;
		format: (value: number) => string;
		height?: number;
	} = $props();

	let host = $state<HTMLDivElement | null>(null);
	let chart: EChartsType | null = null;

	function token(style: CSSStyleDeclaration, name: string): string {
		return style.getPropertyValue(name).trim();
	}

	$effect(() => {
		if (!host) return;
		const c = init(host, undefined, { renderer: 'svg' });
		chart = c;
		const observer = new ResizeObserver(() => c.resize());
		observer.observe(host);
		return () => {
			observer.disconnect();
			c.dispose();
			chart = null;
		};
	});

	$effect(() => {
		// The resolved theme: the tokens change under the chart, so read them again.
		void prefs.resolvedTheme;
		const input = { categories, series, horizontal, format };
		if (!chart || !host) return;
		const style = getComputedStyle(host);
		const option = chartOption(
			input,
			{ 1: token(style, '--chart-1'), 2: token(style, '--chart-2'), 3: token(style, '--chart-3') },
			{
				color: token(style, '--text'),
				muted: token(style, '--text-3'),
				grid: token(style, '--border')
			}
		) as { tooltip: Record<string, unknown> };
		option.tooltip = {
			...option.tooltip,
			backgroundColor: token(style, '--surface'),
			borderColor: token(style, '--border-strong')
		};
		chart.setOption(option, true);
	});
</script>

<figure class="chart" role="img" aria-label={label} style:height="{height}px">
	<div class="host" bind:this={host}></div>
</figure>

<style>
	.chart {
		margin: 0;
		min-width: 0;
	}
	.host {
		width: 100%;
		height: 100%;
	}
</style>
