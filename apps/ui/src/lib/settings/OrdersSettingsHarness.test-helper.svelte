<!-- «Настройки → Заказы» for unit tests: the tooltips of its buttons need their provider, as in the app. -->
<script lang="ts">
	import { Tooltip } from 'bits-ui';
	import Page from '../../routes/(app)/settings/orders/+page.svelte';
</script>

<Tooltip.Provider>
	<Page />
</Tooltip.Provider>
