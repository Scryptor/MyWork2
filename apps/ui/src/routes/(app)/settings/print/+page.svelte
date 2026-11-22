<!-- «Печать» (spec 5.2, plan 6): kept on this computer only; every field applies at once. The program
     prints by itself and has the printers and the copies; a browser prints through a PDF, so there only
     what shapes the document is here: the label, its offset, the printer's resolution. -->
<script lang="ts">
	import { onMount } from 'svelte';
	import { ru } from '#lib/i18n/ru.js';
	import { canPrintDirectly, listPrinters, type PrinterInfo } from '#lib/print/platform.js';
	import {
		COPIES,
		formatMm,
		LABEL_MM,
		LABEL_PRESETS,
		OFFSET_MM,
		parseMm,
		PRINTER_DPIS,
		printSettings
	} from '#lib/print/settings.svelte.js';
	import Callout from '#lib/ui/Callout.svelte';
	import Card from '#lib/ui/Card.svelte';
	import HelpHint from '#lib/ui/HelpHint.svelte';
	import SegmentedControl from '#lib/ui/SegmentedControl.svelte';
	import Select from '#lib/ui/Select.svelte';
	import TextInput from '#lib/ui/TextInput.svelte';

	const t = ru.print;
	const help = ru.help.print;
	const direct = canPrintDirectly();

	/** The printers of this computer; null while they are asked for (or when they could not be). */
	let printers = $state.raw<PrinterInfo[] | null>(null);
	let printersError = $state<string | null>(null);
	onMount(() => {
		if (!direct) return;
		listPrinters().then(
			(list) => (printers = list),
			(e: unknown) => (printersError = t.printersFailed(e instanceof Error ? e.message : String(e)))
		);
	});

	/** «Принтер по умолчанию» is the empty value: the setting is null. */
	const SYSTEM = '';
	/** «Принтер по умолчанию», a chosen printer that is gone (marked once the list is read), then the list. */
	function printerItems(chosen: string | null) {
		const list = printers ?? [];
		const gone =
			chosen !== null && !list.some((p) => p.name === chosen)
				? [{ value: chosen, label: printers ? t.missingPrinter(chosen) : chosen }]
				: [];
		return [
			{ value: SYSTEM, label: t.defaultPrinter },
			...gone,
			...list.map((p) => ({
				value: p.name,
				label: p.is_default ? t.systemDefault(p.name) : p.name
			}))
		];
	}
	const printerOf = (v: string | null) => (v === null || v === SYSTEM ? null : v);

	const CUSTOM = 'custom';
	const key = (mm: readonly [number, number]) => `${mm[0]}x${mm[1]}`;
	const isPreset = (mm: readonly [number, number]) =>
		LABEL_PRESETS.some((p) => p[0] === mm[0] && p[1] === mm[1]);
	const SIZES = [
		...LABEL_PRESETS.map((p) => ({ value: key(p), label: t.presetMm(p[0], p[1]) })),
		{ value: CUSTOM, label: t.customSize }
	];
	/** «Свой» stays chosen while its fields are typed, even through the size of a preset. */
	let custom = $state(!isPreset(printSettings.labelMm));
	const size = $derived(custom ? CUSTOM : key(printSettings.labelMm));
	function chooseSize(v: string) {
		if (v === CUSTOM) {
			custom = true;
			return;
		}
		const preset = LABEL_PRESETS.find((p) => key(p) === v);
		if (!preset) return;
		custom = false;
		width.reset();
		height.reset();
		printSettings.set({ labelMm: [preset[0], preset[1]] });
	}

	const mmCheck = (r: { min: number; max: number }) => (n: number) =>
		n >= r.min && n <= r.max ? undefined : t.mmRange(r.min, r.max);
	const copiesCheck = (n: number) =>
		!Number.isInteger(n)
			? t.notWhole
			: n >= COPIES.min && n <= COPIES.max
				? undefined
				: t.copiesRange(COPIES.min, COPIES.max);

	/**
	 * A number typed into a field: kept at once when it fits. What does not fit stays in the field with
	 * its mistake (shown once the field is left) and is not kept: the setting is as it was before the
	 * field was typed into, not the «30» on the way to «300».
	 */
	function numberField(opts: {
		read: () => number;
		save: (n: number) => void;
		check: (n: number) => string | undefined;
		notNumber: string;
	}) {
		let draft = $state<string | null>(null);
		let error = $state<string | undefined>();
		/** The setting when the typing began. */
		let before: number | null = null;
		const mistake = (text: string): string | undefined => {
			const n = parseMm(text);
			return n === null ? opts.notNumber : opts.check(n);
		};
		return {
			get text() {
				return draft ?? formatMm(opts.read());
			},
			get error() {
				return error;
			},
			input(text: string) {
				before ??= opts.read();
				draft = text;
				const wrong = mistake(text);
				if (wrong === undefined) {
					opts.save(parseMm(text)!);
					error = undefined;
					return;
				}
				if (opts.read() !== before) opts.save(before);
				// A mistake already shown follows what is typed; a new one waits for the field to be left.
				if (error !== undefined) error = wrong;
			},
			leave() {
				if (draft === null) return;
				error = mistake(draft);
				if (error !== undefined) return;
				draft = null;
				before = null;
			},
			reset() {
				draft = null;
				error = undefined;
				before = null;
			}
		};
	}

	const width = numberField({
		read: () => printSettings.labelMm[0],
		save: (n) => printSettings.set({ labelMm: [n, printSettings.labelMm[1]] }),
		check: mmCheck(LABEL_MM),
		notNumber: t.notNumber
	});
	const height = numberField({
		read: () => printSettings.labelMm[1],
		save: (n) => printSettings.set({ labelMm: [printSettings.labelMm[0], n] }),
		check: mmCheck(LABEL_MM),
		notNumber: t.notNumber
	});
	const offsetX = numberField({
		read: () => printSettings.offsetMm[0],
		save: (n) => printSettings.set({ offsetMm: [n, printSettings.offsetMm[1]] }),
		check: mmCheck(OFFSET_MM),
		notNumber: t.notNumber
	});
	const offsetY = numberField({
		read: () => printSettings.offsetMm[1],
		save: (n) => printSettings.set({ offsetMm: [printSettings.offsetMm[0], n] }),
		check: mmCheck(OFFSET_MM),
		notNumber: t.notNumber
	});
	const actCopies = numberField({
		read: () => printSettings.actCopies,
		save: (n) => printSettings.set({ actCopies: n }),
		check: copiesCheck,
		notNumber: t.notWhole
	});
	const labelCopies = numberField({
		read: () => printSettings.labelCopies,
		save: (n) => printSettings.set({ labelCopies: n }),
		check: copiesCheck,
		notNumber: t.notWhole
	});

	const DPIS = PRINTER_DPIS.map((value) => ({ value, label: String(value) }));
