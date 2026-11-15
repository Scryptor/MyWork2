<script lang="ts">
	import { contrastRatio } from '#lib/design/contrast.js';
	import { STATUS_HUES } from '#lib/design/status-colors.js';
	import { prefs } from '#lib/design/prefs.svelte.js';

	const COLOR_TOKENS = [
		'bg',
		'surface',
		'surface-2',
		'surface-3',
		'border',
		'border-strong',
		'text',
		'text-2',
		'text-3',
		'text-disabled',
		'accent',
		'accent-hover',
		'accent-active',
		'on-accent',
		'accent-soft',
		'accent-text',
		'focus',
		'danger',
		'danger-hover',
		'on-danger',
		'danger-soft',
		'danger-text',
		'warning-soft',
		'warning-text',
		'success-soft',
		'success-text',
		'info-soft',
		'info-text'
	];
	const TEXT_SIZES = ['xs', 'sm', 'md', 'lg', 'xl', '2xl'];

	let swatches = $state<{ name: string; value: string; ratio: string }[]>([]);

	function computed(name: string): string {
		return getComputedStyle(document.documentElement).getPropertyValue(`--${name}`).trim();
	}

	$effect(() => {
		void prefs.resolvedTheme;
		const surface = computed('surface');
		swatches = COLOR_TOKENS.map((name) => {
			const value = computed(name);
			let ratio: string;
			try {
				ratio = contrastRatio(value, surface).toFixed(2);
			} catch {
				ratio = '';
			}
			return { name, value, ratio };
		});
	});
</script>

<h3>Цвета ({prefs.resolvedTheme === 'dark' ? 'тёмная' : 'светлая'} тема)</h3>
<ul class="swatches">
	{#each swatches as s (s.name)}
		<li>
			<span class="chip" style:background={`var(--${s.name})`}></span>
			<code>--{s.name}</code>
			<span class="num muted">{s.value}</span>
			<span class="num muted">{s.ratio ? `${s.ratio}:1` : ''}</span>
		</li>
	{/each}
</ul>

<h3>Шкала текста</h3>
{#each TEXT_SIZES as size (size)}
	<p style:font-size={`var(--text-${size})`} style:margin="0">
		<code>--text-{size}</code> Принят в ремонт: «Айфон», экран разбит
	</p>
{/each}
<p class="label-caps">Подпись группы</p>

<h3>Оттенки статусов</h3>
<div class="hues">
	{#each STATUS_HUES as hue (hue)}
		<span class={`pill st-${hue}`}>
			<span class="dot" style:background="var(--st-dot)"></span>
			{hue}
		</span>
	{/each}
</div>

<style>
	.swatches {
		list-style: none;
		padding: 0;
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(16rem, 1fr));
		gap: var(--space-2);
	}
	li {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		font-size: var(--text-sm);
	}
	.chip {
		width: 1.5rem;
		height: 1.5rem;
		border-radius: var(--radius-sm);
		border: 1px solid var(--border-strong);
		flex: none;
	}
	.muted {
		color: var(--text-3);
	}
	.hues {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
	}
	.pill {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1-5);
		padding: var(--space-0-5) var(--space-2);
		border-radius: var(--radius-full);
		font-size: var(--text-xs);
		font-weight: 600;
	}
	.dot {
		width: 0.5rem;
		height: 0.5rem;
		border-radius: var(--radius-full);
	}
</style>
