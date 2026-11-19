<script lang="ts">
	import SortableList from './SortableList.svelte';

	type Item = { id: number; name: string };
	let { disabled = false, onorder }: { disabled?: boolean; onorder?: (ids: number[]) => void } =
		$props();

	let items = $state<Item[]>([
		{ id: 1, name: 'Первый' },
		{ id: 2, name: 'Второй' },
		{ id: 3, name: 'Третий' }
	]);

	function reorder(ids: number[]) {
		onorder?.(ids);
		items = ids.map((id) => items.find((i) => i.id === id)!);
	}
</script>

<SortableList {items} key={(i) => i.id} label={(i) => i.name} onreorder={reorder} {disabled}>
	{#snippet row(i)}<span>{i.name}</span>{/snippet}
</SortableList>
