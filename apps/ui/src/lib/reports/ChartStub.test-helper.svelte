<!-- Stands in for the chart in unit tests: shows what it was given. -->
<script lang="ts">
	let {
		label,
		categories,
		series
	}: { label: string; categories: string[]; series: { name: string; values: number[] }[] } =
		$props();
</script>

<div role="img" aria-label={label} data-categories={categories.join('|')}>
	{series.map((s) => s.name).join(',')}
</div>
