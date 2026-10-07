// Test bundle: a fake wasm-bindgen glue module with the client ABI 1.
// It writes each lifecycle step to window.__leptosLog.
const log = (window.__leptosLog = window.__leptosLog || []);
let bundle = null;
let instances = 0;

export default async function init(options) {
  const response = await options.module_or_path;
  if (!response.ok) throw new Error('wasm fetch failed: ' + response.status);
  const bytes = new Uint8Array(await response.arrayBuffer());
  if (bytes[1] !== 0x61) throw new Error('not wasm');
  log.push('init');
}

export function autumn_leptos_abi() {
  return 1;
}

export function autumn_leptos_start(key) {
  bundle = key;
  log.push('start');
  return ['Echo', 'Broken', 'Partial', 'Panic'];
}

function parse(text) {
  const value = JSON.parse(text);
  if (value === null || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error('props must be a JSON object');
  }
  return value;
}

function panic(message) {
  document.dispatchEvent(
    new CustomEvent('autumn:leptos:panic', { detail: { bundle, message } }),
  );
}

// A panic in an event handler, after the mount.
window.__fakePanic = () => panic('late');

class Handle {
  constructor(el, props) {
    instances += 1;
    this.span = document.createElement('span');
    this.span.className = 'echo';
    this.span.dataset.instance = String(instances);
    this.span.textContent = JSON.stringify(props);
    el.appendChild(this.span);
    log.push('create:' + this.span.textContent);
  }

  update(text) {
    this.span.textContent = JSON.stringify(parse(text));
    log.push('set:' + this.span.textContent);
  }

  unmount() {
    this.span.remove();
    log.push('destroy:' + this.span.textContent);
  }
}

export function autumn_leptos_mount(name, el, text) {
  if (name === 'Broken') throw new Error('broken mount');
  if (name === 'Partial') {
    el.appendChild(document.createElement('b'));
    throw new Error('partial');
  }
  if (name === 'Panic') {
    panic('at mount');
    throw new WebAssembly.RuntimeError('unreachable');
  }
  if (name !== 'Echo') throw new Error('no island component is registered as `' + name + '`');
  return new Handle(el, parse(text));
}
