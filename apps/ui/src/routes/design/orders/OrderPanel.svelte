<!-- The order panel of the demo (spec 6.2): fields saved as they change, status, money, photos, history. -->
<script lang="ts">
	import Camera from '@lucide/svelte/icons/camera';
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import Ellipsis from '@lucide/svelte/icons/ellipsis';
	import Eye from '@lucide/svelte/icons/eye';
	import EyeOff from '@lucide/svelte/icons/eye-off';
	import Phone from '@lucide/svelte/icons/phone';
	import Printer from '@lucide/svelte/icons/printer';
	import { MediaQuery } from 'svelte/reactivity';
	import type { Status } from '$api/Status';
	import { PHONE_QUERY } from '#lib/design/breakpoints.js';
	import { DEMO_NOW, makeDictionaries } from '#lib/demo/fake.js';
	import { deviceOf, estimateText, toPay, type DemoOrder } from '#lib/demo/orders.js';
	import { displayName, formatMoney, formatPhone, formatRelative } from '#lib/format/index.js';
	import { ru } from '#lib/i18n/ru.js';
	import {
		Badge,
		Button,
		ChipGroup,
		DropdownMenu,
		FileDrop,
		IconButton,
		MoneyInput,
		PhotoStrip,
		Section,
		Select,
		SlideOver,
		StatusPicker,
		Textarea,
		Timeline,
		type MenuItem
	} from '#lib/ui/index.js';

	let {
		order,
		statuses,
		pointName,
		saveState,
		returnFocusTo = null,
		onclose,
		onprev,
		onnext,
		onedit,
		onflush,
		onstatus,
		onissue,
		onreopen,
		onprint,
		onaddphotos,
		onremovephoto
	}: {
		order: DemoOrder | undefined;
		statuses: Status[];
		pointName: string;
		saveState: 'idle' | 'saving' | 'saved';
		returnFocusTo?: HTMLElement | null;
		onclose: () => void;
		onprev?: () => void;
		onnext?: () => void;
		/** A field changed: the page saves it (text 600 ms after the last change, choices at once). */
		onedit: (patch: Partial<DemoOrder>) => void;
		/** Focus left a field: save what is pending now. */
		onflush: () => void;
		onstatus: (statusId: number) => void;
		onissue: () => void;
		onreopen: () => void;
		onprint: (document: string) => void;
		onaddphotos: (files: File[]) => void;
		onremovephoto: (id: string) => void;
	} = $props();

	const CALL = ru.flags.needCall;
	const PARTS = ru.flags.needParts;

	const phone = new MediaQuery(PHONE_QUERY);
	const kind = $derived(statuses.find((s) => s.id === order?.status_id)?.kind);
	const issued = $derived(kind === 'issued');
	const DICTIONARIES = makeDictionaries();
	const masterItems = DICTIONARIES.master.map((m) => ({ value: m, label: displayName(m) }));
	const warrantyItems = DICTIONARIES.warranty.map((w) => ({ value: w, label: w }));
	const printItems: MenuItem[] = ['Акт приёма', 'Акт выдачи', 'Наклейка'].map((label) => ({
		label,
		onselect: () => onprint(label)
	}));
	const moreItems = $derived<MenuItem[]>([
		{ label: 'Вернуть в работу', disabled: !issued, onselect: onreopen }
	]);

	let drop = $state<ReturnType<typeof FileDrop>>();
	let revealedFor = $state<number | null>(null);
</script>

