<!-- The frame of every screen of the program (spec 5.1): shell, Ctrl K, the scanner, undo, and the
     layers that open from anywhere: the order panel, the intake sheet and the preview of a document. -->
<script lang="ts">
	import Ellipsis from '@lucide/svelte/icons/ellipsis';
	import FileText from '@lucide/svelte/icons/file-text';
	import KeyRound from '@lucide/svelte/icons/key-round';
	import LogOut from '@lucide/svelte/icons/log-out';
	import Monitor from '@lucide/svelte/icons/monitor';
	import Moon from '@lucide/svelte/icons/moon';
	import Palette from '@lucide/svelte/icons/palette';
	import Plus from '@lucide/svelte/icons/plus';
	import Settings from '@lucide/svelte/icons/settings';
	import Sun from '@lucide/svelte/icons/sun';
	import Type from '@lucide/svelte/icons/type';
	import ZoomIn from '@lucide/svelte/icons/zoom-in';
	import ZoomOut from '@lucide/svelte/icons/zoom-out';
	import Users from '@lucide/svelte/icons/users';
	import Warehouse from '@lucide/svelte/icons/warehouse';
	import Wrench from '@lucide/svelte/icons/wrench';
	import { untrack } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import type { OrderListItem } from '$api/OrderListItem';
	import { logout, revokeToken } from '#lib/api/auth.js';
	import { ApiError, errorMessage, setUnauthenticatedHandler } from '#lib/api/client.js';
	import { getClient, listClients } from '#lib/api/clients.js';
	import { getOrder, listOrders, ordersByBarcode } from '#lib/api/orders.js';
	import {
		clearToken,
		expireToken,
		setSessionLostHandler,
		type LostReason
	} from '#lib/api/session.js';
	import AppSidebar, { ATTENTION_ICONS } from '#lib/app/AppSidebar.svelte';
	import AppTopBar from '#lib/app/AppTopBar.svelte';
	import ChangePasswordDialog from '#lib/app/ChangePasswordDialog.svelte';
	import { app } from '#lib/app/context.svelte.js';
	import { loginPath } from '#lib/app/login.js';
	import { clearLoginNotices, queueLoginNotice } from '#lib/app/login-notice.js';
	import { discardQueues, resetSession, settleQueues } from '#lib/app/session-reset.js';
	import UnsavedLogoutDialog from '#lib/app/UnsavedLogoutDialog.svelte';
	import {
		createPaletteSearch,
		loadRecent,
		paletteGroups,
		type PaletteResults
	} from '#lib/app/palette.js';
	import { recentIds } from '#lib/app/recent.js';
	import { resolveScan } from '#lib/app/scan.js';
	import ScanChoiceDialog from '#lib/app/ScanChoiceDialog.svelte';
	import { ATTENTION_KEYS, summary } from '#lib/app/summary.svelte.js';
	import { undo } from '#lib/app/undo.svelte.js';
	import { forgetDraft } from '#lib/intake/draft.js';
	import { clientHref } from '#lib/clients/view.js';
	import { registerHotkey } from '#lib/design/hotkeys.svelte.js';
	import { layers } from '#lib/design/layers.svelte.js';
	import { prefs, type ThemePref } from '#lib/design/prefs.svelte.js';
	import { installScanner, type ScanResult } from '#lib/design/scanner.js';
	import { ru } from '#lib/i18n/ru.js';
	import { autoSize, stepSize } from '#lib/settings/size.js';
	import { intake } from '#lib/intake/intake.svelte.js';
	import IntakeSheet from '#lib/intake/IntakeSheet.svelte';
	import { onOrderChanged } from '#lib/orders/events.js';
	import { stockCount } from '#lib/stock/summary.svelte.js';
	import DisputeDialog from '#lib/orders/DisputeDialog.svelte';
	import OrderPanel from '#lib/orders/OrderPanel.svelte';
	import { orderPanel } from '#lib/orders/panel.svelte.js';
	import DocumentPreview from '#lib/print/DocumentPreview.svelte';
	import { preview, printDocument } from '#lib/print/print.svelte.js';
	import AppShell from '#lib/ui/shell/AppShell.svelte';
	import BottomNav from '#lib/ui/shell/BottomNav.svelte';
	import type { NavItem } from '#lib/ui/shell/nav.js';
	import Button from '#lib/ui/Button.svelte';
	import CommandPalette from '#lib/ui/CommandPalette.svelte';
	import type { CommandItem } from '#lib/ui/commands.js';
	import Dialog from '#lib/ui/Dialog.svelte';
	import Select from '#lib/ui/Select.svelte';
	import ShortcutsHelp from '#lib/ui/ShortcutsHelp.svelte';
	import { toast } from '#lib/ui/toast/toast.svelte.js';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();

	const canIntake = $derived(app.can('orders_create'));
	const here = () => page.url.pathname + page.url.search;

	// A session that ends while the program is open (expired, closed elsewhere): sign in again here.
	$effect(() => {
		setUnauthenticatedHandler(() => {
			expireToken();
			if (page.url.pathname !== '/login') void goto(resolve(loginPath(here())));
		});
		return () => setUnauthenticatedHandler(() => {});
	});

	// Another tab signed out, or in as someone else: this tab's session is over. Its edits are given up
	// (the token they belong to is closed, and the next one is not this user's), its screens go.
	$effect(() => {
		setSessionLostHandler((reason, token) => void sessionEnded(reason, token));
		return () => setSessionLostHandler(() => {});
	});
	async function sessionEnded(reason: LostReason, token: string) {
		// The token this tab had is closed on the server (nobody else uses it); an expired one is dead.
		if (reason === 'changed') void revokeToken(token);
		discardQueues();
		// The login page shows the notice after its own reset (which clears toasts). `loginPath` always
		// leads to `/login`, and this layout is not mounted there, so the page does mount and takes it;
		// a notice left from an earlier end that never reached the page is dropped first.
		clearLoginNotices();
		queueLoginNotice({
			message: reason === 'expired' ? ru.auth.sessionExpired : ru.auth.sessionChanged
		});
		// The login page resets everything the user left in memory when it opens.
		await goto(resolve(loginPath(here())));
	}

	// Signing out (spec 6.11): the edits still on their way go first, under this user's token; what cannot
	// be sent in 5 s is asked about. Then the token goes, the login page opens, and everything the user
	// left in memory is reset (the screens are gone by then, so none reads a reset singleton).
	let unsavedOpen = $state(false);
	let unsavedBusy = $state(false);
	let unsavedFailed = $state(false);
	let leaving = false;
	async function signOut() {
		if (leaving) return;
		leaving = true;
		try {
			if (await settleQueues()) await finishSignOut();
			else {
				unsavedFailed = false;
				unsavedOpen = true;
			}
		} finally {
			leaving = false;
		}
	}
	async function finishSignOut() {
		try {
			await logout();
		} catch {
			// No connection: the token is dropped on this device anyway.
		}
		clearToken();
		await goto(resolve('/login'));
		resetSession();
		// Only signing out takes the draft of the intake away; an expired session keeps it.
		forgetDraft();
	}
	async function saveAndSignOut() {
		unsavedBusy = true;
		unsavedFailed = false;
		try {
			if (await settleQueues()) {
				unsavedOpen = false;
				await finishSignOut();
			} else unsavedFailed = true;
		} finally {
			unsavedBusy = false;
		}
	}
	async function leaveUnsaved() {
		discardQueues();
		unsavedOpen = false;
		await finishSignOut();
	}

	let passwordOpen = $state(false);
	/** The server closed every session of the user: sign in with the new password. */
	async function passwordChanged() {
		clearToken();
		const to = resolve(loginPath(here()));
		// The login page resets the session (and its toasts): it shows the notice after that.
		clearLoginNotices();
		queueLoginNotice({ message: ru.password.changed, tone: 'success' });
		await goto(to);
	}

	// Counts of the sidebar (and of the quick filters): at the start, for another point, after edits.
	$effect(() => summary.refresh(app.point));
	$effect(() => onOrderChanged(() => summary.refresh(app.point)));
	// «Склад» of the sidebar counts what is to be bought; it is the same for every point.
	$effect(() => {
		if (app.can('stock_view')) stockCount.refresh();
	});

	// Ctrl K (spec 6.10).
	let paletteOpen = $state(false);
	let paletteQuery = $state('');
	let results = $state.raw<PaletteResults | null>(null);
	let recent = $state.raw<OrderListItem[]>([]);
	const search = createPaletteSearch(
		{ listOrders, getOrder, listClients },
		{
			result: (r) => (results = r),
			// The query is answered (nothing found), so the spinner stops; the toast says why.
			error: (e, query) => {
				results = { query, orders: [], exact: null, clients: [] };
				toast.show({ message: errorMessage(e), tone: 'danger' });
			}
		}
	);
	$effect(() => () => search.cancel());
	$effect(() => {
		if (!paletteOpen) return untrack(() => search.cancel());
		const query = paletteQuery;
		untrack(() => search.search(query));
	});
	$effect(() => {
		if (!paletteOpen) return;
		untrack(() => {
			void loadRecent(recentIds(), { getOrder, getClient }).then((r) => (recent = r));
		});
	});
	// Only answers to the query in the field: Enter never opens what an older query found.
	const current = $derived(
		results !== null && results.query === paletteQuery.trim() ? results : null
	);
	const searching = $derived(paletteQuery.trim() !== '' && current === null);

	const THEME_ICONS = { light: Sun, dark: Moon, system: Monitor } as const;
	const commands = $derived<CommandItem[]>([
		...(canIntake
			? [{ id: 'cmd:intake', label: ru.app.intakeDevice, icon: Plus, shortcut: 'N' }]
			: []),
		...(['light', 'dark', 'system'] as const).map((t) => ({
			id: `cmd:theme:${t}`,
			label: ru.app.themeCommand[t],
			icon: THEME_ICONS[t]
		})),
		{
			id: 'cmd:blankAct',
			label: ru.print.documents.blank_intake_act,
			icon: FileText,
			keywords: [ru.print.print, ru.print.blankKeyword]
		},
		{ id: 'cmd:settings', label: ru.app.settings, icon: Settings },
		{ id: 'cmd:appearance', label: ru.settings.appearance, icon: Palette },
		{ id: 'cmd:sizeUp', label: ru.shortcuts.sizeUp, icon: ZoomIn, shortcut: 'Mod+Plus' },
		{ id: 'cmd:sizeDown', label: ru.shortcuts.sizeDown, icon: ZoomOut, shortcut: 'Mod+Minus' },
		{ id: 'cmd:sizeAuto', label: ru.shortcuts.sizeAuto, icon: Type, shortcut: 'Mod+0' },
		...ATTENTION_KEYS.map((k) => ({
			id: `cmd:attention:${k}`,
			label: `${ru.app.attention}: ${ru.app.attentionItems[k].toLowerCase()}`,
			icon: ATTENTION_ICONS[k]
		}))
	]);
	const groups = $derived(
		paletteGroups({
			query: paletteQuery,
			orders: current?.orders ?? [],
			exact: current?.exact ?? null,
			clients: current?.clients ?? [],
			recent,
			commands,
			statusOf: (id) => app.statusById(id)
		})
	);

	function onselect(item: CommandItem, opts: { alt: boolean }) {
		const [kind, ...rest] = item.id.split(':');
		const value = rest.join(':');
		if (kind === 'order') {
			const id = Number(value);
			if (!opts.alt) return void orderPanel.open(id);
			return void goto(resolve(`orders/${id}`));
		}
		// A client: their panel on «Клиенты», with their orders.
		if (kind === 'client') return void goto(resolve(clientHref(Number(value))));
		const [command, arg] = value.split(':');
		if (command === 'intake') intake.open();
		else if (command === 'blankAct') void printDocument('blank_intake_act', {});
		else if (command === 'theme') prefs.set({ theme: arg as ThemePref });
		else if (command === 'settings') void goto(resolve('settings'));
		else if (command === 'appearance') void goto(resolve('settings/appearance'));
		else if (command === 'sizeUp') stepSize(1);
		else if (command === 'sizeDown') stepSize(-1);
		else if (command === 'sizeAuto') autoSize();
		else if (command === 'attention') void goto(resolve(`orders?attention=${arg}`));
	}

	// The scanner works from anywhere outside fields and modals; a scan into a field stays there.
	let choice = $state.raw<{ code: string; orders: OrderListItem[] }>({ code: '', orders: [] });
	let choiceOpen = $state(false);
	const byNumber = (n: number) =>
		getOrder(n).catch((e: unknown) => {
			if (e instanceof ApiError && e.status === 404) return null;
			throw e;
		});
	async function onscan(scan: ScanResult) {
		try {
			const action = await resolveScan(scan, { byBarcode: ordersByBarcode, byNumber });
			if (action.kind === 'open') void orderPanel.open(action.id);
			else if (action.kind === 'choose') {
				choice = { code: scan.kind === 'barcode' ? scan.value : '', orders: action.orders };
				choiceOpen = true;
			} else if (action.kind === 'notFound') toast.show({ message: action.text, tone: 'danger' });
			else {
				paletteQuery = action.text;
				paletteOpen = true;
			}
		} catch (e) {
			toast.show({ message: errorMessage(e), tone: 'danger' });
		}
	}
	$effect(() =>
		installScanner((scan) => {
			if (!layers.blocking) void onscan(scan);
		})
	);

	// Keys of the frame; Ctrl/⌘+K and ? come with the palette and the help.
	$effect(() => {
		if (canIntake) return registerHotkey('N', () => intake.open());
	});
	$effect(() => registerHotkey('Mod+Z', () => void undo.run()));
	// The size of the program replaces the browser zoom (spec decision 49): the keys work in fields and
	// over dialogs, like the zoom they replace.
	const SIZE_KEYS = { allowInFields: true, allowWithOverlay: true };
	$effect(() => registerHotkey('Mod+Plus', () => stepSize(1), SIZE_KEYS));
	$effect(() => registerHotkey('Mod+Minus', () => stepSize(-1), SIZE_KEYS));
	$effect(() => registerHotkey('Mod+0', () => autoSize(), SIZE_KEYS));
	const SHORTCUTS = [
		{
			title: ru.shortcuts.general,
			items: [
				{ label: ru.shortcuts.find, shortcut: 'Mod+K' },
				{ label: ru.shortcuts.intake, shortcut: 'N' },
				{ label: ru.shortcuts.undo, shortcut: 'Mod+Z' },
				{ label: ru.shortcuts.help, shortcut: '?' },
				{ label: ru.shortcuts.close, shortcut: 'Escape' }
			]
		},
		{
			title: ru.shortcuts.display,
			items: [
				{ label: ru.shortcuts.sizeUp, shortcut: 'Mod+Plus' },
				{ label: ru.shortcuts.sizeDown, shortcut: 'Mod+Minus' },
				{ label: ru.shortcuts.sizeAuto, shortcut: 'Mod+0' }
			]
		},
		{
			title: ru.shortcuts.list,
			items: [
				{ label: ru.shortcuts.filter, shortcut: '/' },
				{ label: ru.shortcuts.nextRow, shortcut: 'ArrowDown' },
				{ label: ru.shortcuts.openOrder, shortcut: 'Enter' }
			]
		},
		{
			title: ru.shortcuts.panel,
			items: [
				{ label: ru.shortcuts.neighbour, shortcut: 'Alt+ArrowDown' },
				{ label: ru.shortcuts.issue, shortcut: 'Mod+Enter' },
				{ label: ru.shortcuts.printAct, shortcut: 'Mod+P' }
			]
		}
	];
	let helpOpen = $state(false);

	// Phone (spec 5.6): «Заказы», «Клиенты», «＋», «Ещё» with the stock, the settings and the user's actions.
	let moreOpen = $state(false);
	const canSeeClients = $derived(app.can('clients_view') || app.can('orders_create'));
	const BOTTOM = $derived<NavItem[]>([
		{ id: 'orders', label: ru.app.orders, icon: Wrench, href: resolve('orders') },
		...(canSeeClients
			? [{ id: 'clients', label: ru.app.clients, icon: Users, href: resolve('clients') }]
			: []),
		{ id: 'more', label: ru.common.more, icon: Ellipsis }
	]);
	const canSeeStock = $derived(app.can('stock_view'));
	const bottomActive = $derived(
		page.url.pathname === resolve('orders') || page.url.pathname.startsWith(`${resolve('orders')}/`)
			? 'orders'
			: page.url.pathname === resolve('clients')
				? 'clients'
				: ''
	);
	/** The point of the top bar, on a phone in «Ещё»; 0 stands for «Все точки» (ids start at 1). */
	const ALL_POINTS = 0;
	const pointItems = $derived([
		{ value: ALL_POINTS, label: ru.app.allPoints },
		...app.activePoints.map((p) => ({ value: p.id, label: p.name }))
	]);
	function bottomNavigate(id: string) {
		if (id === 'more') moreOpen = true;
	}
