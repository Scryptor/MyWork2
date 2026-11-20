<!-- «Добавить пользователя»: name, sign-in (login and password only with it), role, «Мастер». The server's
     mistakes (a taken login, a short password, no right) stay at their fields and go to a notice too. -->
<script lang="ts">
	import type { Role } from '$api/Role';
	import type { User } from '$api/User';
	import { createUser } from '#lib/api/users.js';
	import { submitShortcut } from '#lib/design/hotkeys.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import HelpHint from '#lib/ui/HelpHint.svelte';
	import Select from '#lib/ui/Select.svelte';
	import Switch from '#lib/ui/Switch.svelte';
	import TextInput from '#lib/ui/TextInput.svelte';
	import { toast } from '#lib/ui/toast/toast.svelte.js';
	import { userFailure, userFieldErrors } from './users.js';

	let {
		open = $bindable(false),
		roles,
		oncreated
	}: { open: boolean; roles: readonly Role[]; oncreated: (u: User) => void } = $props();

	const t = ru.users;
	let name = $state('');
	let canSignIn = $state(false);
	let login = $state('');
	let password = $state('');
	// svelte-ignore state_referenced_locally
	let roleId = $state<number | null>(roles.find((r) => r.name === 'Мастер')?.id ?? null);
	let isMaster = $state(false);
	let errors = $state<Record<string, string>>({});
	let busy = $state(false);

	$effect(() => {
		if (open) return submitShortcut(() => void submit());
	});

	async function submit() {
		if (busy) return;
		const local: Record<string, string> = {};
		if (canSignIn) {
			if (!login.trim()) local.login = t.loginRequired;
			if (!password) local.password = t.passwordRequired;
		} else if (!name.trim()) local.display_name = t.nameOrLogin;
		if (roleId === null) local.role_id = t.chooseRole;
		errors = local;
		if (Object.keys(local).length > 0 || roleId === null) return;
		busy = true;
		try {
			const user = await createUser({
				display_name: name.trim(),
				login: canSignIn ? login.trim() : '',
				password: canSignIn ? password : '',
				role_id: roleId,
				can_sign_in: canSignIn,
				is_master: isMaster
			});
			toast.show({ message: t.created });
			oncreated(user);
			open = false;
		} catch (e) {
			errors = userFieldErrors(e);
			toast.show({ message: t.failed(userFailure(e)), tone: 'danger' });
		} finally {
			busy = false;
		}
	}
</script>

<Dialog bind:open title={t.addTitle} dismissible={!busy}>
	<form
		class="form"
		onsubmit={(e) => {
			e.preventDefault();
			void submit();
		}}
	>
		<TextInput label={t.name} autocomplete="off" error={errors.display_name} bind:value={name} />
		<Switch label={t.canSignIn} help={ru.help.users.canSignIn} bind:checked={canSignIn} />
		{#if canSignIn}
			<TextInput
				label={t.login}
				autocomplete="off"
				required
				error={errors.login}
				bind:value={login}
			/>
			<TextInput
				label={t.password}
				type="password"
				revealable
				autocomplete="new-password"
				required
				error={errors.password}
				bind:value={password}
			/>
		{/if}
		<div class="role-head">
			<span class="role-label">{t.role}</span>
			<HelpHint title={t.role} text={ru.help.users.role} />
		</div>
		<Select
			ariaLabel={t.role}
			required
			error={errors.role_id}
			items={roles.map((r) => ({ value: r.id, label: r.name }))}
			bind:value={roleId}
		/>
		<Switch label={t.isMaster} help={ru.help.users.master} bind:checked={isMaster} />
		<div class="actions">
			<Button onclick={() => (open = false)} disabled={busy}>{ru.common.cancel}</Button>
			<Button type="submit" variant="primary" {busy}>{t.create}</Button>
		</div>
	</form>
</Dialog>

<style>
	.role-head {
		display: flex;
		align-items: center;
		gap: var(--space-1);
	}
	.role-label {
		font-size: var(--text-sm);
		font-weight: 500;
		color: var(--text-2);
	}
	.form {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}
	.actions {
		display: flex;
		justify-content: flex-end;
		gap: var(--space-2);
	}
</style>
