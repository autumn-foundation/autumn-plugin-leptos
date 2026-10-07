// Test bundle: an ABI version that the loader does not know.
export default async function init(options) {
  await options.module_or_path;
}

export function autumn_leptos_abi() {
  return 2;
}

export function autumn_leptos_start() {
  return ['Bad'];
}

export function autumn_leptos_mount() {
  throw new Error('must not mount');
}
