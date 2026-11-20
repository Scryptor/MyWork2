<!-- «Роли»: a role is a set of rights; the list shows how many users each has. A row opens the role: the
     name, the rights as checkboxes (built-in roles too), delete. Right: «users_manage». -->
<script lang="ts">
	import Plus from '@lucide/svelte/icons/plus';
	import { onMount } from 'svelte';
	import type { Role } from '$api/Role';
	import type { User } from '$api/User';
	import { errorMessage } from '#lib/api/client.js';
	import { listRoles, listUsers } from '#lib/api/users.js';
	import { app } from '#lib/app/context.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import RoleAddDialog from '#lib/settings/RoleAddDialog.svelte';
	import RoleEditDialog from '#lib/settings/RoleEditDialog.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import Card from '#lib/ui/Card.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import Skeleton from '#lib/ui/Skeleton.svelte';

	const t = ru.roles;
	const allowed = app.can('users_manage');

	let roles = $state.raw<Role[]>([]);
	let users = $state.raw<Pick<User, 'role_id'>[]>([]);
	let loading = $state(allowed);
	let failure = $state<string | null>(null);
	let adding = $state(false);
	let editingId = $state<number | null>(null);

	const sorted = $derived(
		[...roles].sort((a, b) => a.name.localeCompare(b.name, 'ru', { sensitivity: 'base' }))
	);
	const editing = $derived(roles.find((r) => r.id === editingId));
	const count = (id: number) => users.filter((u) => u.role_id === id).length;

	function put(r: Role) {
		roles = roles.some((x) => x.id === r.id)
			? roles.map((x) => (x.id === r.id ? r : x))
			: [...roles, r];
	}

	onMount(async () => {
		if (!allowed) return;
		try {
			[roles, users] = await Promise.all([listRoles(), listUsers()]);
		} catch (e) {
			failure = errorMessage(e);
		} finally {
			loading = false;
		}
	});
</script>

<h1 class="title">{ru.settings.roles}</h1>

{#if !allowed}
	<Callout tone="info">{t.rights}</Callout>
{:else}
	<div><Button icon={Plus} variant="primary" onclick={() => (adding = true)}>{t.add}</Button></div>
	{#if failure}
		<Callout tone="danger">{t.loadFailed}: {failure}</Callout>
	{:else if loading}
		<Skeleton height="2rem" />
	{:else if roles.length === 0}
		<EmptyState title={t.empty} />
	{:else}
		<Card>
			<ul class="list" aria-label={t.list}>
				{#each sorted as r (r.id)}
					<li>
						<button
							type="button"
							class="row"
							aria-label={t.edit(r.name)}
							onclick={() => (editingId = r.id)}
						>
							<span class="name">{r.name}</span>
							<span class="sub">{t.usersCount(count(r.id))}</span>
						</button>
					</li>
				{/each}
			</ul>
		</Card>
	{/if}
{/if}

{#if adding}
	<RoleAddDialog oncreated={put} bind:open={() => adding, (v) => (adding = v)} />
{/if}
{#if editing}
	<RoleEditDialog
		role={editing}
		members={count(editing.id)}
		onchanged={put}
		ondeleted={(id) => (roles = roles.filter((r) => r.id !== id))}
		bind:open={() => true, (v) => v || (editingId = null)}
	/>
{/if}

<style>
	.title {
		margin: 0;
		font-size: var(--text-xl);
		font-weight: 600;
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
	.name {
		min-width: 0;
		overflow-wrap: anywhere;
		font-weight: 500;
	}
	.sub {
		font-size: var(--text-sm);
		color: var(--text-3);
	}
</style>
