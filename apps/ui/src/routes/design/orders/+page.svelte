<!-- Demo of the main screen (spec 6.1, 6.2, 6.4, 6.12) over 2 000 fictional orders, state kept locally. -->
<script lang="ts">
	import Boxes from '@lucide/svelte/icons/boxes';
	import CalendarClock from '@lucide/svelte/icons/calendar-clock';
	import ChartColumn from '@lucide/svelte/icons/chart-column';
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import CircleCheck from '@lucide/svelte/icons/circle-check';
	import Clock from '@lucide/svelte/icons/clock';
	import Ellipsis from '@lucide/svelte/icons/ellipsis';
	import Moon from '@lucide/svelte/icons/moon';
	import Package from '@lucide/svelte/icons/package';
	import PhoneCall from '@lucide/svelte/icons/phone-call';
	import Plus from '@lucide/svelte/icons/plus';
	import Printer from '@lucide/svelte/icons/printer';
	import ScanLine from '@lucide/svelte/icons/scan-line';
	import Search from '@lucide/svelte/icons/search';
	import Settings from '@lucide/svelte/icons/settings';
	import Sun from '@lucide/svelte/icons/sun';
	import Users from '@lucide/svelte/icons/users';
	import Wrench from '@lucide/svelte/icons/wrench';
	import { tick, untrack, type Component } from 'svelte';
	import { MediaQuery } from 'svelte/reactivity';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { PHONE_QUERY } from '#lib/design/breakpoints.js';
	import { registerHotkey } from '#lib/design/hotkeys.svelte.js';
	import { layers } from '#lib/design/layers.svelte.js';
	import { prefs } from '#lib/design/prefs.svelte.js';
	import { installScanner } from '#lib/design/scanner.js';
	import { DEMO_NOW, DEMO_POINTS, DEMO_STATUSES, makeOrders } from '#lib/demo/fake.js';
	import {
		attentionCounts,
		matchesOrder,
		matchesQuickFilter,
		needsAttention,
		queryWords,
		quickFilterItems,
		resolveScan,
		searchClients,
		searchOrders,
		sortOrders,
		withDetails,
		withStatus,
		type DemoOrder,
		type DemoPhoto
	} from '#lib/demo/orders.js';
	import {
		countLabel,
		displayName,
		formatFull,
		formatMoney,
		formatPhone,
		formatRelative,
		shortName
	} from '#lib/format/index.js';
	import { ru } from '#lib/i18n/ru.js';
	import { ATTENTION_KEYS, deviceOf, estimateText, type AttentionKey } from '#lib/orders/view.js';
	import '#lib/ui/field.css';
	import {
		AppShell,
		BottomNav,
		Button,
		CommandPalette,
		DataTable,
		DropdownMenu,
		EmptyState,
		FilterChips,
		FlagIcons,
		IconButton,
		InfiniteList,
		Kbd,
		ListCard,
		SelectionBar,
		ShortcutsHelp,
		SidebarNav,
		StatusBadge,
		TopBar,
		VisuallyHidden,
		filterCommands,
		toast,
		type Column,
		type CommandGroup,
		type CommandItem,
		type MenuItem,
		type NavItem,
		type NavSection,
		type RowId,
		type SortState
	} from '#lib/ui/index.js';
	import IssueDialog, { type IssuePatch } from './IssueDialog.svelte';
	import OrderPanel from './OrderPanel.svelte';

	/** Autosave is simulated: «Сохраняю…» until 600 ms after the last change (spec 6.2). */
	const SAVE_MS = 600;
	const LIST_PAGE = 30;
	const MAX_PHOTOS = 10;

	const p2 = (n: number) => String(n).padStart(2, '0');
	/** «Now» of the demo as the API writes it: changes and issues are stamped with it. */
	const NOW = `${DEMO_NOW.year}-${p2(DEMO_NOW.month)}-${p2(DEMO_NOW.day)}T${p2(DEMO_NOW.hour)}:${p2(DEMO_NOW.minute)}:${p2(DEMO_NOW.second)}`;

	const statusById = new Map(DEMO_STATUSES.map((s) => [s.id, s]));
	const kindOf = (id: number) => statusById.get(id)?.kind;
	const pointById = new Map(DEMO_POINTS.map((p) => [p.id, p.name]));
	const pointName = (id: number | null) => (id === null ? '' : (pointById.get(id) ?? ''));
	const sortContext = {
		statusOrder: (id: number) => statusById.get(id)?.sort_order ?? 0,
		pointName
	};
	const ISSUED = DEMO_STATUSES.find((s) => s.kind === 'issued') ?? DEMO_STATUSES[0]!;
	const BACK_TO_WORK = DEMO_STATUSES.find((s) => s.name === 'Принят в работу') ?? DEMO_STATUSES[0]!;
	const ORDERS_FORMS = ['заказ', 'заказа', 'заказов'] as const;

	const phone = new MediaQuery(PHONE_QUERY);

	let orders = $state.raw<DemoOrder[]>(
		makeOrders(2000).map((o) => withDetails(o, kindOf(o.status_id) ?? 'open'))
	);
	const byId = $derived(new Map(orders.map((o) => [o.id, o])));

	function update(id: number, change: (o: DemoOrder) => DemoOrder) {
		orders = orders.map((o) => (o.id === id ? change(o) : o));
	}

	// The view: quick filters or an item of «Требуют внимания», the point, the filter text, the sort.
	let chips = $state.raw<string[]>(['work']);
	let attention = $state<AttentionKey | null>(null);
	let point = $state<number | null>(null);
	let query = $state('');
	let sort = $state.raw<SortState>({ columnId: 'received', descending: true });
	let selected = $state.raw(new Set<RowId>());
	let loaded = $state(LIST_PAGE);

	/** Another view is another data set: the selection and the phone list start over. */
	function setView(change: () => void) {
		change();
		selected = new Set();
		loaded = LIST_PAGE;
	}

	// The list is made when the view changes, not when an order is edited: like a list from the
	// server, a row whose new status left the filter stays until the view changes («Отменить» finds it).
	const listIds = $derived.by(() => {
		const view = { chips, attention, point, words: queryWords(query), sort };
		return untrack(() => {
			const list = orders.filter((o) => {
				const kind = kindOf(o.status_id);
				return (
					(view.attention
						? needsAttention(view.attention, o, kind, DEMO_NOW)
						: matchesQuickFilter(view.chips, o.status_id, kind)) &&
					(view.point === null || o.service_point_id === view.point) &&
					matchesOrder(o, view.words)
				);
			});
			return sortOrders(list, view.sort, sortContext).map((o) => o.id);
		});
	});
	const rows = $derived(
		listIds.flatMap((id) => {
			const o = byId.get(id);
			return o ? [o] : [];
		})
	);
	const cards = $derived(rows.slice(0, loaded));

	const counts = $derived(attentionCounts(orders, kindOf, DEMO_NOW));
	const chipItems = $derived(quickFilterItems(orders, DEMO_STATUSES));
	const inWork = $derived(chipItems.find((c) => c.id === 'work')?.count ?? 0);

	// The open order: its panel, the table cursor on its row, «Недавние» of Ctrl K.
	let activeId = $state<RowId | null>(null);
	let openedId = $state<number | null>(null);
	let recent = $state.raw<number[]>([]);
	const opened = $derived(openedId === null ? undefined : byId.get(openedId));
	const openedAt = $derived(openedId === null ? -1 : listIds.indexOf(openedId));
	const prevId = $derived(openedAt > 0 ? listIds[openedAt - 1] : undefined);
	const nextId = $derived(openedAt >= 0 ? listIds[openedAt + 1] : undefined);
	const canIssue = $derived(opened !== undefined && kindOf(opened.status_id) !== 'issued');

	let tableWrap = $state<HTMLElement | null>(null);
	const grid = $derived(tableWrap?.querySelector<HTMLElement>('[role="grid"]') ?? null);

	function openOrder(id: number) {
		if (id !== openedId) resetSave();
		openedId = id;
		activeId = id;
		recent = [id, ...recent.filter((x) => x !== id)].slice(0, 5);
	}
	function closePanel() {
		openedId = null;
		resetSave();
	}

	let saveState = $state<'idle' | 'saving' | 'saved'>('idle');
	let saveTimer: ReturnType<typeof setTimeout> | undefined;
	function saving() {
		saveState = 'saving';
		clearTimeout(saveTimer);
		saveTimer = setTimeout(() => (saveState = 'saved'), SAVE_MS);
	}
	/** Focus left a field: what was typed is saved now. */
	function flush() {
		if (saveState !== 'saving') return;
		clearTimeout(saveTimer);
		saveState = 'saved';
	}
	function resetSave() {
		clearTimeout(saveTimer);
		saveState = 'idle';
	}

	function edit(patch: Partial<DemoOrder>) {
		if (openedId === null) return;
		update(openedId, (o) => ({ ...o, ...patch }));
		saving();
	}

	/** A new status (and what comes with it); returns the undo, which gives all of it back. */
	function setStatus(
		id: number,
		statusId: number,
		patch: Partial<DemoOrder> = {}
	): (() => void) | undefined {
		const before = byId.get(id);
		const status = statusById.get(statusId);
		if (!before || !status || before.status_id === statusId) return undefined;
		const change = withStatus(before, status, NOW, patch);
		update(id, () => change.order);
		if (id === openedId) saving();
		return () => update(id, change.undo);
	}

	/**
	 * A status picked in the panel. An issued one is not set here: it opens «Выдать» with its checks
	 * (spec 6.4) and the order keeps its status unless the dialog issues it.
	 */
	function changeStatus(statusId: number) {
		if (openedId === null) return;
		if (kindOf(statusId) === 'issued') {
			openIssue(statusId);
			return;
		}
		const id = openedId;
		const undo = setStatus(id, statusId);
		if (undo) toast.undoable(`№ ${id} → ${statusById.get(statusId)?.name ?? ''}`, undo);
	}

	let issueOpen = $state(false);
	/** The issued status the dialog sets: «Выдан», or the issued one picked in the panel. */
	let issueTo = $state(ISSUED.id);
	function openIssue(statusId: number = ISSUED.id) {
		if (!canIssue) return;
		issueTo = statusId;
		issueOpen = true;
	}
	async function issue(patch: IssuePatch, printAct: boolean) {
		if (openedId === null) return;
		const id = openedId;
		const status = statusById.get(issueTo) ?? ISSUED;
		const undo = setStatus(id, status.id, patch);
		// Toasts cannot be used under a modal: they appear once the dialog has gone.
		await tick();
		if (undo) toast.undoable(`№ ${id} → ${status.name}`, undo);
		if (printAct) toast.show({ message: 'Акт выдачи отправлен на принтер «Приёмка»' });
	}
	function reopen() {
		if (openedId === null) return;
		const id = openedId;
		const undo = setStatus(id, BACK_TO_WORK.id);
		if (undo) toast.undoable(`№ ${id} → ${BACK_TO_WORK.name}`, undo);
	}
	function printDocument(name: string) {
		toast.show({ message: `${name}: отправлено на принтер «Приёмка»` });
	}

	// Photos are local pictures shown by object URLs; they are let go with the photo or the page.
	let photoSeq = 0;
	let photoUrls: string[] = [];
	function addPhotos(files: File[]) {
		const order = opened;
		if (!order) return;
		const added: DemoPhoto[] = files.slice(0, MAX_PHOTOS - order.photos.length).map((f) => {
			const src = URL.createObjectURL(f);
			photoUrls.push(src);
			return { id: `photo-${++photoSeq}`, src, alt: f.name };
		});
		if (added.length === 0) return;
		update(order.id, (o) => ({ ...o, photos: [...o.photos, ...added] }));
		saving();
	}
	function removePhoto(photoId: string) {
		const order = opened;
		const photo = order?.photos.find((p) => p.id === photoId);
		if (!order || !photo) return;
		URL.revokeObjectURL(photo.src);
		photoUrls = photoUrls.filter((u) => u !== photo.src);
		update(order.id, (o) => ({ ...o, photos: o.photos.filter((p) => p.id !== photoId) }));
		saving();
	}
	$effect(() => () => {
		clearTimeout(saveTimer);
		for (const url of photoUrls) URL.revokeObjectURL(url);
	});

	// Bulk actions on the selected rows. Issued statuses are left out: issuing many orders at once has to
	// ask for the issue date (plan 5); one order is issued through «Выдать».
	const bulkItems: MenuItem[] = DEMO_STATUSES.filter((s) => !s.archived && s.kind !== 'issued').map(
		(s) => ({
			label: s.name,
			onselect: () => bulkStatus(s.id)
		})
	);
	function bulkStatus(statusId: number) {
		const undos = [...selected].flatMap((id) => {
			const undo = typeof id === 'number' ? setStatus(id, statusId) : undefined;
			return undo ? [undo] : [];
		});
		if (undos.length === 0) return;
		toast.undoable(
			`${statusById.get(statusId)?.name ?? ''}: ${countLabel(undos.length, ORDERS_FORMS)}`,
			() => undos.forEach((undo) => undo())
		);
	}
	function printSelected() {
		toast.show({ message: `Отправлено на печать: ${countLabel(selected.size, ORDERS_FORMS)}` });
	}

	// Navigation: sections, «Требуют внимания» with counts (zero is not shown), settings.
	const ATTENTION_NAV: Record<AttentionKey, { label: string; icon: Component }> = {
		call: { label: 'Позвонить', icon: PhoneCall },
		parts: { label: 'Заказать запчасть', icon: Package },
		longDiagnostics: { label: 'Долгая диагностика', icon: Clock },
		longReady: { label: 'Давно готовы', icon: CalendarClock }
	};
	const attentionItems = $derived<NavItem[]>(
		ATTENTION_KEYS.filter((k) => counts[k] > 0).map((k) => ({
			id: `attention:${k}`,
			label: ATTENTION_NAV[k].label,
			icon: ATTENTION_NAV[k].icon,
			href: `#${k}`,
			count: counts[k],
			attention: true
		}))
	);
	const sections = $derived<NavSection[]>([
		{
			items: [
				{ id: 'orders', label: 'Заказы', icon: Wrench, href: '#orders', count: inWork },
				{ id: 'clients', label: 'Клиенты', icon: Users, href: '#clients' },
				{ id: 'stock', label: 'Склад', icon: Boxes, href: '#stock' },
				{ id: 'reports', label: 'Отчёты', icon: ChartColumn, href: '#reports' }
			]
		},
		{
			title: 'Требуют внимания',
			items:
				attentionItems.length > 0
					? attentionItems
					: [
							{
								id: 'attention:none',
								label: 'Всё в порядке ✓',
								icon: CircleCheck,
								href: '#attention'
							}
						]
		},
		{ items: [{ id: 'settings', label: 'Настройки', icon: Settings, href: '#settings' }] }
	]);
	const BOTTOM: NavItem[] = [
		{ id: 'orders', label: 'Заказы', icon: Wrench, href: '#orders' },
		{ id: 'clients', label: 'Клиенты', icon: Users, href: '#clients' },
		{ id: 'scan', label: 'Скан', icon: ScanLine, href: '#scan' },
		{ id: 'more', label: 'Ещё', icon: Ellipsis, href: resolve('/design') }
	];
	const activeNav = $derived(attention ? `attention:${attention}` : 'orders');

	function navigate(id: string) {
		if (id === 'orders') {
			setView(() => {
				attention = null;
				chips = ['work'];
			});
			return;
		}
		const key = ATTENTION_KEYS.find((k) => id === `attention:${k}`);
		if (key) {
			setView(() => {
				attention = key;
				chips = [];
			});
		}
	}

	/** «Скан» on a phone: the camera comes with plan 5; here it opens the search by number or barcode. */
	function bottomNavigate(id: string) {
		if (id === 'scan') paletteOpen = true;
		else navigate(id);
	}

	const pointItems: MenuItem[] = [
		{ label: 'Все точки', onselect: () => setView(() => (point = null)) },
		...DEMO_POINTS.map((p) => ({ label: p.name, onselect: () => setView(() => (point = p.id)) }))
	];

	const dark = $derived(prefs.resolvedTheme === 'dark');
	function toggleTheme() {
		prefs.set({ theme: dark ? 'light' : 'dark' });
	}
	function toIntake() {
		void goto(resolve('/design/intake'));
	}

	// Ctrl K: «Недавние» for an empty query; otherwise orders (a number first), clients, commands.
	let paletteOpen = $state(false);
	let paletteQuery = $state('');
	const COMMANDS: CommandItem[] = [
		{ id: 'cmd:intake', label: 'Принять устройство', icon: Plus, shortcut: 'N' },
		{ id: 'cmd:theme', label: 'Сменить тему', icon: Moon, keywords: ['тема', 'тёмная', 'светлая'] },
		...ATTENTION_KEYS.map((k) => ({
			id: `cmd:attention:${k}`,
			label: `Требуют внимания: ${ATTENTION_NAV[k].label.toLowerCase()}`,
			icon: ATTENTION_NAV[k].icon
		})),
		{ id: 'cmd:design', label: 'Витрина дизайн-системы', icon: Settings }
	];
	function orderCommand(o: DemoOrder): CommandItem {
		const s = statusById.get(o.status_id);
		return {
			id: `order:${o.id}`,
			label: `№ ${o.id}`,
			hint: `${deviceOf(o)} · ${shortName(o.client_name)}`,
			status: s ? { name: s.name, color: s.color } : undefined
		};
	}
	const paletteGroups = $derived.by<CommandGroup[]>(() => {
		if (queryWords(paletteQuery).length === 0) {
			return [
				{
					id: 'recent',
					title: 'Недавние',
					items: recent.flatMap((id) => {
						const o = byId.get(id);
						return o ? [orderCommand(o)] : [];
					})
				},
				{ id: 'commands', title: 'Команды', items: COMMANDS }
			];
		}
		return [
			{
				id: 'orders',
				title: 'Заказы',
				items: searchOrders(orders, paletteQuery, 8).map(orderCommand)
			},
			{
				id: 'clients',
				title: 'Клиенты',
				items: searchClients(orders, paletteQuery, 5).map((c) => ({
					id: `client:${c.phone}`,
					label: displayName(c.name),
					hint: `${formatPhone(c.phone)} · ${countLabel(c.count, ORDERS_FORMS)}`
				}))
			},
			{ id: 'commands', title: 'Команды', items: filterCommands(COMMANDS, paletteQuery) }
		];
	});
	function onselect(item: CommandItem) {
		const [kind, ...rest] = item.id.split(':');
		const value = rest.join(':');
		if (kind === 'order') openOrder(Number(value));
		else if (kind === 'client') {
			setView(() => {
				attention = null;
				chips = ['all'];
				query = value;
			});
		} else if (value === 'intake') toIntake();
		else if (value === 'theme') toggleTheme();
		else if (value === 'design') void goto(resolve('/design'));
		else navigate(value);
	}

	let helpOpen = $state(false);
	const SHORTCUTS = [
		{
			title: 'Общие',
			items: [
				{ label: 'Найти или выполнить', shortcut: 'Mod+K' },
				{ label: 'Принять устройство', shortcut: 'N' },
				{ label: 'Отменить последнее действие', shortcut: 'Mod+Z' },
				{ label: 'Справка по клавишам', shortcut: '?' },
				{ label: 'Закрыть', shortcut: 'Escape' }
			]
		},
		{
			title: 'Список',
			items: [
				{ label: 'Фильтр списка', shortcut: '/' },
				{ label: 'Следующая строка', shortcut: 'ArrowDown' },
				{ label: 'Открыть заказ', shortcut: 'Enter' }
			]
		},
		{
			title: 'Панель заказа',
			items: [
				{ label: 'Соседний заказ', shortcut: 'Alt+ArrowDown' },
				{ label: 'Выдать', shortcut: 'Mod+Enter' }
			]
		}
	];

	// Keys of this screen; Ctrl/⌘+K, ? and Ctrl/⌘+Z come with the palette, the help and the toasts.
	let filterEl = $state<HTMLInputElement | null>(null);
	$effect(() => registerHotkey('N', toIntake));
	$effect(() => registerHotkey('/', () => filterEl?.focus()));
	$effect(() => {
		if (!canIssue) return;
		return registerHotkey('Mod+Enter', () => openIssue(), { allowInFields: true });
	});

	// A scanner works from anywhere outside fields and modals (spec 6.10).
	$effect(() =>
		installScanner((scan) => {
			if (layers.blocking) return;
			const action = resolveScan(scan, orders);
			if (action.kind === 'open') openOrder(action.id);
			else if (action.kind === 'missing') toast.show({ message: action.message, tone: 'danger' });
			else {
				paletteQuery = action.query;
				paletteOpen = true;
			}
		})
	);

	const orderId = (o: DemoOrder) => o.id;
	const COLUMNS: Column<DemoOrder>[] = [
		{ id: 'id', header: '№', width: 84, sortable: true, cell: numberCell },
		{ id: 'received', header: 'Принят', width: 132, sortable: true, cell: receivedCell },
		{ id: 'client', header: 'Клиент', width: 200, minWidth: 120, sortable: true, cell: clientCell },
		{ id: 'device', header: 'Устройство', width: 180, sortable: true, cell: deviceCell },
		{ id: 'malfunction', header: 'Неисправность', width: 200, cell: malfunctionCell },
		{ id: 'status', header: 'Статус', width: 232, sortable: true, cell: statusCell },
		{ id: 'master', header: 'Мастер', width: 110, sortable: true, cell: masterCell },
		{ id: 'sum', header: 'Сумма', width: 140, align: 'end', sortable: true, cell: sumCell },
		{ id: 'flags', header: 'Отметки', width: 96, cell: flagsCell },
		{ id: 'phone', header: 'Телефон', width: 150, hiddenByDefault: true, cell: phoneCell },
		{
			id: 'type',
			header: 'Тип',
			width: 120,
			sortable: true,
			hiddenByDefault: true,
			cell: typeCell
		},
		{ id: 'serial', header: 'Серийный/IMEI', width: 170, hiddenByDefault: true, cell: serialCell },
		{
			id: 'point',
			header: 'Точка',
			width: 110,
			sortable: true,
			hiddenByDefault: true,
			cell: pointCell
		},
		{
			id: 'issued',
			header: 'Выдан',
			width: 132,
			sortable: true,
			hiddenByDefault: true,
			cell: issuedCell
		},
		{
			id: 'prepayment',
			header: 'Предоплата',
			width: 120,
			align: 'end',
			sortable: true,
			hiddenByDefault: true,
			cell: prepaymentCell
		},
		{ id: 'barcode', header: 'Штрихкод', width: 140, hiddenByDefault: true, cell: barcodeCell }
	];
