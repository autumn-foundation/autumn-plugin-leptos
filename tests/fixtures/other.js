// Test bundle: a second fake bundle with one component, `Other`.
export default async function init(options) {
  await options.module_or_path;
}

export function autumn_leptos_abi() {
  return 1;
}

export function autumn_leptos_start() {
  return ['Other'];
}

export function autumn_leptos_mount(name, el) {
  const b = document.createElement('b');
  b.className = 'other';
  b.textContent = 'other';
  el.appendChild(b);
  return { update() {}, unmount() { b.remove(); } };
}
