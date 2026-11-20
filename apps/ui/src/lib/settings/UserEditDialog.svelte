<!-- One user: the name is saved as typed (the shared editor, one queue of writes per record, a 409 merged);
     the role, «Мастер», sign-in on and off, disabling and the password are actions at once, each on the
     newest version. A refusal of the server (the last administrator, a taken login) stays at its field and
     goes to a notice; the control then shows what the server has. -->
<script lang="ts">
	import { onDestroy } from 'svelte';
	import type { Role } from '$api/Role';
	import type { User } from '$api/User';
	import type { UserPatch } from '$api/UserPatch';
	import { ApiError } from '#lib/api/client.js';
	import { listUsers, setUserPassword, updateUser } from '#lib/api/users.js';
	import { createEntityEditor } from '#lib/app/entity-editor.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import HelpHint from '#lib/ui/HelpHint.svelte';
	import Select from '#lib/ui/Select.svelte';
	import Switch from '#lib/ui/Switch.svelte';
	import TextInput from '#lib/ui/TextInput.svelte';
	import { toast } from '#lib/ui/toast/toast.svelte.js';
	import { userFailure, userFieldErrors, userTitle } from './users.js';

	let {
		open = $bindable(false),
		user,
		roles,
		onchanged
	}: {
		open: boolean;
		user: User;
		roles: readonly Role[];
		onchanged: (u: User) => void;
	} = $props();

	const t = ru.users;
	// Opened for one user: a dialog shows one record for its whole life.
	// svelte-ignore state_referenced_locally
	const initial = user;
	const load = async () => {
		const found = (await listUsers()).find((u) => u.id === initial.id);
		if (!found) throw new ApiError(404, 'not_found', t.loadFailed, null);
		return found;
	};
	const editor = createEntityEditor<User, 'display_name'>({
		initial,
		fields: ['display_name'],
		load,
		save: (patch, version) => updateUser(initial.id, version, patch),
		emit: (u) => onchanged(u)
	});
	const rec = $derived(editor.record);
	const shownName = $derived(editor.autosave.value('display_name', rec.display_name));

	let errors = $state<Record<string, string>>({});
	let busy = $state(false);
	let enabling = $state(false);
	let login = $state(initial.login);
	let password = $state('');
	let changingPassword = $state(false);
	let newPassword = $state('');

	async function act(patch: UserPatch, onerror?: () => void): Promise<boolean> {
		if (busy) return false;
		busy = true;
		errors = {};
		try {
			await editor.run((version) => updateUser(rec.id, version, patch));
			return true;
		} catch (e) {
			errors = userFieldErrors(e);
			toast.show({ message: t.failed(userFailure(e)), tone: 'danger' });
			onerror?.();
			return false;
		} finally {
			busy = false;
		}
	}

	async function turnOn() {
		const local: Record<string, string> = {};
		if (!login.trim()) local.login = t.loginRequired;
		if (!rec.has_password && !password) local.password = t.passwordRequired;
		errors = local;
		if (Object.keys(local).length > 0) return;
		const patch: UserPatch = { can_sign_in: true, login: login.trim() };
		if (password) patch.password = password;
		if (await act(patch)) {
			enabling = false;
			password = '';
		}
	}

	/** The switch of sign-in: off is at once; on needs the login and a password unless it all is there. */
	function toggleSignIn(on: boolean) {
		errors = {};
		if (!on) {
			enabling = false;
			if (rec.can_sign_in) void act({ can_sign_in: false });
		} else if (rec.login && rec.has_password) void act({ can_sign_in: true });
		else enabling = true;
	}

	async function savePassword() {
		if (!newPassword) {
			errors = { password: t.passwordRequired };
			return;
		}
		busy = true;
		errors = {};
		try {
			await setUserPassword(rec.id, newPassword);
			toast.show({ message: t.passwordChanged });
			newPassword = '';
			changingPassword = false;
			const fresh = await load();
			editor.receive(fresh);
			onchanged(fresh);
		} catch (e) {
			errors = userFieldErrors(e);
			toast.show({ message: t.failed(userFailure(e)), tone: 'danger' });
		} finally {
			busy = false;
		}
	}

	onDestroy(() => {
		void editor
			.flush()
			.then((ok) => {
				if (!ok)
					toast.show({
						message: t.failed(
							`${userTitle(editor.current)}: ${editor.autosave.error?.message ?? ru.api.failed}`
						),
						tone: 'danger'
					});
			})
			.finally(() => editor.dispose());
	});
