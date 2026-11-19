<!-- The stages of a transfer from MyWork 2 (spec 6.9): done, under way with its percent, waiting; a bar for
     the stage under way. `written`: the base is known to be written (everything to «Запись» is done). -->
<script lang="ts">
	import Check from '@lucide/svelte/icons/check';
	import Circle from '@lucide/svelte/icons/circle';
	import CircleX from '@lucide/svelte/icons/circle-x';
	import LoaderCircle from '@lucide/svelte/icons/loader-circle';
	import type { ImportJob } from '$api/ImportJob';
	import { ru } from '#lib/i18n/ru.js';
	import ProgressBar from '#lib/ui/ProgressBar.svelte';
	import VisuallyHidden from '#lib/ui/VisuallyHidden.svelte';
	import { stageSteps } from './progress.js';

	let {
		job,
		written = false,
		failed = false
	}: { job: Pick<ImportJob, 'stage' | 'percent'>; written?: boolean; failed?: boolean } = $props();

	const t = ru.legacy;
	const steps = $derived(stageSteps(job, { written, failed }));
	const current = $derived(steps.find((s) => s.state === 'current'));
</script>

<div class="progress">
	<ol class="steps" aria-label={t.progress}>
		{#each steps as step (step.stage)}
			<li class="step {step.state}">
				<span class="mark" aria-hidden="true">
					{#if step.state === 'done'}<Check size={16} />
					{:else if step.state === 'current'}<LoaderCircle size={16} class="spin" />
					{:else if step.state === 'failed'}<CircleX size={16} />
					{:else}<Circle size={16} />{/if}
				</span>
				<span class="label">{step.label}</span>
				{#if step.state === 'current'}<span class="percent num">{step.percent} %</span>
				{:else if step.state === 'done'}<VisuallyHidden>{t.stageDone}</VisuallyHidden>
				{:else if step.state === 'failed'}<span class="percent">{t.stageFailed}</span>{/if}
			</li>
		{/each}
	</ol>
	{#if current}<ProgressBar value={current.percent} label={current.label} />{/if}
</div>

<style>
	.progress {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}
	.steps {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		margin: 0;
		padding: 0;
		list-style: none;
	}
	.step {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		color: var(--text-3);
		font-size: var(--text-md);
	}
	.mark {
		display: inline-flex;
		flex: none;
	}
	.done {
		color: var(--text-2);
	}
	.done .mark {
		color: var(--success-text);
	}
	.current {
		color: var(--text);
		font-weight: 600;
	}
	.current .mark {
		color: var(--accent-text);
	}
	.failed,
	.failed .mark {
		color: var(--danger-text);
	}
	.percent {
		margin-inline-start: auto;
		font-weight: 400;
	}
	.mark :global(.spin) {
		animation: spin 900ms linear infinite;
	}
	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.mark :global(.spin) {
			animation: none;
		}
	}
</style>