{#if order}
	{@const o = order}
	{@const pay = toPay(o)}
	{@const estimate = estimateText(o)}
	<SlideOver
		open={true}
		title="№ {o.id} · {deviceOf(o)}"
		{onclose}
		{onprev}
		{onnext}
		{returnFocusTo}
	>
		{#snippet actions()}
			<!-- Phone: «←», number, «⋯» at the top (spec 6.12); the bottom bar has call, photo, status. -->
			{#if phone.current}
				<DropdownMenu
					items={[...printItems, { separator: true }, ...moreItems]}
					label="Ещё действия"
				>
					{#snippet trigger(props)}
						<IconButton {...props} label="Ещё действия" icon={Ellipsis} />
					{/snippet}
				</DropdownMenu>
			{/if}
		{/snippet}
		{#snippet header()}
			<p class="meta num">
				<span>принят {formatRelative(o.received_at, DEMO_NOW)}</span>
				{#if pointName}<span aria-hidden="true">·</span><span>{pointName}</span>{/if}
				{#if saveState === 'saving'}
					<span aria-hidden="true">·</span><span>{ru.save.saving}</span>
				{:else if saveState === 'saved'}
					<span aria-hidden="true">·</span><span class="saved">✓ {ru.save.saved}</span>
				{/if}
			</p>
			{#if !phone.current}
				<StatusPicker
					{statuses}
					value={o.status_id}
					disabled={issued}
					onchange={(id) => onstatus(id)}
				/>
			{/if}
		{/snippet}

		<div class="fields" onfocusout={onflush}>
			<Section title="Клиент">
				<p class="line">
					<span class="strong">{displayName(o.client_name)}</span>
					{#if o.client_blacklisted}<Badge tone="danger">{ru.flags.blacklisted}</Badge>{/if}
				</p>
				<p class="line num">
					<a href="tel:+{o.client_phone}">{formatPhone(o.client_phone)}</a>
				</p>
			</Section>

			<Section title="Устройство">
				<p class="line">
					{[o.device_type, o.brand, o.model, o.color].filter(Boolean).join(' · ')}
				</p>
				{#if o.serial || o.device_password}
					<p class="line muted num">
						{#if o.serial}<span>IMEI/серийный {o.serial}</span>{/if}
						{#if o.device_password}
							<span>
								пароль {revealedFor === o.id ? o.device_password : '••••'}
								<button
									type="button"
									class="reveal"
									aria-pressed={revealedFor === o.id}
									aria-label={revealedFor === o.id
										? ru.fields.hidePassword
										: ru.fields.showPassword}
									onclick={() => (revealedFor = revealedFor === o.id ? null : o.id)}
								>
									{#if revealedFor === o.id}
										<EyeOff size={16} aria-hidden="true" />
									{:else}
										<Eye size={16} aria-hidden="true" />
									{/if}
								</button>
							</span>
						{/if}
					</p>
				{/if}
			</Section>

			<Section title="Неисправность и работа">
				<Textarea
					label="Неисправность"
					bind:value={() => o.malfunction, (v) => onedit({ malfunction: v })}
				/>
				<div class="pair">
					<Textarea
						label="Выполненные работы"
						bind:value={() => o.work_done, (v) => onedit({ work_done: v })}
					/>
					<Select
						label="Мастер"
						items={masterItems}
						bind:value={() => o.master || null, (v) => onedit({ master: v ?? '' })}
					/>
				</div>
			</Section>

			<Section title="Деньги">
				<p class="line muted num">
					{estimate ? `Оценка ${estimate}` : 'Без оценки'}
					{#if o.prepayment > 0}
						· Предоплата {formatMoney(o.prepayment)}{o.prepaid_at
							? ` (${formatRelative(o.prepaid_at, DEMO_NOW)})`
							: ''}
					{/if}
				</p>
				<div class="money">
					<MoneyInput
						label="Итог"
						bind:value={() => o.final_price, (v) => onedit({ final_price: v })}
					/>
					<MoneyInput
						label="Скидка"
						nullable={false}
						bind:value={() => o.discount, (v) => onedit({ discount: v ?? 0 })}
					/>
					<MoneyInput
						label="Расходы"
						nullable={false}
						bind:value={() => o.extra_costs, (v) => onedit({ extra_costs: v ?? 0 })}
					/>
				</div>
				<p class="to-pay num" class:negative={pay !== null && pay < 0}>
					К оплате <strong>{pay === null ? '—' : formatMoney(pay)}</strong>
				</p>
			</Section>

			<Section title="Гарантия">
				<Select
					label="Срок гарантии"
					items={warrantyItems}
					bind:value={() => o.warranty || null, (v) => onedit({ warranty: v ?? '' })}
				/>
			</Section>

			<ChipGroup
				label="Отметки"
				options={[CALL, PARTS]}
				bind:selected={
					() => [...(o.need_call ? [CALL] : []), ...(o.need_parts ? [PARTS] : [])],
					(v) => onedit({ need_call: v.includes(CALL), need_parts: v.includes(PARTS) })
				}
			/>

			<Textarea
				label="Комментарий (только для сервиса)"
				bind:value={() => o.internal_comment, (v) => onedit({ internal_comment: v })}
			/>

			<FileDrop bind:this={drop} onfiles={onaddphotos}>
				<PhotoStrip photos={o.photos} onadd={() => drop?.open()} onremove={onremovephoto} />
			</FileDrop>

			<Section title="История">
				<Timeline items={[...o.history].reverse()} now={DEMO_NOW} />
			</Section>
		</div>

		{#snippet footer()}
			{#if phone.current}
				<Button href="tel:+{o.client_phone}" icon={Phone}>Позвонить</Button>
				<Button icon={Camera} onclick={() => drop?.open()}>Фото</Button>
				<span class="grow"></span>
				<StatusPicker
					{statuses}
					value={o.status_id}
					disabled={issued}
					onchange={(id) => onstatus(id)}
				/>
			{:else}
				<DropdownMenu items={printItems} label="Печать">
					{#snippet trigger(props)}
						<Button {...props} icon={Printer} iconEnd={ChevronDown}>Печать</Button>
					{/snippet}
				</DropdownMenu>
				<DropdownMenu items={moreItems} label="Ещё действия">
					{#snippet trigger(props)}
						<IconButton {...props} label="Ещё действия" icon={Ellipsis} variant="secondary" />
					{/snippet}
				</DropdownMenu>
				<span class="grow"></span>
				{#if !issued}
					<Button
						variant={kind === 'ready' ? 'primary' : 'secondary'}
						shortcut="Mod+Enter"
						onclick={onissue}>Выдать</Button
					>
				{/if}
			{/if}
		{/snippet}
	</SlideOver>
{/if}

<style>
	.meta {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-1-5);
		margin: 0;
		color: var(--text-3);
		font-size: var(--text-sm);
	}
	.saved {
		color: var(--success-text);
	}
	.fields {
		display: flex;
		flex-direction: column;
		gap: var(--space-5);
	}
	.line {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
		margin: 0;
	}
	.strong {
		font-weight: 500;
	}
	.muted {
		color: var(--text-3);
		font-size: var(--text-sm);
	}
	a {
		color: var(--accent);
	}
	.reveal {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		vertical-align: middle;
		padding: 0;
		border: 0;
		background: transparent;
		color: var(--text-3);
		cursor: pointer;
	}
	.reveal:hover {
		color: var(--text);
	}
	.pair {
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(0, 11rem);
		align-items: start;
		gap: var(--space-3);
	}
	.money {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: var(--space-3);
	}
	.to-pay {
		margin: 0;
		font-size: var(--text-lg);
	}
	.to-pay strong {
		font-weight: 600;
	}
	.to-pay.negative {
		color: var(--danger-text);
	}
	.grow {
		flex: 1;
	}
	@media (pointer: coarse) {
		.reveal {
			min-width: 2.75rem;
			min-height: 2.75rem;
		}
	}
	@media (max-width: 599px) {
		.pair,
		.money {
			grid-template-columns: minmax(0, 1fr);
		}
	}
</style>
