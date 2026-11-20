<script lang="ts">
	import { Tooltip } from 'bits-ui';
	import type { ImportReportView } from '$api/ImportReportView';
	import ReportView from './ReportView.svelte';

	let {
		report,
		onorder
	}: { report: ImportReportView; onorder: (id: number, ids: readonly number[]) => void } = $props();
</script>

<Tooltip.Provider>
	<ReportView {report} {onorder} />
</Tooltip.Provider>
