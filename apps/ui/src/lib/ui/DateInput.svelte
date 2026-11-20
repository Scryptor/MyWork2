<script lang="ts">
	import { Calendar, Popover } from 'bits-ui';
	import { CalendarDate, parseDate } from '@internationalized/date';
	import CalendarIcon from '@lucide/svelte/icons/calendar';
	import ChevronLeft from '@lucide/svelte/icons/chevron-left';
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import { untrack } from 'svelte';
	import { layers } from '#lib/design/layers.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import Field from './Field.svelte';
	import './popup.css';

	let {
		value = $bindable(null),
		invalid = $bindable(false),
		min,
		max,
		label,
		hint,
		help,
		error,
		required = false,
		disabled = false,
		id
	}: {
		value: string | null;
		/** Out: the text is not a date within min and max (the value stays); forms do not submit. */
		invalid?: boolean;
		min?: string;
		max?: string;
		label?: string;
		hint?: string;
		/** «Что это?»: an icon right of the label with an explanation. */
		help?: string | readonly string[];
		error?: string;
		required?: boolean;
		disabled?: boolean;
		id?: string;
	} = $props();

	const uid = $props.id();
	const fid = $derived(id ?? uid);
	let open = $state(false);
	let text = $state('');
	let problem = $state<string | undefined>();
	let focused = false;
	/** The user typed in the field since it got focus: the text is theirs, not a copy of the value. */
	let dirty = false;
	/** The value the text stands for; any other value came from outside (another record, the calendar). */
	let synced: string | null | undefined;

	function toIso(d: CalendarDate): string {
		return d.toString();
	}
	function toDisplay(iso: string | null): string {
		if (!iso) return '';
		const [y, m, d] = iso.split('-');
		return `${d}.${m}.${y}`;
	}
	function tryIso(iso: string | undefined): CalendarDate | undefined {
		try {
			return iso ? parseDate(iso) : undefined;
		} catch {
			return undefined;
		}
	}
	/** dd.mm.yyyy (also with / or -) to a CalendarDate; undefined when not a real date. */
	function parseDisplay(s: string): CalendarDate | undefined {
		const m = /^(\d{1,2})[./-](\d{1,2})[./-](\d{4})$/.exec(s.trim());
		if (!m) return undefined;
		const iso = `${m[3]}-${m[2]!.padStart(2, '0')}-${m[1]!.padStart(2, '0')}`;
		return tryIso(iso);
	}

	function show(v: string | null) {
		synced = v;
		text = toDisplay(v);
		problem = undefined;
		if (invalid) invalid = false;
	}

	// A value from outside replaces the text, unless the field has focus: the user's own text stays until
	// they leave it, then the field shows the value (see `commit`).
	$effect.pre(() => {
		const v = value;
		untrack(() => {
			if (v !== synced && !(focused && dirty)) show(v);
		});
	});
	$effect(() => {
		if (open) return untrack(() => layers.open('popup', () => (open = false)));
	});

	const calValue = $derived(tryIso(value ?? undefined));
	const minValue = $derived(tryIso(min));
	const maxValue = $derived(tryIso(max));

	/** The date of a text: null when empty, a problem when it is not a date within min and max. */
	function check(t: string): { date: CalendarDate | null } | { problem: string } {
		const s = t.trim();
		if (s === '') return { date: null };
		const d = parseDisplay(s);
		if (!d) return { problem: ru.fields.dateError };
		if ((minValue && d.compare(minValue) < 0) || (maxValue && d.compare(maxValue) > 0)) {
			return { problem: ru.fields.dateRangeError };
		}
		return { date: d };
	}

	function markInvalid(bad: boolean) {
		if (bad !== invalid) invalid = bad;
	}

	/** The value changes as the user types: a text that is a date (or empty) is written at once. */
	function input(next: string) {
		dirty = true;
		const r = check(next);
		markInvalid('problem' in r);
		if ('problem' in r) return;
		problem = undefined;
		const iso = r.date === null ? null : toIso(r.date);
		synced = iso;
		if (iso !== value) value = iso;
	}

	/** On leaving the field or Enter: the error shows; a value that came from outside meanwhile wins. */
	function commit(own = true) {
		if (own && value !== synced) return show(value);
		const r = check(text);
		markInvalid('problem' in r);
		if ('problem' in r) {
			problem = r.problem;
			return;
		}
		problem = undefined;
		if (r.date) text = toDisplay(value);
	}
</script>

