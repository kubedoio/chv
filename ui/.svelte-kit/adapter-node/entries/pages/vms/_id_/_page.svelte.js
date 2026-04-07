import { c as escape_html, s as store_get, u as unsubscribe_stores } from "../../../../chunks/renderer.js";
import { p as page } from "../../../../chunks/stores.js";
function _page($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    var $$store_subs;
    $$renderer2.push(`<section class="grid gap-4 lg:grid-cols-2"><div class="table-card"><div class="card-header px-4 py-3"><div class="text-[11px] uppercase tracking-[0.16em] text-muted">VM Detail</div> <div class="mt-1 text-lg font-semibold mono">${escape_html(store_get($$store_subs ??= {}, "$page", page).params.id)}</div></div> <dl class="grid grid-cols-[160px_minmax(0,1fr)] gap-x-4 gap-y-3 p-4 text-sm"><dt class="text-muted">QCOW2 disk</dt> <dd class="mono">/var/lib/chv/vms/&lt;vm-id>/disk.qcow2</dd> <dt class="text-muted">Seed ISO</dt> <dd class="mono">/var/lib/chv/vms/&lt;vm-id>/seed.iso</dd> <dt class="text-muted">Workspace</dt> <dd class="mono">/var/lib/chv/vms/&lt;vm-id></dd> <dt class="text-muted">Cloud-init</dt> <dd>user-data, meta-data, optional network-config</dd> <dt class="text-muted">Operations</dt> <dd>Activity history will be listed from the controller log.</dd></dl></div> <div class="table-card"><div class="card-header px-4 py-3"><div class="text-[11px] uppercase tracking-[0.16em] text-muted">State Notes</div> <div class="mt-1 text-lg font-semibold">Seed ISO Boot Gate</div></div> <div class="space-y-3 p-4 text-sm text-muted"><p>VM boot remains blocked until the image, storage pool, network, workspace, and \`seed.iso\` are ready.</p> <p>This route is intentionally conservative for MVP-1 and should expose only backend-confirmed state.</p></div></div></section>`);
    if ($$store_subs) unsubscribe_stores($$store_subs);
  });
}
export {
  _page as default
};
