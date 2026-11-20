<!-- CatalogRows for unit tests: the tooltips of its buttons need their provider; the extra columns of
     «Статусы» (colour, kind) as plain cells. -->
<script lang="ts">
	import { Tooltip } from 'bits-ui';
	import CatalogRows from './CatalogRows.svelte';

	type Row = { id: number; label: string; archived: boolean; count: number | null };
	let {
		rows,
		readonly = false,
		withColumns = true,
		countTitle
	}: { rows: Row[]; readonly?: boolean; withColumns?: boolean; countTitle?: string } = $props();
	const noop = () => {};
</script>

<Tooltip.Provider>
	<CatalogRows
		{rows}
		{readonly}
		{countTitle}
		countLabel={(n) => `в ${n} заказах`}
		columns={withColumns
			? [
					{ id: 'color', title: 'Цвет', width: '2rem' },
					{ id: 'kind', title: 'Вид', help: ['Что значит вид.'], width: '10rem' }
				]
			: []}
		onrename={noop}
		onarchive={noop}
		onreorder={noop}
	>
		{#snippet extra(id)}
			<span style:grid-area="color">цвет {id}</span>
			<span style:grid-area="kind">вид {id}</span>
		{/snippet}
	</CatalogRows>
</Tooltip.Provider>
