<!-- The journal of the panel with a button that makes a part action: the version stays, `refresh` grows. -->
<script lang="ts">
	import OrderHistory from './OrderHistory.svelte';

	let refresh = $state(0);
</script>

<OrderHistory orderId={7} version={4} {refresh} onall={() => {}} />
<button type="button" onclick={() => (refresh += 1)}>part action</button>
