<!-- «Пользователи»: who works in the program — name, login («без входа» for one that does not sign in), role,
     «Мастер». A row opens the user; the disabled ones are a group of their own. Right: «users_manage». -->
<script lang="ts">
	import Plus from '@lucide/svelte/icons/plus';
	import { onMount } from 'svelte';
	import type { Role } from '$api/Role';
	import type { User } from '$api/User';
	import { listRoles, listUsers } from '#lib/api/users.js';
	import { errorMessage } from '#lib/api/client.js';
	import { app } from '#lib/app/context.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import UserAddDialog from '#lib/settings/UserAddDialog.svelte';
	import UserEditDialog from '#lib/settings/UserEditDialog.svelte';
	import { signInLine, userTitle } from '#lib/settings/users.js';
	import Badge from '#lib/ui/Badge.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import Card from '#lib/ui/Card.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import Skeleton from '#lib/ui/Skeleton.svelte';

	const t = ru.users;
	const allowed = app.can('users_manage');

	let users = $state.raw<User[]>([]);
	let roles = $state.raw<Role[]>([]);
	let loading = $state(allowed);
	let failure = $state<string | null>(null);
	let adding = $state(false);
	let editingId = $state<number | null>(null);

	const roleName = (id: number) => roles.find((r) => r.id === id)?.name ?? '';
	const byName = (a: User, b: User) =>
		userTitle(a).localeCompare(userTitle(b), 'ru', { sensitivity: 'base' });
	const active = $derived(users.filter((u) => u.active).sort(byName));
	const disabled = $derived(users.filter((u) => !u.active).sort(byName));
	const editing = $derived(users.find((u) => u.id === editingId));

	/** An answer of the server about one user (a new one, a changed one) goes into the list. */
	function put(u: User) {
		users = users.some((x) => x.id === u.id)
			? users.map((x) => (x.id === u.id ? u : x))
			: [...users, u];
	}

	onMount(async () => {
		if (!allowed) return;
		try {
			[users, roles] = await Promise.all([listUsers(), listRoles()]);
		} catch (e) {
			failure = errorMessage(e);
		} finally {
			loading = false;
		}
	});
</script>

<h1 class="title">{ru.settings.users}</h1>

{#if !allowed}
	<Callout tone="info">{t.rights}</Callout>
{:else}
	<div><Button icon={Plus} variant="primary" onclick={() => (adding = true)}>{t.add}</Button></div>
	{#if failure}
		<Callout tone="danger">{t.loadFailed}: {failure}</Callout>
	{:else if loading}
		<Skeleton height="2rem" />
	{:else if users.length === 0}
		<EmptyState title={t.empty} />
	{:else}
		{#snippet group(list: User[], label: string, muted: boolean)}
			<Card>
				<ul class="list" aria-label={label}>
					{#each list as u (u.id)}
						<li>
							<button
								type="button"
								class="row"
								class:muted
								aria-label={t.edit(userTitle(u))}
								onclick={() => (editingId = u.id)}
							>
								<span class="who">
									<span class="name">{userTitle(u)}</span>
									<span class="sub">{signInLine(u)}</span>
								</span>
								<span class="tags">
									<span class="role">{roleName(u.role_id)}</span>
									{#if u.is_master}<Badge tone="accent">{t.master}</Badge>{/if}
								</span>
							</button>
						</li>
					{/each}
				</ul>
			</Card>
		{/snippet}
		{@render group(active, t.list, false)}
		{#if disabled.length > 0}
			<h2 class="group-title">{t.disabledGroup}</h2>
			{@render group(disabled, t.disabledGroup, true)}
		{/if}
	{/if}
{/if}

{#if adding}
	<UserAddDialog {roles} oncreated={put} bind:open={() => adding, (v) => (adding = v)} />
{/if}
{#if editing}
	<UserEditDialog
		user={editing}
		{roles}
		onchanged={put}
		bind:open={() => true, (v) => v || (editingId = null)}
	/>
{/if}

<style>
	.title {
		margin: 0;
		font-size: var(--text-xl);
		font-weight: 600;
	}
	.group-title {
		margin: 0;
		font-size: var(--text-md);
		font-weight: 600;
		color: var(--text-3);
	}
	.list {
		margin: 0;
		padding: 0;
		list-style: none;
	}
	li + li {
		border-top: 1px solid var(--border);
	}
	.row {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
		box-sizing: border-box;
		width: 100%;
		min-height: var(--control-lg);
		padding: var(--space-2) var(--space-3);
		border: 0;
		background: none;
		color: var(--text);
		font: inherit;
		text-align: left;
		cursor: pointer;
	}
	.row:hover {
		background: var(--surface-3);
	}
	.row.muted {
		color: var(--text-3);
	}
	.who {
		display: flex;
		flex-direction: column;
		min-width: 0;
		overflow-wrap: anywhere;
	}
	.name {
		font-weight: 500;
	}
	.sub {
		font-size: var(--text-sm);
		color: var(--text-3);
	}
	.tags {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
		font-size: var(--text-sm);
		color: var(--text-2);
	}
</style>
