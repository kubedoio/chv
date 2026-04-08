const BROWSER = false;
function experimental_async_required(name) {
  {
    throw new Error(`https://svelte.dev/e/experimental_async_required`);
  }
}
function lifecycle_outside_component(name) {
  {
    throw new Error(`https://svelte.dev/e/lifecycle_outside_component`);
  }
}
function await_invalid() {
  const error = new Error(`await_invalid
Encountered asynchronous work while rendering synchronously.
https://svelte.dev/e/await_invalid`);
  error.name = "Svelte error";
  throw error;
}
function dynamic_element_invalid_tag(tag) {
  const error = new Error(`dynamic_element_invalid_tag
\`<svelte:element this="${tag}">\` is not a valid element name — the element will not be rendered
https://svelte.dev/e/dynamic_element_invalid_tag`);
  error.name = "Svelte error";
  throw error;
}
function hydratable_serialization_failed(key, stack) {
  const error = new Error(`hydratable_serialization_failed
Failed to serialize \`hydratable\` data for key \`${key}\`.

\`hydratable\` can serialize anything [\`uneval\` from \`devalue\`](https://npmjs.com/package/uneval) can, plus Promises.

Cause:
${stack}
https://svelte.dev/e/hydratable_serialization_failed`);
  error.name = "Svelte error";
  throw error;
}
function invalid_csp() {
  const error = new Error(`invalid_csp
\`csp.nonce\` was set while \`csp.hash\` was \`true\`. These options cannot be used simultaneously.
https://svelte.dev/e/invalid_csp`);
  error.name = "Svelte error";
  throw error;
}
function server_context_required() {
  const error = new Error(`server_context_required
Could not resolve \`render\` context.
https://svelte.dev/e/server_context_required`);
  error.name = "Svelte error";
  throw error;
}
function get_render_context() {
  const store = als?.getStore();
  {
    server_context_required();
  }
  return store;
}
let als = null;
export {
  BROWSER as B,
  await_invalid as a,
  dynamic_element_invalid_tag as d,
  experimental_async_required as e,
  get_render_context as g,
  hydratable_serialization_failed as h,
  invalid_csp as i,
  lifecycle_outside_component as l
};
