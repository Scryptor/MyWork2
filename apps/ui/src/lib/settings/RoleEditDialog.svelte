<!-- One role: the name is saved as typed (the shared editor, one queue of writes per record, a 409 merged);
     each right is saved at once on the newest version. A refusal of the server (the last «Пользователи и
     права», the last signing-in administrator, a taken name) is shown and the box shows what the server has.
     A role with users is not deleted: the button says why. -->
<script lang="ts">
	import { onDestroy } from 'svelte';
	import type { Permission } from '$api/Permission';
	import type { Role } from '$api/Role';
	import type { RolePatch } from '$api/RolePatch';
	import { ApiError } from '#lib/api/client.js';
	import { deleteRole, listRoles, updateRole } from '#lib/api/users.js';
	import { createEntityEditor } from '#lib/app/entity-editor.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';
	import ConfirmDialog from '#lib/ui/ConfirmDialog.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import TextInput from '#lib/ui/TextInput.svelte';
	import { toast } from '#lib/ui/toast/toast.svelte.js';
	import PermissionChecks from './PermissionChecks.svelte';
	import { PERMISSION_GROUPS } from './permissions.js';
	import { userFailure, userFieldErrors } from './users.js';

	let {
		open = $bindable(false),
		role,
		members,
		onchanged,
		ondeleted
	}: {
		open: boolean;
		role: Role;
		/** How many users have this role. */
		members: number;
		onchanged: (r: Role) => void;
		ondeleted: (id: number) => void;
	} = $props();

	const t = ru.roles;
	const ORDER = PERMISSION_GROUPS.flatMap((g) => g.permissions);
	// Opened for one role: a dialog shows one record for its whole life.
	// svelte-ignore state_referenced_locally
	const initial = role;
	const load = async () => {
		const found = (await listRoles()).find((r) => r.id === initial.id);
		if (!found) throw new ApiError(404, 'not_found', t.loadFailed, null);
		return found;
	};
	const editor = createEntityEditor<Role, 'name'>({
		initial,
		fields: ['name'],
		load,
		save: (patch, version) => updateRole(initial.id, version, patch),
		emit: (r) => onchanged(r)
	});
	const rec = $derived(editor.record);
	const shownName = $derived(editor.autosave.value('name', rec.name));

	let errors = $state<Record<string, string>>({});
	let busy = $state(false);
	let resync = $state(0);
	let confirming = $state(false);

	async function act(patch: RolePatch) {
		if (busy) return;
		busy = true;
		errors = {};
		try {
			await editor.run((version) => updateRole(rec.id, version, patch));
		} catch (e) {
			resync += 1;
			errors = userFieldErrors(e);
			// A refusal without a field (409) is shown at the rights.
			if (Object.keys(errors).length === 0) errors = { permissions: userFailure(e) };
			toast.show({ message: t.failed(userFailure(e)), tone: 'danger' });
		} finally {
			busy = false;
		}
	}

	function toggle(p: Permission, on: boolean) {
		const next = on ? [...rec.permissions, p] : rec.permissions.filter((x) => x !== p);
		void act({ permissions: ORDER.filter((x) => next.includes(x)) });
	}

	async function remove() {
		errors = {};
		try {
			await editor.flush();
			await deleteRole(rec.id, editor.record.version);
			toast.show({ message: t.deleted });
			ondeleted(rec.id);
			open = false;
		} catch (e) {
			errors = { delete: userFailure(e) };
			toast.show({ message: t.failed(userFailure(e)), tone: 'danger' });
		}
	}

	onDestroy(() => {
		void editor
			.flush()
			.then((ok) => {
				if (!ok)
					toast.show({
						message: t.failed(
							`${editor.current.name}: ${editor.autosave.error?.message ?? ru.api.failed}`
						),
						tone: 'danger'
					});
			})
			.finally(() => editor.dispose());
	});
</script>

<Dialog bind:open title={t.editTitle} description={t.usersCount(members)}>
	<div class="form">
		<TextInput
			label={t.name}
			autocomplete="off"
			error={errors.name ??
				(editor.autosave.error?.code === 'validation' ? editor.autosave.error.message : undefined)}
			bind:value={() => shownName, (v) => editor.autosave.edit('name', v)}
		/>
		<h3 class="rights">{t.permissions}</h3>
		<PermissionChecks value={rec.permissions} disabled={busy} {resync} ontoggle={toggle} />
		{#if errors.permissions}<p class="error" role="alert">{errors.permissions}</p>{/if}
		<div class="danger">
			<div>
				<Button variant="danger" disabled={members > 0 || busy} onclick={() => (confirming = true)}>
					{t.delete}
				</Button>
			</div>
			{#if members > 0}<p class="note">{t.cannotDelete(members)}</p>{/if}
			{#if errors.delete}<p class="error" role="alert">{errors.delete}</p>{/if}
		</div>
	</div>
</Dialog>

<ConfirmDialog
	bind:open={confirming}
	title={t.deleteTitle(rec.name)}
	description={t.deleteText}
	confirmLabel={t.confirmDelete}
	onconfirm={remove}
/>

<style>
	.form {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}
	.rights {
		margin: 0;
		font-size: var(--text-md);
		font-weight: 600;
	}
	.danger {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		padding-top: var(--space-2);
		border-top: 1px solid var(--border);
	}
	.note {
		margin: 0;
		font-size: var(--text-sm);
		color: var(--text-3);
	}
	.error {
		margin: 0;
		font-size: var(--text-sm);
		color: var(--danger-text);
	}
</style>
