import type { PageLoad } from './$types';
import { getStoredToken } from '#lib/api/client.ts';
import {
	listDeliveries,
	listIncidents,
	listRules,
	type AlertDelivery,
	type AlertRule,
	type Incident
} from '#lib/bff/alerting.ts';

export type AlertsTab = 'incidents' | 'rules';

export type AlertsPageModel = {
	tab: AlertsTab;
	incidents: Incident[];
	incidentTotal: number;
	/** True when the incident listing failed (e.g. monitoring store down). */
	incidentsError: boolean;
	rules: AlertRule[];
	ruleTotal: number;
	rulesError: boolean;
	deliveries: AlertDelivery[];
};

export const load: PageLoad = async ({ url }): Promise<AlertsPageModel> => {
	const token = getStoredToken() ?? undefined;
	const tab: AlertsTab = url.searchParams.get('tab') === 'rules' ? 'rules' : 'incidents';

	// The three listings fail independently: rules live in the
	// operational database, incidents depend on the monitoring store
	// (a 503 there must not blank the rule list), and deliveries are
	// audit-only. Catching per fetch keeps each surface honest.
	const [incidentResult, ruleResult, deliveryResult] = await Promise.all([
		listIncidents({ include_resolved: false, limit: 100 }, token).catch(() => null),
		listRules({ limit: 100 }, token).catch(() => null),
		listDeliveries({ limit: 10 }, token).catch(() => null)
	]);

	return {
		tab,
		incidents: incidentResult?.incidents ?? [],
		incidentTotal: incidentResult?.total ?? 0,
		incidentsError: incidentResult === null,
		rules: ruleResult?.rules ?? [],
		ruleTotal: ruleResult?.total ?? 0,
		rulesError: ruleResult === null,
		deliveries: deliveryResult?.deliveries ?? []
	};
};
