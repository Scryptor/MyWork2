<!-- Top bar of the program (spec 5.1): search, point, «＋ Принять», theme, the user's menu. -->
<script lang="ts">
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import CircleUser from '@lucide/svelte/icons/circle-user';
	import KeyRound from '@lucide/svelte/icons/key-round';
	import LogOut from '@lucide/svelte/icons/log-out';
	import Moon from '@lucide/svelte/icons/moon';
	import Plus from '@lucide/svelte/icons/plus';
	import Search from '@lucide/svelte/icons/search';
	import Sun from '@lucide/svelte/icons/sun';
	import { display, prefs } from '#lib/design/prefs.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';
	import DropdownMenu, { type MenuItem } from '#lib/ui/DropdownMenu.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import Kbd from '#lib/ui/Kbd.svelte';
	import TopBar from '#lib/ui/shell/TopBar.svelte';
	import { app } from './context.svelte.js';

	let {
		onsearch,
		onintake,
		onchangepassword,
		onlogout
	}: {
		onsearch: () => void;
		onintake?: () => void;
		onchangepassword: () => void;
		onlogout: () => void;
	} = $props();

	const dark = $derived(prefs.resolvedTheme === 'dark');
	const pointName = $derived(app.activePoints.find((p) => p.id === app.point)?.name ?? null);

	const pointItems = $derived<MenuItem[]>([
		{ label: ru.app.allPoints, onselect: () => (app.point = null) },
		...app.activePoints.map((p) => ({ label: p.name, onselect: () => (app.point = p.id) }))
	]);
	const userItems: MenuItem[] = [
		{ label: ru.app.changePassword, icon: KeyRound, onselect: () => onchangepassword() },
		{ label: ru.app.logout, icon: LogOut, onselect: () => onlogout() }
	];
</script>

<TopBar title={ru.app.name}>
	{#snippet search()}
		<button type="button" class="find" onclick={onsearch}>
			<Search size={14} aria-hidden="true" />
			<span>{ru.palette.title}…</span>
			<Kbd shortcut="Mod+K" />
		</button>
	{/snippet}
	{#snippet actions()}
		{#if !display.phone}
			{#if app.activePoints.length > 0}
				<DropdownMenu items={pointItems} label={ru.app.point}>
					{#snippet trigger(props)}
						<Button {...props} iconEnd={ChevronDown}>{ru.app.pointButton(pointName)}</Button>
					{/snippet}
				</DropdownMenu>
			{/if}
			{#if onintake}
				<Button variant="primary" icon={Plus} shortcut="N" onclick={onintake}
					>{ru.app.intake}</Button
				>
			{/if}
		{/if}
		<IconButton
			label={dark ? ru.app.lightTheme : ru.app.darkTheme}
			icon={dark ? Sun : Moon}
			onclick={() => prefs.set({ theme: dark ? 'light' : 'dark' })}
		/>
		{#if !app.me.open_mode && !display.phone}
			<DropdownMenu items={userItems} label={ru.app.userMenu(app.me.name)}>
				{#snippet trigger(props)}
					<Button
						{...props}
						variant="ghost"
						icon={CircleUser}
						iconEnd={ChevronDown}
						aria-label={ru.app.userMenu(app.me.name)}>{app.me.name}</Button
					>
				{/snippet}
			</DropdownMenu>
		{/if}
	{/snippet}
</TopBar>

<style>
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