</script>

<h1 class="title">{ru.settings.print}</h1>

{#if !direct}
	<Callout tone="info" title={t.browserTitle}>{t.browserText}</Callout>
{/if}

<Card>
	<div class="fields">
		<p class="note">{ru.settings.thisComputer}</p>
		{#if direct}
			{#if printersError}<Callout tone="warning">{printersError}</Callout>{/if}
			<div class="row">
				<div class="cell wide">
					<Select
						label={t.actPrinter}
						help={help.actPrinter}
						items={printerItems(printSettings.actPrinter)}
						bind:value={
							() => printSettings.actPrinter ?? SYSTEM,
							(v) => printSettings.set({ actPrinter: printerOf(v) })
						}
					/>
				</div>
				<div class="cell wide">
					<Select
						label={t.labelPrinter}
						help={help.labelPrinter}
						items={printerItems(printSettings.labelPrinter)}
						bind:value={
							() => printSettings.labelPrinter ?? SYSTEM,
							(v) => printSettings.set({ labelPrinter: printerOf(v) })
						}
					/>
				</div>
			</div>
		{/if}
		<div class="field">
			<span class="head">
				<span class="label">{t.labelSize}</span>
				<HelpHint title={t.labelSize} text={help.labelSize} />
			</span>
			<SegmentedControl label={t.labelSize} options={SIZES} bind:value={() => size, chooseSize} />
		</div>
		{#if custom}
			<div class="row">
				<div class="cell">
					<TextInput
						label={t.width}
						inputmode="decimal"
						error={width.error}
						bind:value={() => width.text, (v) => width.input(v)}
						onblur={() => width.leave()}
					/>
				</div>
				<div class="cell">
					<TextInput
						label={t.height}
						inputmode="decimal"
						error={height.error}
						bind:value={() => height.text, (v) => height.input(v)}
						onblur={() => height.leave()}
					/>
				</div>
			</div>
		{/if}
		<div class="row">
			<div class="cell">
				<TextInput
					label={t.offsetX}
					help={help.offset}
					inputmode="decimal"
					error={offsetX.error}
					bind:value={() => offsetX.text, (v) => offsetX.input(v)}
					onblur={() => offsetX.leave()}
				/>
			</div>
			<div class="cell">
				<TextInput
					label={t.offsetY}
					inputmode="decimal"
					error={offsetY.error}
					bind:value={() => offsetY.text, (v) => offsetY.input(v)}
					onblur={() => offsetY.leave()}
				/>
			</div>
		</div>
		<div class="field">
			<span class="head">
				<span class="label">{t.dpi}</span>
				<HelpHint title={t.dpi} text={help.dpi} />
			</span>
			<SegmentedControl
				label={t.dpi}
				options={DPIS}
				bind:value={() => printSettings.dpi, (v) => printSettings.set({ dpi: v })}
			/>
		</div>
		{#if direct}
			<div class="row">
				<div class="cell">
					<TextInput
						label={t.actCopies}
						help={help.copies}
						inputmode="numeric"
						error={actCopies.error}
						bind:value={() => actCopies.text, (v) => actCopies.input(v)}
						onblur={() => actCopies.leave()}
					/>
				</div>
				<div class="cell">
					<TextInput
						label={t.labelCopies}
						inputmode="numeric"
						error={labelCopies.error}
						bind:value={() => labelCopies.text, (v) => labelCopies.input(v)}
						onblur={() => labelCopies.leave()}
					/>
				</div>
			</div>
		{/if}
	</div>
</Card>

<style>
	.title {
		margin: 0;
		font-size: var(--text-xl);
		font-weight: 600;
	}
	.fields {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
	}
	.field {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: var(--space-1);
		max-width: 100%;
	}
	.head {
		display: flex;
		align-items: center;
		gap: var(--space-1);
	}
	.label {
		font-size: var(--text-sm);
		font-weight: 600;
	}
	.note {
		margin: 0;
		color: var(--text-3);
		font-size: var(--text-sm);
	}
	/* Two fields side by side; in a narrow window one under the other. */
	.row {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-3);
	}
	.cell {
		flex: 0 1 12rem;
		min-width: 0;
	}
	.cell.wide {
		flex: 1 1 16rem;
	}
</style>
