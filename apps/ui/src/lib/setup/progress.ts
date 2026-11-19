// The progress of a transfer from MyWork 2 (spec 6.9): the server runs it in the background, the screen asks
// for its state once a second. The state is the server's, so a reloaded page (or another tab) sees the same
// progress. A 401 is not an expired session here: the base the import wrote has users of its own (or the
// replacement closed every session), and one of them signs in to see the report.
import type { ImportJob } from '$api/ImportJob';
import type { ImportStage } from '$api/ImportStage';
import { ApiError, errorMessage } from '#lib/api/client.js';
import { ru } from '#lib/i18n/ru.js';

/** How often the progress is asked for. */
export const POLL_MS = 1000;

/** What a state of the job means to the screen. */
export type JobState =
	| { kind: 'running' }
	| { kind: 'done'; importId: number }
	| { kind: 'failed'; message: string; importId: number | null };

/** Done only with the number of the import (the server sets both at once); an error is a failure. */
export function jobState(job: ImportJob): JobState {
	if (job.error !== null) return { kind: 'failed', message: job.error, importId: job.import_id };
	if (job.stage === 'done' && job.import_id !== null)
		return { kind: 'done', importId: job.import_id };
	return { kind: 'running' };
}

/** What following the import has come to. */
export type Followed =
	/** Under way; `offline`: the last question got no answer (the last known state is shown). */
	| { kind: 'running'; job: ImportJob; offline: boolean }
	| { kind: 'done'; importId: number }
	| { kind: 'failed'; message: string; importId: number | null }
	/** The base is written and asks to sign in now. */
	| { kind: 'signin' }
	| { kind: 'forbidden'; message: string }
	/** The server knows of no import: it was restarted (the past imports say how it ended). */
	| { kind: 'gone' };

/**
 * Asks `current` every `interval` (the next question after the answer to the previous one) and tells
 * `onstate` what it hears, until the import ends or `stop` is called. The first question comes after the
 * interval too: whoever starts following has just had a state of the job (the start, the screen's own
 * question).
 */
export function followImport(
	current: () => Promise<ImportJob | null>,
	onstate: (state: Followed) => void,
	opts: { interval?: number } = {}
): () => void {
	const interval = opts.interval ?? POLL_MS;
	let stopped = false;
	let timer: ReturnType<typeof setTimeout> | undefined;
	let last: ImportJob | null = null;

	const report = (state: Followed) => {
		if (!stopped) onstate(state);
	};
	const next = () => {
		if (!stopped) timer = setTimeout(() => void ask(), interval);
	};

	async function ask() {
		let job: ImportJob | null;
		try {
			job = await current();
		} catch (e) {
			if (stopped) return;
			if (e instanceof ApiError && e.status === 401) return report({ kind: 'signin' });
			if (e instanceof ApiError && e.status === 403)
				return report({ kind: 'forbidden', message: e.message });
			if (e instanceof ApiError && e.code === 'network') {
				if (last !== null) report({ kind: 'running', job: last, offline: true });
				return next();
			}
			return report({ kind: 'failed', message: errorMessage(e), importId: null });
		}
		if (stopped) return;
		if (job === null) return report({ kind: 'gone' });
		last = job;
		const state = jobState(job);
		if (state.kind !== 'running') return report(state);
		report({ kind: 'running', job, offline: false });
		next();
	}

	next();
	return () => {
		stopped = true;
		clearTimeout(timer);
	};
}

/** The stages of the import in their order (`done` is the end, not a stage of its own). */
export const STAGES = [
	'reading',
	'converting',
	'copying',
	'writing',
	'placing'
] as const satisfies readonly ImportStage[];

export type StepState = 'done' | 'current' | 'failed' | 'pending';

export interface Step {
	stage: (typeof STAGES)[number];
	label: string;
	state: StepState;
	percent: number;
}

/**
 * The list of stages as the screen shows it. `written`: the base is known to be written (the 401 after
 * it), so everything up to «Запись» is done, whatever the last answer said. `failed`: the import stopped at
 * its stage.
 */
export function stageSteps(
	job: Pick<ImportJob, 'stage' | 'percent'>,
	opts: { written?: boolean; failed?: boolean } = {}
): Step[] {
	const at = job.stage === 'done' ? STAGES.length : STAGES.indexOf(job.stage);
	const writtenUpTo = opts.written ? STAGES.indexOf('writing') : -1;
	return STAGES.map((stage, i) => {
		const label = ru.legacy.stages[stage];
		if (i < at || i <= writtenUpTo) return { stage, label, state: 'done', percent: 100 };
		if (i === at && !opts.written)
			return { stage, label, state: opts.failed ? 'failed' : 'current', percent: job.percent };
		return { stage, label, state: 'pending', percent: 0 };
	});
}
