/**
 * Terminal-failure cause surfacing (#502, Option 1 — maintainer-adopted
 * 2026-10-06).
 *
 * The fast-fail work (#498, #500) journals a genuine diagnostic to
 * `operations` on terminal failure: `error_code`
 * (UNSUPPORTED_BY_AGENT / DISPATCH_FAILED / AGENT_REJECTED /
 * MIGRATION_FAILED, …) plus the agents' verbatim refusal text in
 * `error_message`. The BFF now passes those two columns through on
 * every operation-carrying response (task list/get/stream, the
 * per-resource recent-task listings, the overview panel); this helper
 * is the UI's single read of them.
 *
 * The discipline mirrors the BFF pass-through exactly: the fields are
 * surfaced only when the row actually records them — never fabricated,
 * never defaulted to a generic placeholder. A terminal op with no
 * recorded cause renders exactly as it did before this change; a
 * running or succeeded op has nothing recorded and surfaces nothing.
 */

export interface TaskFailureFields {
	error_code?: string | null;
	error_message?: string | null;
}

export interface TaskFailureCause {
	/** The journaled code, e.g. `UNSUPPORTED_BY_AGENT` — null when the row records a message but no code. */
	code: string | null;
	/** The journaled diagnostic — the agents' refusal text, rendered as-is. */
	message: string | null;
}

/**
 * The recorded terminal-failure cause of an operation row, or `null`
 * when the row records none (running, succeeded, cancelled, or a
 * terminal failure whose writer left no diagnostic).
 *
 * The status is deliberately NOT consulted: the BFF passes the two
 * columns through verbatim, and the writers only populate them on
 * failure paths — the fields' presence IS the signal. A blank string
 * counts as absent (defensive against a writer that journals `''`).
 */
export function getTaskFailureCause(
	task: TaskFailureFields | null | undefined
): TaskFailureCause | null {
	if (!task) return null;

	const code =
		typeof task.error_code === 'string' && task.error_code.trim().length > 0
			? task.error_code
			: null;
	const message =
		typeof task.error_message === 'string' && task.error_message.trim().length > 0
			? task.error_message
			: null;

	if (code === null && message === null) return null;

	return { code, message };
}
