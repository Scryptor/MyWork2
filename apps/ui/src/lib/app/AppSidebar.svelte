<!-- Sidebar of the program (spec 5.1, 6.1): «Заказы», «Требуют внимания», «Мои виды». Sections
     that have no screen yet (Клиенты, Склад, Отчёты, Настройки) come with their plans. -->
<script lang="ts" module>
	import CalendarClock from '@lucide/svelte/icons/calendar-clock';
	import Clock from '@lucide/svelte/icons/clock';
	import Package from '@lucide/svelte/icons/package';
	import PhoneCall from '@lucide/svelte/icons/phone-call';
	import type { Component } from 'svelte';
	import type { AttentionKey } from './summary.svelte.js';

	/** Icons of «Требуют внимания»: the sidebar and the commands of Ctrl K. */
	export const ATTENTION_ICONS: Record<AttentionKey, Component> = {
		call: PhoneCall,
		parts: Package,
		longDiagnostics: Clock,
		longReady: CalendarClock
	};
</script>

<script lang="ts">
	import CircleCheck from '@lucide/svelte/icons/circle-check';
	import Wrench from '@lucide/svelte/icons/wrench';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { ru } from '#lib/i18n/ru.js';
	import type { NavItem, NavSection } from '#lib/ui/shell/nav.js';
	import SidebarNav from '#lib/ui/shell/SidebarNav.svelte';
	import { app } from './context.svelte.js';
	import { ATTENTION_KEYS, attentionCounts, inWorkCount, summary } from './summary.svelte.js';

	/** Saved views of the orders list (task 7). */
	let { views = [] }: { views?: NavItem[] } = $props();

	const ORDERS = resolve('orders');

	const counts = $derived(attentionCounts(summary.current));
	const attentionItems = $derived<NavItem[]>(
		counts === null
			? []
			: ATTENTION_KEYS.filter((k) => counts[k] > 0).map((k) => ({
					id: `attention:${k}`,
					label: ru.app.attentionItems[k],
					icon: ATTENTION_ICONS[k],
					href: resolve(`orders?attention=${k}`),
					count: counts[k],
					attention: true
				}))
	);
	const allClear: NavItem = {
		id: 'attention:none',
		label: ru.app.allClear,
		icon: CircleCheck,
		href: ORDERS
	};

	const sections = $derived<NavSection[]>([
		{
			items: [
				{
					id: 'orders',
					label: ru.app.orders,
					icon: Wrench,
					href: ORDERS,
					count: inWorkCount(summary.current, app.statuses)
				}
			]
		},
		...(counts === null
			? []
			: [
					{
						title: ru.app.attention,
						items: attentionItems.length > 0 ? attentionItems : [allClear]
					}
				]),
		...(views.length > 0 ? [{ title: ru.app.views, items: views }] : [])
	]);

	const active = $derived.by(() => {
		if (page.url.pathname !== ORDERS) return '';
		const key = ATTENTION_KEYS.find((k) => k === page.url.searchParams.get('attention'));
		return key ? `attention:${key}` : 'orders';
	});
</script>

<SidebarNav {sections} {active} />
