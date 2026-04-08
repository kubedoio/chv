import { p as ssr_context } from "./renderer.js";
import "clsx";
function onDestroy(fn) {
  /** @type {SSRContext} */
  ssr_context.r.on_destroy(fn);
}
export {
  onDestroy as o
};
