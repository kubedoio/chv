<script lang="ts">
	// This route redirects to `/` (see +page.ts — the nav entry and its
	// `/observability` alias land on the fleet overview, which now hosts
	// the monitoring health surface and per-target metric panels).
	//
	// History: this page used to render a "Live Telemetry Sequence"
	// dashboard with fabricated content (a claimed Prometheus/OTel
	// gateway, invented "API response < 5ms"/"bus saturation" posture
	// rows, and a synthetic 5.0s "observation depth"). None of it was
	// real. It was removed with the native monitoring stack (#602
	// PR-2): the honest replacements are the MonitoringHealthCard on
	// the overview page and the TargetMonitoringPanel on node/VM detail
	// pages, both fed by the real /v1/monitoring/* read API.
</script>

<!-- Redirect target: the +page.ts load function always throws a 307 to
     `/` before this component renders. -->
