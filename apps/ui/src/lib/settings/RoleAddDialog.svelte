<!-- «Добавить роль»: a name and the rights ticked at once. The server's refusals stay at their fields and go
     to a notice too. -->
<script lang="ts">
	import type { Permission } from '$api/Permission';
	import type { Role } from '$api/Role';
	import { createRole } from '#lib/api/users.js';
	import { submitShortcut } from '#lib/design/hotkeys.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import TextInput from '#lib/ui/TextInput.svelte';
	import { toast } from '#lib/ui/toast/toast.svelte.js';
	import PermissionChecks from './PermissionChecks.svelte';
	import { PERMISSION_GROUPS } from './permissions.js';
	import { userFailure, userFieldErrors } from './users.js';

	let { open = $bindable(false), oncreated }: { open: boolean; oncreated: (r: Role) => void } =
		$props();

	const t = ru.roles;
	const ORDER = PERMISSION_GROUPS.flatMap((g) => g.permissions);
	let name = $state('');
	let chosen = $state<Permission[]>([]);
	let errors = $state<Record<string, string>>({});
	let busy = $state(false);

	$effect(() => {
		if (open) return submitShortcut(() => void submit());
	});

	function toggle(p: Permission, on: boolean) {
		chosen = on ? [...chosen.filter((x) => x !== p), p] : chosen.filter((x) => x !== p);
	}

	async function submit() {
		if (busy) return;
		if (!name.trim()) {
			errors = { name: t.nameRequired };
			return;
		}
		errors = {};
		busy = true;
		try {
			const role = await createRole({
				name: name.trim(),
				permissions: ORDER.filter((p) => chosen.includes(p))
			});
			toast.show({ message: t.created });
			oncreated(role);
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
		<TextInput label={t.name} autocomplete="off" error={errors.name} bind:value={name} />
		<PermissionChecks value={chosen} disabled={busy} ontoggle={toggle} />
		{#if errors.permissions}<p class="error" role="alert">{errors.permissions}</p>{/if}
		<div class="actions">
			<Button onclick={() => (open = false)} disabled={busy}>{ru.common.cancel}</Button>
			<Button type="submit" variant="primary" {busy}>{t.create}</Button>
		</div>
	</form>
</Dialog>

<style>
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
	.error {
		margin: 0;
		font-size: var(--text-sm);
		color: var(--danger-text);
	}
</style>
