// Entrypoint for the cloudflare worker runtime.

import * as imports from "./pyoci.js";
import wasmModule from "./pyoci_bg.wasm";
import { WorkerEntrypoint } from "cloudflare:workers";

// Run the worker's initialization function.
imports.start?.();

export { wasmModule };

class Entrypoint extends WorkerEntrypoint {
    async fetch(request) {
        return await imports.fetch(request, this.env, this.ctx)
    }

    // async queue(batch) {
    //     return await imports.queue(batch, this.env, this.ctx)
    // }
    //
    // async scheduled(event) {
    //     return await imports.scheduled(event, this.env, this.ctx)
    // }
}

const EXCLUDE_EXPORT = [
    "IntoUnderlyingByteSource",
    "IntoUnderlyingSink",
    "IntoUnderlyingSource",
    "MinifyConfig",
    "PolishConfig",
    "R2Range",
    "RequestRedirect",
    "fetch",
    "queue",
    "scheduled",
    "getMemory"
];

Object.keys(imports).map(k => {
    if (!(EXCLUDE_EXPORT.includes(k) | k.startsWith("__"))) {
        Entrypoint.prototype[k] = imports[k];
    }
})

export default Entrypoint;
