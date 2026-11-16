<!-- «Сменить пароль» from the user menu. The server closes every session of the user after the
     change, so `onchanged` sends the user to sign in again. -->
<script lang="ts">
	import { untrack } from 'svelte';
	import { changePassword } from '#lib/api/auth.js';
	import { submitShortcut } from '#lib/design/hotkeys.svelte.js';
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import TextInput from '#lib/ui/TextInput.svelte';
	import { checkPasswordForm, passwordErrorsFromServer, type PasswordErrors } from './password.js';

	let { open = $bindable(false), onchanged }: { open: boolean; onchanged: () => void } = $props();

	const formId = $props.id();
	let current = $state('');
	let next = $state('');
	let repeat = $state('');
	let errors = $state<PasswordErrors>({});
	let busy = $state(false);

	// Every opening starts empty.
	$effect(() => {
		if (!open) return;
		untrack(() => {
			current = next = repeat = '';
			errors = {};
			busy = false;
		});
	});

	$effect(() => {
		if (open) return submitShortcut(() => void submit());
	});

	async function submit() {
		if (busy) return;
		errors = checkPasswordForm({ current, next, repeat });
		if (Object.keys(errors).length > 0) return;
		busy = true;
		try {
			await changePassword({ current_password: current, new_password: next });
			open = false;
			onchanged();
		} catch (e) {
			errors = passwordErrorsFromServer(e);
		} finally {
			busy = false;
		}
	}
</script>

<Dialog bind:open title={ru.password.title} size="sm">
	<form
		id={formId}
		class="form"
		novalidate
		onsubmit={(e) => {
			e.preventDefault();
			void submit();
		}}
	>
		<TextInput
			label={ru.password.current}
			type="password"
			autocomplete="current-password"
			bind:value={current}
			error={errors.current}
			required
		/>
		<TextInput
			label={ru.password.next}
			type="password"
			autocomplete="new-password"
			bind:value={next}
			error={errors.next}
			required
		/>
		<TextInput
			label={ru.password.repeat}
			type="password"
			autocomplete="new-password"
			bind:value={repeat}
			error={errors.repeat}
			required
		/>
		{#if errors.form}<p class="error" role="alert">{errors.form}</p>{/if}
	</form>
	{#snippet footer()}
		<Button onclick={() => (open = false)}>{ru.common.cancel}</Button>
		<Button variant="primary" type="submit" form={formId} {busy} shortcut="Mod+Enter"
			>{ru.password.submit}</Button
		>
	{/snippet}
</Dialog>

<style>
	.form {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}
	.error {
		margin: 0;
		color: var(--danger-text);
		font-size: var(--text-sm);
	}
</style>