<Field {label} {hint} {help} error={error ?? problem} {required} id={fid}>
	{#snippet children(f)}
		<div class="mw-box" class:invalid={f.invalid} class:disabled>
			<input
				class="mw-input num"
				id={f.id}
				type="text"
				inputmode="numeric"
				autocomplete="off"
				placeholder={ru.fields.datePlaceholder}
				{disabled}
				bind:value={text}
				aria-describedby={f.describedBy}
				aria-invalid={f.invalid ? 'true' : undefined}
				aria-required={required ? 'true' : undefined}
				onfocus={() => {
					focused = true;
					dirty = false;
				}}
				oninput={(e) => input(e.currentTarget.value)}
				onblur={() => {
					focused = false;
					const own = dirty;
					dirty = false;
					commit(own);
				}}
				onkeydown={(e) => {
					if (e.key === 'Enter') commit();
				}}
			/>
			<Popover.Root bind:open>
				<Popover.Trigger {disabled} class="cal-btn" aria-label={ru.fields.openCalendar}>
					<CalendarIcon size={16} aria-hidden="true" />
				</Popover.Trigger>
				<Popover.Portal>
					<Popover.Content class="mw-popup cal" sideOffset={4} align="end">
						<Calendar.Root
							type="single"
							locale="ru"
							weekStartsOn={1}
							value={calValue}
							{minValue}
							{maxValue}
							onValueChange={(v) => {
								if (v) {
									const iso = toIso(new CalendarDate(v.year, v.month, v.day));
									show(iso);
									value = iso;
									open = false;
								}
							}}
						>
							{#snippet children({ months, weekdays })}
								<Calendar.Header class="cal-head">
									<Calendar.PrevButton class="cal-nav" aria-label={ru.fields.prevMonth}>
										<ChevronLeft size={16} aria-hidden="true" />
									</Calendar.PrevButton>
									<Calendar.Heading class="cal-title" />
									<Calendar.NextButton class="cal-nav" aria-label={ru.fields.nextMonth}>
										<ChevronRight size={16} aria-hidden="true" />
									</Calendar.NextButton>
								</Calendar.Header>
								{#each months as month (month.value.toString())}
									<Calendar.Grid class="cal-grid">
										<Calendar.GridHead>
											<Calendar.GridRow class="cal-row">
												{#each weekdays as day, d (d)}
													<Calendar.HeadCell class="cal-headcell">{day}</Calendar.HeadCell>
												{/each}
											</Calendar.GridRow>
										</Calendar.GridHead>
										<Calendar.GridBody>
											{#each month.weeks as week, w (w)}
												<Calendar.GridRow class="cal-row">
													{#each week as date (date.toString())}
														<Calendar.Cell {date} month={month.value} class="cal-cell">
															<Calendar.Day class="cal-day" />
														</Calendar.Cell>
													{/each}
												</Calendar.GridRow>
											{/each}
										</Calendar.GridBody>
									</Calendar.Grid>
								{/each}
							{/snippet}
						</Calendar.Root>
					</Popover.Content>
				</Popover.Portal>
			</Popover.Root>
		</div>
	{/snippet}
</Field>

<style>
	:global(.cal-btn) {
		display: inline-flex;
		align-items: center;
		padding: 0;
		border: 0;
		background: transparent;
		color: var(--text-3);
		cursor: pointer;
	}
	:global(.cal-btn:hover) {
		color: var(--text);
	}
	:global(.mw-popup.cal) {
		padding: var(--space-3);
		min-width: 0;
	}
	:global(.cal-head) {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: var(--space-2);
	}
	:global(.cal-title) {
		font-weight: 500;
		text-transform: capitalize;
	}
	:global(.cal-nav) {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: var(--control-sm);
		height: var(--control-sm);
		border: 0;
		border-radius: var(--radius-sm);
		background: transparent;
		color: var(--text);
		cursor: pointer;
	}
	:global(.cal-nav:hover) {
		background: var(--surface-3);
	}
	:global(.cal-grid) {
		border-collapse: collapse;
	}
	:global(.cal-row) {
		display: flex;
	}
	:global(.cal-headcell) {
		width: var(--control-sm);
		text-align: center;
		font-size: var(--text-xs);
		color: var(--text-3);
		font-weight: 500;
	}
	:global(.cal-cell) {
		padding: 1px;
	}
	:global(.cal-day) {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: var(--control-sm);
		height: var(--control-sm);
		border-radius: var(--radius-sm);
		cursor: pointer;
		font-variant-numeric: tabular-nums;
	}
	:global(.cal-day:hover) {
		background: var(--surface-3);
	}
	:global(.cal-day[data-outside-month]) {
		color: var(--text-disabled);
	}
	:global(.cal-day[data-today]) {
		border: 1px solid var(--accent);
	}
	:global(.cal-day[data-selected]) {
		background: var(--accent);
		color: var(--on-accent);
	}
	:global(.cal-day[data-disabled]),
	:global(.cal-day[data-unavailable]) {
		color: var(--text-disabled);
		cursor: not-allowed;
		text-decoration: line-through;
	}
	@media (pointer: coarse) {
		:global(.cal-btn) {
			justify-content: center;
			min-width: var(--control-md);
			min-height: var(--control-md);
			margin: 0 calc(-1 * var(--space-3));
		}
		:global(.cal-nav),
		:global(.cal-day) {
			width: var(--control-md);
			height: var(--control-md);
		}
		:global(.cal-headcell) {
			width: var(--control-md);
		}
	}
</style>
