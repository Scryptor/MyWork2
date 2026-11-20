<!-- Sign-in (spec 6.11): only when the program has users; afterwards back where the user was. -->
<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { login } from '#lib/api/auth.js';
	import { ApiError } from '#lib/api/client.js';
	import { adoptStored, setSessionFoundHandler, setToken } from '#lib/api/session.js';
	import { safeNext } from '#lib/app/login.js';
	import { takeLoginNotices } from '#lib/app/login-notice.js';
	import { resetSession } from '#lib/app/session-reset.js';
	import '#lib/app/page-card.css';
	import { ru } from '#lib/i18n/ru.js';
	import Button from '#lib/ui/Button.svelte';
	import Checkbox from '#lib/ui/Checkbox.svelte';
	import TextInput from '#lib/ui/TextInput.svelte';
	import { toast } from '#lib/ui/toast/toast.svelte.js';

	// Nobody is signed in here: whatever the previous user left in memory (also after an expired session)
	// is gone before the next one signs in. Signing in in another tab signs this one in too: it goes on
	// where it was going.
	onMount(() => {
		resetSession();
		// After the reset: it clears the toasts.
		for (const n of takeLoginNotices()) toast.show(n);
		setSessionFoundHandler(() => {
			if (adoptStored() !== null) void goToNext();
		});
		return () => setSessionFoundHandler(() => {});
	});

	function goToNext() {
		const next = safeNext(page.url.searchParams.get('next'), page.url.origin);
		// safeNext keeps only a path of this program.
		// eslint-disable-next-line svelte/no-navigation-without-resolve
		return goto(next, { replaceState: true });
	}

	let name = $state('');
	let password = $state('');
	let remember = $state(true);
	let busy = $state(false);
	let nameError = $state<string | undefined>();
	let passwordError = $state<string | undefined>();

	function failure(e: unknown): string {
		if (!(e instanceof ApiError)) return ru.api.failed;
		if (e.code === 'invalid_credentials') return ru.auth.invalidCredentials;
		if (e.code === 'too_many_attempts') {
			return ru.auth.tooManyAttempts(e.body?.retry_after_secs ?? 60);
		}
		if (e.code === 'network') return ru.api.offline;
		return e.message;
	}

	async function submit(e: SubmitEvent) {
		e.preventDefault();
		if (busy) return;
		nameError = name.trim() ? undefined : ru.auth.loginRequired;
		passwordError = password ? undefined : ru.auth.passwordRequired;
		if (nameError || passwordError) return;
		busy = true;
		try {
			const session = await login({ login: name, password, device_label: '' });
			setToken(session.token, remember, session.user.id);
			await goToNext();
		} catch (err) {
			passwordError = failure(err);
		} finally {
			busy = false;
		}
	}
</script>

<svelte:head><title>{ru.auth.title}</title></svelte:head>

<main class="mw-page-center" tabindex="-1">
	<form class="mw-page-card" novalidate onsubmit={submit}>
		<h1>{ru.auth.title}</h1>
		<TextInput
			label={ru.auth.login}
			bind:value={name}
			error={nameError}
			autocomplete="username"
			autofocus
		/>
		<TextInput
			label={ru.auth.password}
			type="password"
			bind:value={password}
			error={passwordError}
			autocomplete="current-password"
		/>
		<Checkbox label={ru.auth.remember} bind:checked={remember} />
		<Button type="submit" variant="primary" {busy}>{ru.auth.submit}</Button>
	</form>
</main>
