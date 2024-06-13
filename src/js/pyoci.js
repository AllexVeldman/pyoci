import * as imports from "./pyoci_bg.js";

import wkmod from "./pyoci_bg.wasm";
const instance = new WebAssembly.Instance(wkmod, {"./pyoci_bg.js": imports});
imports.__wbg_set_wasm(instance.exports);

export * from "./pyoci_bg.js";
