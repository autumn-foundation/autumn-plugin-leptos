// Test bundle: registers `Echo` and `Late`. `Echo` is a duplicate.
export default async function init(options) {
  await options.module_or_path;
}

export function autumn_leptos_abi() {
  return 1;
}

export function autumn_leptos_start() {
  return ['Echo', 'Late'];
}

export function autumn_leptos_mount(name, el) {
  const b = document.createElement('b');
  b.className = 'dup';
  b.textContent = 'from dup: ' + name;
  el.appendChild(b);
  return { update() {}, unmount() { b.remove(); } };
}
