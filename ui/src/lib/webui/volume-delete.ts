// Shared payload + confirm-copy helpers for the volume-delete
// detail-page surface (#522 DP12, PR 4 of the adopted decomposition).
// Mirrors the volume-create.ts discipline: the server stays
// authoritative; these client-side helpers exist so the wire body and
// the confirm copy are pinned by tests instead of living only in the
// page component (the repo's vitest tier renders leaf components, not
// route pages — the #513 PR 4 coverage story).
import type { DeleteVolumeRequest } from '#lib/bff/types.ts';

// Builds the wire payload: exactly the one contract key. DP5's
// no-force stance is structural — a `force` flag exists nowhere in
// this module, so the payload cannot carry it (the BFF's attached
// guard is the safety net and its 400 names the detach path); DP6's
// kind gate has no override either, same story.
export function buildDeleteVolumePayload(volumeId: string): DeleteVolumeRequest {
	return { volume_id: volumeId.trim() };
}

// DP12's confirm copy: names the volume, its size, and the
// irreversibility — the design's words ("the backing store on the
// node is destroyed; this cannot be undone"). Rendered inside the
// detail page's existing confirm-group pattern (never
// `window.confirm` — the page already has the better primitive).
export function buildDeleteConfirmText(name: string, size: string): string {
	const trimmedName = name.trim();
	const subject = trimmedName === '' ? 'this volume' : `"${trimmedName}"`;
	const trimmedSize = size.trim();
	const sized = trimmedSize === '' ? '' : ` (${trimmedSize})`;
	return `Delete ${subject}${sized}? The backing store on the node is destroyed; this cannot be undone.`;
}