</script>

<Dialog bind:open title={t.editTitle} description={rec.login ? rec.login : t.noSignIn}>
	<div class="form">
		{#if !rec.active}<Callout tone="warning">{t.disabledNote}</Callout>{/if}
		<TextInput
			label={t.name}
			autocomplete="off"
			error={errors.display_name ??
				(editor.autosave.error?.code === 'validation' ? editor.autosave.error.message : undefined)}
			bind:value={() => shownName, (v) => editor.autosave.edit('display_name', v)}
		/>
		<div class="role-head">
			<span class="role-label">{t.role}</span>
			<HelpHint title={t.role} text={ru.help.users.role} />
		</div>
		<Select
			ariaLabel={t.role}
			error={errors.role_id}
			disabled={busy}
			items={roles.map((r) => ({ value: r.id, label: r.name }))}
			bind:value={
				() => rec.role_id, (v) => v !== null && v !== rec.role_id && void act({ role_id: v })
			}
		/>
		<Switch
			label={t.isMaster}
			help={ru.help.users.master}
			disabled={busy}
			bind:checked={() => rec.is_master, (v) => void act({ is_master: v })}
		/>
		<Switch
			label={t.canSignIn}
			help={ru.help.users.canSignIn}
			disabled={busy}
			bind:checked={() => rec.can_sign_in || enabling, toggleSignIn}
		/>
		{#if errors.can_sign_in}<p class="error" role="alert">{errors.can_sign_in}</p>{/if}
		{#if rec.can_sign_in && !rec.has_password}
			<Callout tone="warning">{t.noPasswordHint}</Callout>
		{/if}
		{#if enabling}
			<div class="group">
				<TextInput
					label={t.login}
					autocomplete="off"
					required
					error={errors.login}
					bind:value={login}
				/>
				{#if !rec.has_password}
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
				<div>
					<Button variant="primary" {busy} onclick={() => void turnOn()}>{t.enableSignIn}</Button>
				</div>
			</div>
		{/if}
		{#if rec.login}
			{#if changingPassword}
				<div class="group">
					<TextInput
						label={t.newPassword}
						type="password"
						revealable
						autocomplete="new-password"
						error={errors.password}
						bind:value={newPassword}
					/>
					<div class="actions">
						<Button variant="primary" {busy} onclick={() => void savePassword()}>
							{t.savePassword}
						</Button>
						<Button disabled={busy} onclick={() => ((changingPassword = false), (errors = {}))}>
							{ru.common.cancel}
						</Button>
					</div>
				</div>
			{:else}
				<div><Button onclick={() => (changingPassword = true)}>{t.changePassword}</Button></div>
			{/if}
		{/if}
		<div class="danger">
			<div class="actions">
				<Button
					variant={rec.active ? 'danger' : 'secondary'}
					{busy}
					onclick={() => void act({ active: !rec.active })}
				>
					{rec.active ? t.disable : t.enable}
				</Button>
				<HelpHint title={t.disable} text={ru.help.users.disable} />
			</div>
			{#if errors.active}<p class="error" role="alert">{errors.active}</p>{/if}
		</div>
	</div>
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
	.group {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		padding: var(--space-3);
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
	}
	.actions {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
	}
	.danger {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		padding-top: var(--space-2);
		border-top: 1px solid var(--border);
	}
	.error {
		margin: 0;
		font-size: var(--text-sm);
		color: var(--danger-text);
	}
</style>