</script>

{#snippet numberCell(o: DemoOrder)}{o.id}{/snippet}
{#snippet receivedCell(o: DemoOrder)}
	<span title={formatFull(o.received_at)}>{formatRelative(o.received_at, DEMO_NOW)}</span>
{/snippet}
{#snippet clientCell(o: DemoOrder)}
	{#if o.client_blacklisted}
		<FlagIcons needCall={false} needParts={false} blacklisted focusable={false} />
	{/if}
	<span class="strong">{displayName(o.client_name)}</span>
{/snippet}
{#snippet deviceCell(o: DemoOrder)}<span class="strong">{deviceOf(o)}</span>{/snippet}
{#snippet malfunctionCell(o: DemoOrder)}{o.malfunction}{/snippet}
{#snippet statusCell(o: DemoOrder)}
	{@const s = statusById.get(o.status_id)}
	{#if s}<StatusBadge name={s.name} color={s.color} size="sm" />{/if}
	{#if needsAttention('longDiagnostics', o, s?.kind, DEMO_NOW)}
		<span class="clock"
			><Clock size={14} aria-hidden="true" /><VisuallyHidden>Долгая диагностика</VisuallyHidden
			></span
		>
	{/if}
{/snippet}
{#snippet masterCell(o: DemoOrder)}{o.master ? displayName(o.master) : '—'}{/snippet}
{#snippet sumCell(o: DemoOrder)}
	{#if o.final_price !== null}
		{formatMoney(o.final_price)}
	{:else if estimateText(o)}
		<span class="muted">≈ {estimateText(o)}</span>
	{:else}
		<span class="muted">—</span>
	{/if}
{/snippet}
{#snippet flagsCell(o: DemoOrder)}
	<FlagIcons needCall={o.need_call} needParts={o.need_parts} focusable={false} />
{/snippet}
{#snippet phoneCell(o: DemoOrder)}{formatPhone(o.client_phone)}{/snippet}
{#snippet typeCell(o: DemoOrder)}{o.device_type}{/snippet}
{#snippet serialCell(o: DemoOrder)}{o.serial || '—'}{/snippet}
{#snippet pointCell(o: DemoOrder)}{pointName(o.service_point_id) || '—'}{/snippet}
{#snippet issuedCell(o: DemoOrder)}
	{#if o.issued_at}
		<span title={formatFull(o.issued_at)}>{formatRelative(o.issued_at, DEMO_NOW)}</span>
	{:else}
		<span class="muted">—</span>
	{/if}
{/snippet}
{#snippet prepaymentCell(o: DemoOrder)}
	{o.prepayment > 0 ? formatMoney(o.prepayment) : '—'}
{/snippet}
{#snippet barcodeCell(o: DemoOrder)}{o.barcode}{/snippet}

{#snippet nothingFound()}
	<EmptyState title={ru.common.nothingFound} description="Измените фильтр или выберите «Все»">
		{#snippet actions()}
			<Button size="sm" onclick={() => setView(() => (query = ''))}>Сбросить фильтр</Button>
		{/snippet}
	</EmptyState>
{/snippet}

<div class="app">
	<AppShell>
		{#snippet topbar()}
			<TopBar title="MyWork">
				{#snippet search()}
					<button type="button" class="find" onclick={() => (paletteOpen = true)}>
						<Search size={14} aria-hidden="true" />
						<span>{ru.palette.title}…</span>
						<Kbd shortcut="Mod+K" />
					</button>
				{/snippet}
				{#snippet actions()}
					{#if !phone.current}
						<DropdownMenu items={pointItems} label="Точка обслуживания">
							{#snippet trigger(props)}
								<Button {...props} iconEnd={ChevronDown}>
									Точка: {point === null ? 'все' : pointName(point)}
								</Button>
							{/snippet}
						</DropdownMenu>
						<Button variant="primary" icon={Plus} shortcut="N" onclick={toIntake}>Принять</Button>
					{/if}
					<IconButton
						label={dark ? 'Светлая тема' : 'Тёмная тема'}
						icon={dark ? Sun : Moon}
						onclick={toggleTheme}
					/>
				{/snippet}
			</TopBar>
		{/snippet}
		{#snippet sidebar()}
			<SidebarNav {sections} active={activeNav} onnavigate={navigate} />
		{/snippet}
		{#snippet bottomNav()}
			<BottomNav
				items={BOTTOM}
				active={activeNav}
				primary={{ label: 'Принять', icon: Plus, onclick: toIntake }}
				onnavigate={bottomNavigate}
			/>
		{/snippet}

		<div class="content" class:phone={phone.current}>
			<FilterChips
				items={chipItems}
				label="Быстрые фильтры"
				bind:selected={
					() => chips,
					(v) =>
						setView(() => {
							chips = v;
							attention = null;
						})
				}
			/>
			<div class="filter-row">
				<div class="mw-box filter">
					<span class="mw-affix"><Search size={16} aria-hidden="true" /></span>
					<input
						bind:this={filterEl}
						class="mw-input"
						type="search"
						aria-label="Фильтр списка"
						placeholder="Фильтр: имя, телефон, модель, IMEI…"
						autocomplete="off"
						bind:value={() => query, (v) => setView(() => (query = v))}
					/>
					<span class="mw-affix"><Kbd shortcut="/" /></span>
				</div>
				<span class="count num" aria-live="polite">{countLabel(rows.length, ORDERS_FORMS)}</span>
			</div>

			{#if phone.current}
				<InfiniteList
					items={cards}
					key={orderId}
					onendreached={() => {
						if (loaded < rows.length) loaded += LIST_PAGE;
					}}
					empty={nothingFound}
				>
					{#snippet item(o)}
						{@const s = statusById.get(o.status_id)}
						<ListCard
							number="№ {o.id}"
							title="{deviceOf(o)} · {o.malfunction}"
							subtitle={shortName(o.client_name)}
							meta={formatRelative(o.received_at, DEMO_NOW)}
							status={s ? { name: s.name, color: s.color } : undefined}
							flags={{ needCall: o.need_call, needParts: o.need_parts }}
							active={openedId === o.id}
							onclick={() => openOrder(o.id)}
						/>
					{/snippet}
				</InfiniteList>
			{:else}
				<div class="table" bind:this={tableWrap}>
					<DataTable
						{rows}
						columns={COLUMNS}
						rowId={orderId}
						storageKey="demo.orders"
						ariaLabel="Заказы"
						{sort}
						onsortchange={(s) => setView(() => (sort = s))}
						selectable
						bind:selected
						bind:activeId
						onopen={(o) => openOrder(o.id)}
						empty={nothingFound}
					/>
					<div class="selection">
						<SelectionBar count={selected.size} onclear={() => (selected = new Set())}>
							<DropdownMenu items={bulkItems} label="Сменить статус">
								{#snippet trigger(props)}
									<Button {...props} variant="ghost" size="sm" iconEnd={ChevronDown}>Статус</Button>
								{/snippet}
							</DropdownMenu>
							<Button size="sm" variant="ghost" icon={Printer} onclick={printSelected}
								>Печать</Button
							>
						</SelectionBar>
					</div>
				</div>
			{/if}
		</div>
	</AppShell>
</div>

<OrderPanel
	order={opened}
	statuses={DEMO_STATUSES}
	pointName={opened ? pointName(opened.service_point_id) : ''}
	{saveState}
	returnFocusTo={phone.current ? null : grid}
	onclose={closePanel}
	onprev={prevId === undefined ? undefined : () => openOrder(prevId)}
	onnext={nextId === undefined ? undefined : () => openOrder(nextId)}
	onedit={edit}
	onflush={flush}
	onstatus={changeStatus}
	onissue={() => openIssue()}
	onreopen={reopen}
	onprint={printDocument}
	onaddphotos={addPhotos}
	onremovephoto={removePhoto}
/>
<IssueDialog bind:open={issueOpen} order={opened} onissue={issue} />
<CommandPalette
	bind:open={paletteOpen}
	bind:query={paletteQuery}
	groups={paletteGroups}
	{onselect}
/>
<ShortcutsHelp bind:open={helpOpen} groups={SHORTCUTS} />

<style>
	/* The shell fills the window: the table needs a definite height to draw only the visible rows. */
	.app {
		height: 100vh;
		height: 100dvh;
	}
	.content {
		box-sizing: border-box;
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		height: 100%;
		padding: var(--space-4);
	}
	.content.phone {
		height: auto;
		min-height: 100%;
		padding: var(--space-3);
	}
	.filter-row {
		display: flex;
		align-items: center;
		gap: var(--space-3);
	}
	.filter {
		flex: 1;
		max-width: 36rem;
	}
	.phone .filter {
		max-width: none;
	}
	.count {
		flex: none;
		color: var(--text-3);
		font-size: var(--text-sm);
	}
	.table {
		position: relative;
		flex: 1;
		min-height: 0;
		border: 1px solid var(--border);
		border-radius: var(--radius-lg);
		overflow: hidden;
	}
	.selection {
		position: absolute;
		bottom: var(--space-3);
		left: 50%;
		z-index: var(--z-sticky);
		max-width: calc(100% - 2 * var(--space-3));
		transform: translateX(-50%);
	}
	.strong {
		font-weight: 500;
	}
	.muted {
		color: var(--text-3);
	}
	.clock {
		display: inline-flex;
		margin-left: var(--space-1);
		vertical-align: middle;
		color: var(--warning-text);
	}
	.find {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		box-sizing: border-box;
		width: min(24rem, 100%);
		height: var(--control-md);
		padding: 0 var(--space-3);
		border: 1px solid var(--border-strong);
		border-radius: var(--radius-md);
		background: var(--surface-2);
		color: var(--text-3);
		font: inherit;
		cursor: pointer;
	}
	.find span {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-align: left;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	@media (pointer: coarse) {
		.find {
			min-height: 2.75rem;
		}
	}
</style>