</script>

<div class="app">
	<AppShell>
		{#snippet topbar()}
			<AppTopBar
				onsearch={() => (paletteOpen = true)}
				onintake={canIntake ? () => intake.open() : undefined}
				onchangepassword={() => (passwordOpen = true)}
				onlogout={() => void signOut()}
			/>
		{/snippet}
		{#snippet sidebar()}
			<AppSidebar />
		{/snippet}
		{#snippet bottomNav()}
			<BottomNav
				items={BOTTOM}
				active={bottomActive}
				primary={canIntake
					? { label: ru.app.intake, icon: Plus, onclick: () => intake.open() }
					: undefined}
				onnavigate={bottomNavigate}
			/>
		{/snippet}
		{@render children()}
	</AppShell>
</div>

<OrderPanel />
<DisputeDialog />

<IntakeSheet />
<DocumentPreview request={preview.current} onclose={() => preview.close()} />

<CommandPalette
	bind:open={paletteOpen}
	bind:query={paletteQuery}
	{groups}
	loading={searching}
	{onselect}
/>
<ScanChoiceDialog
	bind:open={choiceOpen}
	code={choice.code}
	orders={choice.orders}
	onchoose={(id) => void orderPanel.open(id)}
/>
<UnsavedLogoutDialog
	bind:open={unsavedOpen}
	busy={unsavedBusy}
	failed={unsavedFailed}
	onsave={() => void saveAndSignOut()}
	onleave={() => void leaveUnsaved()}
/>
<ShortcutsHelp bind:open={helpOpen} groups={SHORTCUTS} />
{#if !app.me.open_mode}
	<ChangePasswordDialog bind:open={passwordOpen} onchanged={() => void passwordChanged()} />
{/if}
<Dialog bind:open={moreOpen} title={ru.common.more} position="bottom">
	<div class="more">
		{#if app.activePoints.length > 0}
			<Select
				label={ru.app.point}
				items={pointItems}
				bind:value={
					() => app.point ?? ALL_POINTS,
					(v) => (app.point = v === null || v === ALL_POINTS ? null : v)
				}
			/>
		{/if}
		{#if canSeeStock}
			<Button icon={Warehouse} href={resolve('stock')} onclick={() => (moreOpen = false)}
				>{ru.app.stock}</Button
			>
		{/if}
		<Button icon={Settings} href={resolve('settings')} onclick={() => (moreOpen = false)}
			>{ru.app.settings}</Button
		>
		{#if !app.me.open_mode}
			<Button
				icon={KeyRound}
				onclick={() => {
					moreOpen = false;
					passwordOpen = true;
				}}>{ru.app.changePassword}</Button
			>
			<Button
				icon={LogOut}
				onclick={() => {
					moreOpen = false;
					void signOut();
				}}>{ru.app.logout}</Button
			>
		{/if}
	</div>
</Dialog>

<style>
	/* The shell fills the window: tables need a definite height to draw only the visible rows. */
	.app {
		height: 100vh;
		height: 100dvh;
	}
	.more {
		display: flex;
		flex-direction: column;
		align-items: stretch;
		gap: var(--space-3);
	}
</style>
