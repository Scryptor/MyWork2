<!-- The frame of every screen of the program (spec 5.1): shell, Ctrl K, the scanner, undo, and the
     layers that open from anywhere: the order panel and the intake sheet. -->
<script lang="ts">
	import Ellipsis from '@lucide/svelte/icons/ellipsis';
	import KeyRound from '@lucide/svelte/icons/key-round';
	import LogOut from '@lucide/svelte/icons/log-out';
	import Monitor from '@lucide/svelte/icons/monitor';
	import Moon from '@lucide/svelte/icons/moon';
	import Plus from '@lucide/svelte/icons/plus';
	import Sun from '@lucide/svelte/icons/sun';
	import Wrench from '@lucide/svelte/icons/wrench';
	import { untrack } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import type { OrderListItem } from '$api/OrderListItem';
	import { logout } from '#lib/api/auth.js';
	import { ApiError, errorMessage, setUnauthenticatedHandler } from '#lib/api/client.js';
	import { getClient, listClients } from '#lib/api/clients.js';
	import { getOrder, listOrders, ordersByBarcode } from '#lib/api/orders.js';
	import { clearToken } from '#lib/api/session.js';
	import AppSidebar, { ATTENTION_ICONS } from '#lib/app/AppSidebar.svelte';
	import AppTopBar from '#lib/app/AppTopBar.svelte';
	import ChangePasswordDialog from '#lib/app/ChangePasswordDialog.svelte';
	import { app } from '#lib/app/context.svelte.js';
	import { loginPath } from '#lib/app/login.js';
	import {
		createPaletteSearch,
		loadRecent,
		paletteGroups,
		type PaletteResults
	} from '#lib/app/palette.js';
	import { pushRecent, recentIds } from '#lib/app/recent.js';
	import { resolveScan } from '#lib/app/scan.js';
	import ScanChoiceDialog from '#lib/app/ScanChoiceDialog.svelte';
	import { ATTENTION_KEYS, summary } from '#lib/app/summary.svelte.js';
	import { undo } from '#lib/app/undo.svelte.js';
	import { registerHotkey } from '#lib/design/hotkeys.svelte.js';
	import { layers } from '#lib/design/layers.svelte.js';
	import { prefs, type ThemePref } from '#lib/design/prefs.svelte.js';
	import { installScanner, type ScanResult } from '#lib/design/scanner.js';
	import { ru } from '#lib/i18n/ru.js';
	import { intake } from '#lib/intake/intake.svelte.js';
	import { onOrderChanged } from '#lib/orders/events.js';
	import OrderPanel from '#lib/orders/OrderPanel.svelte';
	import { orderPanel } from '#lib/orders/panel.svelte.js';
	import AppShell from '#lib/ui/shell/AppShell.svelte';
	import BottomNav from '#lib/ui/shell/BottomNav.svelte';
	import type { NavItem } from '#lib/ui/shell/nav.js';
	import Button from '#lib/ui/Button.svelte';
	import CommandPalette from '#lib/ui/CommandPalette.svelte';
	import type { CommandItem } from '#lib/ui/commands.js';
	import Dialog from '#lib/ui/Dialog.svelte';
	import SegmentedControl from '#lib/ui/SegmentedControl.svelte';
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
			clearToken();
			if (page.url.pathname !== '/login') void goto(resolve(loginPath(here())));
		});
		return () => setUnauthenticatedHandler(() => {});
	});

	async function signOut() {
		try {
			await logout();
		} catch {
			// No connection: the token is dropped on this device anyway.
		}
		clearToken();
		await goto(resolve('/login'));
	}

	let passwordOpen = $state(false);
	/** The server closed every session of the user: sign in with the new password. */
	function passwordChanged() {
		clearToken();
		toast.show({ message: ru.password.changed, tone: 'success' });
		void goto(resolve(loginPath(here())));
	}

	// Counts of the sidebar (and of the quick filters): at the start, for another point, after edits.
	$effect(() => summary.refresh(app.point));
	$effect(() => onOrderChanged(() => summary.refresh(app.point)));

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
			pushRecent(id);
			// The full card comes with task 9 (route /orders/[id]); then this becomes resolve().
			// eslint-disable-next-line svelte/no-navigation-without-resolve
			return void goto(`/orders/${id}`);
		}
		// A client's orders: all of them, issued too.
		if (kind === 'client') return void goto(resolve(`orders?client=${value}&chips=all`));
		const [command, arg] = value.split(':');
		if (command === 'intake') intake.open();
		else if (command === 'theme') prefs.set({ theme: arg as ThemePref });
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
				{ label: ru.shortcuts.issue, shortcut: 'Mod+Enter' }
			]
		}
	];
	let helpOpen = $state(false);

	// Phone (spec 5.6): «Заказы», «＋», «Ещё» with the theme and the user's actions.
	let moreOpen = $state(false);
	const BOTTOM: NavItem[] = [
		{ id: 'orders', label: ru.app.orders, icon: Wrench, href: resolve('orders') },
		{ id: 'more', label: ru.common.more, icon: Ellipsis }
	];
	const THEMES = (['system', 'light', 'dark'] as const).map((value) => ({
		value,
		label: ru.app.themes[value]
	}));
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
				active={page.url.pathname === resolve('orders') ? 'orders' : ''}
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

<!-- Task 10: IntakeSheet mounts here (intake.isOpen, intake.prefill, intake.close). -->

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
<ShortcutsHelp bind:open={helpOpen} groups={SHORTCUTS} />
{#if !app.me.open_mode}
	<ChangePasswordDialog bind:open={passwordOpen} onchanged={passwordChanged} />
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
		<SegmentedControl
			label={ru.app.theme}
			options={THEMES}
			bind:value={() => prefs.value.theme, (v) => prefs.set({ theme: v })}
		/>
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
