<!-- «Настройки → Импорт из MyWork 2» as the program has it: under the provider of tooltips (the icons of
     «Что это?» need it). -->
<script lang="ts">
	import { Tooltip } from 'bits-ui';
	import Page from '../../routes/(app)/settings/import/+page.svelte';
</script>

<Tooltip.Provider>
	<Page />
</Tooltip.Provider>
