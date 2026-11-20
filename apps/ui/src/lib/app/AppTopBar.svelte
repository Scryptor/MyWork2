<!-- Top bar of the program (spec 5.1): search, point, «＋ Принять», theme, the user's menu. With the narrow
     menu (rail) the point and the user's menu are icons with a tooltip; with the full one their names are
     cut at a limit, whole in the tooltip of the browser: the search keeps its room on every screen. -->
<script lang="ts">
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import CircleUser from '@lucide/svelte/icons/circle-user';
	import KeyRound from '@lucide/svelte/icons/key-round';
	import LogOut from '@lucide/svelte/icons/log-out';
	import MapPin from '@lucide/svelte/icons/map-pin';
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

	/** The name of a point or of the user: at most this wide, the rest is cut (Button cuts the text). */
	const NAMED = 'max-width: var(--topbar-named-max)';

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
	const pointLabel = $derived(ru.app.pointButton(pointName));
	const userLabel = $derived(ru.app.userMenu(app.me.name));
	const iconsOnly = $derived(display.value.layout !== 'full');

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
			<!-- A phone leaves the whole room to the words (and a touch screen has no keys: Kbd). -->
			{#if !display.phone}<Kbd shortcut="Mod+K" />{/if}
		</button>
	{/snippet}
	{#snippet actions()}
		{#if !display.phone}
			{#if app.activePoints.length > 0}
				<DropdownMenu items={pointItems} label={ru.app.point}>
					{#snippet trigger(props)}
						{#if iconsOnly}
							<IconButton {...props} label={pointLabel} icon={MapPin} variant="secondary" />
						{:else}
							<Button {...props} style={NAMED} title={pointLabel} iconEnd={ChevronDown}
								>{pointLabel}</Button
							>
						{/if}
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
			<DropdownMenu items={userItems} label={userLabel}>
				{#snippet trigger(props)}
					{#if iconsOnly}
						<IconButton {...props} label={userLabel} icon={CircleUser} />
					{:else}
						<Button
							{...props}
							style={NAMED}
							variant="ghost"
							icon={CircleUser}
							iconEnd={ChevronDown}
							title={app.me.name}
							aria-label={userLabel}>{app.me.name}</Button
						>
					{/if}
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
		width: 100%;
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
			min-height: var(--control-md);
		}
	}
</style>
