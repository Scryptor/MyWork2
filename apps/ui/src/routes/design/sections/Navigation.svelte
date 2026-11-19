<script lang="ts">
	import Bookmark from '@lucide/svelte/icons/bookmark';
	import Gauge from '@lucide/svelte/icons/gauge';
	import Package from '@lucide/svelte/icons/package';
	import PhoneCall from '@lucide/svelte/icons/phone-call';
	import Plus from '@lucide/svelte/icons/plus';
	import Search from '@lucide/svelte/icons/search';
	import Settings from '@lucide/svelte/icons/settings';
	import Users from '@lucide/svelte/icons/users';
	import Wrench from '@lucide/svelte/icons/wrench';
	import { ru } from '#lib/i18n/ru.js';
	import {
		AppShell,
		BottomNav,
		Button,
		CommandPalette,
		FilterChips,
		Kbd,
		ShortcutsHelp,
		SidebarNav,
		TabPane,
		Tabs,
		TextInput,
		TopBar,
		filterCommands,
		type CommandGroup,
		type CommandItem,
		type NavItem,
		type NavSection
	} from '#lib/ui/index.js';

	const MAIN: NavItem[] = [
		{ id: 'orders', label: 'Заказы', icon: Wrench, href: '#nav-orders', count: 53 },
		{ id: 'clients', label: 'Клиенты', icon: Users, href: '#nav-clients' },
		{ id: 'stock', label: 'Склад', icon: Package, href: '#nav-stock' },
		{ id: 'reports', label: 'Отчёты', icon: Gauge, href: '#nav-reports' }
	];
	const ATTENTION: NavItem[] = [
		{
			id: 'call',
			label: 'Позвонить',
			icon: PhoneCall,
			href: '#nav-call',
			count: 4,
			attention: true
		},
		{
			id: 'parts',
			label: 'Заказать з/ч',
			icon: Package,
			href: '#nav-parts',
			count: 10,
			attention: true
		}
	];
	// «Мои виды»: an item with «✕» (NavItem.remove), shown with the labels only.
	let views = $state(['Готовые Samsung', 'Звонки мастера']);
	const SECTIONS = $derived<NavSection[]>([
		{ items: MAIN },
		{ title: 'Требуют внимания', items: ATTENTION },
		...(views.length > 0
			? [
					{
						title: 'Мои виды',
						items: views.map((name) => ({
							id: `view:${name}`,
							label: name,
							icon: Bookmark,
							href: '#nav-view',
							remove: {
								label: 'Удалить вид',
								onclick: () => (views = views.filter((v) => v !== name))
							}
						}))
					}
				]
			: []),
		{ items: [{ id: 'settings', label: 'Настройки', icon: Settings, href: '#nav-settings' }] }
	]);
	const BOTTOM: NavItem[] = [MAIN[0]!, MAIN[1]!, MAIN[3]!];

	const CHIPS = [
		{ id: 'work', label: 'В работе', count: 53 },
		{ id: 'diag', label: 'Диагностика', count: 21, color: 'amber' },
		{ id: 'ready', label: 'Готов', count: 88, color: 'green' },
		{ id: 'all', label: 'Все' }
	];
	const TABS = [
		{ value: 'info', label: 'Сведения' },
		{ value: 'history', label: 'История', count: 7 },
		{ value: 'parts', label: 'Запчасти' }
	];
	const SHORTCUTS = [
		{
			title: 'Общие',
			items: [
				{ label: 'Найти или выполнить', shortcut: 'Mod+K' },
				{ label: 'Справка по клавишам', shortcut: '?' },
				{ label: 'Закрыть', shortcut: 'Escape' }
			]
		},
		{
			title: 'Заказы',
			items: [
				{ label: 'Принять устройство', shortcut: 'N' },
				{ label: 'Фильтр списка', shortcut: '/' }
			]
		}
	];
	const COMMANDS: CommandItem[] = [
		{ id: 'intake', label: 'Принять устройство', icon: Plus, shortcut: 'N' },
		{ id: 'clients', label: 'Клиенты', icon: Users },
		{ id: 'theme', label: 'Сменить тему', keywords: ['тема'] },
		{ id: 'settings', label: 'Настройки', icon: Settings }
	];
	const ORDERS: CommandItem[] = [
		{
			id: '10481',
			label: '№ 10481',
			hint: 'SAMSUNG GALAXY S21 · ИВАНОВ ИВАН',
			status: { name: 'Диагностика', color: 'amber' }
		}
	];

	let active = $state('orders');
	let chips = $state<string[]>(['work']);
	let tab = $state('info');
	let paletteOpen = $state(false);
	let query = $state('');
	let helpOpen = $state(false);
	let chosen = $state('');
	let probe = $state('');

	const groups = $derived<CommandGroup[]>([
		{ id: 'orders', title: 'Заказы', items: filterCommands(ORDERS, query) },
		{ id: 'commands', title: 'Команды', items: filterCommands(COMMANDS, query) }
	]);
</script>

<div class="frame">
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
					<Button size="sm" onclick={() => (helpOpen = true)}>Клавиши</Button>
				{/snippet}
			</TopBar>
		{/snippet}
		{#snippet sidebar()}
			<SidebarNav sections={SECTIONS} {active} onnavigate={(id) => (active = id)} />
		{/snippet}
		{#snippet bottomNav()}
			<BottomNav
				items={BOTTOM}
				{active}
				primary={{ label: 'Принять', icon: Plus, onclick: () => (chosen = 'Выбрано: Принять') }}
				onnavigate={(id) => (active = id)}
			/>
		{/snippet}
		<div class="content">
			<FilterChips items={CHIPS} bind:selected={chips} label="Быстрые фильтры" />
			<!-- TabPane: the panes are the page's own markup, one for any layout (the full order card). -->
			<Tabs bind:value={tab} items={TABS}>
				{#each TABS as item (item.value)}
					<TabPane value={item.value}><p>Содержимое: {item.value}</p></TabPane>
				{/each}
			</Tabs>
			<div class="row">
				<Button variant="secondary" onclick={() => (paletteOpen = true)}>Открыть палитру</Button>
				<Button variant="secondary" onclick={() => (helpOpen = true)}>Справка по клавишам</Button>
			</div>
			<TextInput label="Поле для проверки" bind:value={probe} />
			<p aria-live="polite">{chosen}</p>
		</div>
	</AppShell>
</div>

<CommandPalette
	bind:open={paletteOpen}
	bind:query
	{groups}
	onselect={(item, { alt }) => (chosen = `Выбрано: ${item.label}${alt ? ' (полная)' : ''}`)}
/>
<ShortcutsHelp bind:open={helpOpen} groups={SHORTCUTS} />

<style>
	.frame {
		height: 34rem;
		border: 1px solid var(--border);
		border-radius: var(--radius-lg);
		overflow: hidden;
	}
	.content {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
		padding: var(--space-4);
	}
	.row {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
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
		text-align: left;
	}
</style>
