// autumn-plugin-leptos: the island loader.
//
// Loads each Leptos wasm bundle (`link[data-leptos-wasm]`) and mounts its
// components into `[data-leptos-island]` elements. A bundle is the output
// of `wasm-bindgen --target web`. It has the ABI of
// `autumn-plugin-leptos-client`.
//
// One MutationObserver mounts added islands, unmounts removed islands and
// sends new props when `data-leptos-props` changes. No eval, no HTML
// strings, no inline script.
(function () {
  'use strict';

  const ABI = 1;
  const ISLAND = 'data-leptos-island';
  const PROPS = 'data-leptos-props';
  const MOUNT = 'data-leptos-mount';
  const STATE = 'data-leptos-state';
  const IGNORE = 'data-leptos-ignore';
  const WASM = 'data-leptos-wasm';
  const WASM_INTEGRITY = 'data-leptos-wasm-integrity';
  const ISLAND_SELECTOR = '[' + ISLAND + ']';
  const IGNORE_SELECTOR = '[' + IGNORE + ']';
  const BUNDLE_SELECTOR = 'link[' + WASM + ']';
  const PREFIX = 'autumn-leptos: ';

  const win = window;
  try {
    // A second copy of the loader does nothing.
    const existing = win.autumnLeptos;
    if (existing && existing.loader === true) return;
  } catch (error) {
    // A cross-origin frame named "autumnLeptos" throws on access.
  }

  // Prototype methods and getters: an element named like a DOM property
  // (`<form name="documentElement">`) cannot clobber them.
  const N = Node.prototype;
  const E = Element.prototype;
  const D = Document.prototype;
  const T = EventTarget.prototype;
  const getter = (proto, name) => Object.getOwnPropertyDescriptor(proto, name).get;
  const isConnected = getter(N, 'isConnected');
  const childNodes = getter(N, 'childNodes');
  const parentElement = getter(N, 'parentElement');
  const nodeType = getter(N, 'nodeType');
  const baseURI = getter(N, 'baseURI');
  const documentElement = getter(D, 'documentElement');
  const readyState = getter(D, 'readyState');
  const connected = (node) => isConnected.call(node);
  const isElement = (node) => nodeType.call(node) === 1;
  const contains = (outer, inner) => N.contains.call(outer, inner);
  const getAttr = (el, name) => E.getAttribute.call(el, name);
  const setAttr = (el, name, value) => E.setAttribute.call(el, name, value);
  const closest = (el, selector) => E.closest.call(el, selector);
  const findAll = (el, selector) => E.querySelectorAll.call(el, selector);
  const matches = (el, selector) => E.matches.call(el, selector);
  const on = (target, type, listener) => T.addEventListener.call(target, type, listener);
  const isPlainObject = (value) =>
    value !== null && typeof value === 'object' && !Array.isArray(value);
  const absolute = (url) => new URL(url, baseURI.call(document)).href;

  /** glue URL -> { url, state, mod, error }. */
  const bundles = new Map();
  /** component name -> bundle */
  const registry = new Map();
  /** element -> record */
  const records = new WeakMap();
  /** Records with a handle, a pending trigger, or a pending name. */
  const live = new Set();

  function dispatch(target, type, detail) {
    T.dispatchEvent.call(target, new CustomEvent('autumn:leptos:' + type, { bubbles: true, detail }));
  }

  function emit(record, type, error) {
    const target = connected(record.el) ? record.el : document;
    const detail = { name: record.name, element: record.el };
    if (error !== undefined) detail.error = error;
    dispatch(target, type, detail);
  }

  function setState(record, state) {
    record.state = state;
    setAttr(record.el, STATE, state);
  }

  function fail(record, message, error) {
    console.error(PREFIX + 'island "' + record.name + '" ' + message, error);
    setState(record, 'error');
    emit(record, 'error', error);
  }

  function propsText(record) {
    const text = getAttr(record.el, PROPS);
    return text === null || text === '' ? '{}' : text;
  }

  // Moves the fallback nodes out of the island, into the record.
  function takeFallback(record) {
    record.fallback = Array.from(childNodes.call(record.el));
    E.replaceChildren.call(record.el);
  }

  // Puts the fallback back. It also removes other children.
  function restoreFallback(record) {
    if (record.fallback) E.replaceChildren.apply(record.el, record.fallback);
    record.fallback = null;
  }

  function unmountHandle(record) {
    const handle = record.handle;
    if (!handle) return false;
    record.handle = null;
    try {
      handle.unmount();
    } catch (error) {
      console.error(PREFIX + 'island "' + record.name + '" did not unmount', error);
    }
    return true;
  }

  function mount(record, bundle) {
    record.bundle = bundle;
    takeFallback(record);
    let handle;
    try {
      handle = bundle.mod.autumn_leptos_mount(record.name, record.el, propsText(record));
    } catch (error) {
      restoreFallback(record);
      fail(record, 'did not mount:', error);
      return;
    }
    record.handle = handle;
    setState(record, 'mounted');
    emit(record, 'mount');
  }

  // Sets new props on the same instance. Leptos keeps the state.
  function update(record) {
    const handle = record.handle;
    try {
      handle.update(propsText(record));
    } catch (error) {
      fail(record, 'has bad props:', error);
      return;
    }
    setState(record, 'mounted');
    // Leptos changes the DOM in a microtask. Send the event after it.
    setTimeout(() => {
      if (record.handle === handle) emit(record, 'update');
    }, 0);
  }

  function activate(record) {
    record.cancel = null;
    const bundle = registry.get(record.name);
    if (!bundle) setState(record, 'pending');
    else if (bundle.state === 'crashed') fail(record, 'did not mount: its bundle crashed', bundle.error);
    else mount(record, bundle);
  }

  let visibility = null;
  function observeVisible(record) {
    if (typeof IntersectionObserver !== 'function') {
      activate(record);
      return;
    }
    if (!visibility) {
      visibility = new IntersectionObserver((entries) => {
        for (const entry of entries) {
          if (!entry.isIntersecting) continue;
          visibility.unobserve(entry.target);
          const record = records.get(entry.target);
          if (record && record.state === 'waiting') activate(record);
        }
      });
    }
    visibility.observe(record.el);
    record.cancel = () => visibility.unobserve(record.el);
  }

  function waitForIdle(record) {
    const run = () => activate(record);
    if (typeof win.requestIdleCallback === 'function') {
      const id = win.requestIdleCallback(run, { timeout: 2000 });
      record.cancel = () => win.cancelIdleCallback(id);
    } else {
      const id = setTimeout(run, 200);
      record.cancel = () => clearTimeout(id);
    }
  }

  // Islands in `[data-leptos-ignore]` or in another island never mount.
  function skipped(el) {
    if (closest(el, IGNORE_SELECTOR)) return true;
    const parent = parentElement.call(el);
    return parent !== null && closest(parent, ISLAND_SELECTOR) !== null;
  }

  // Stops all work for a record and puts the fallback back.
  function teardown(record) {
    if (record.cancel) record.cancel();
    record.cancel = null;
    const hadHandle = unmountHandle(record);
    restoreFallback(record);
    E.removeAttribute.call(record.el, STATE);
    records.delete(record.el);
    live.delete(record);
    if (hadHandle) emit(record, 'unmount');
  }

  // Tears down each live record at or inside `el`.
  function teardownWithin(el) {
    for (const record of Array.from(live)) {
      if (contains(el, record.el)) teardown(record);
    }
  }

  // A panic stops the wasm instance. Put back the fallback of each island
  // of the bundle. Make no call into the bundle.
  function crash(bundle, message) {
    if (!bundle || bundle.state !== 'ready') return;
    bundle.state = 'crashed';
    bundle.error = new Error('the wasm bundle panicked: ' + message);
    for (const record of Array.from(live)) {
      if (record.bundle !== bundle || !record.handle) continue;
      record.handle = null;
      restoreFallback(record);
      fail(record, 'crashed:', bundle.error);
    }
  }

  function consider(el) {
    // Before DOMContentLoaded the island can be half parsed.
    if (readyState.call(document) === 'loading') return;
    if (!connected(el) || records.has(el) || skipped(el)) return;
    // htmx history restore brings back old Leptos output, not a fallback.
    if (getAttr(el, STATE) === 'mounted') E.replaceChildren.call(el);
    // A live island inside a new island belongs to the new island now.
    for (const record of Array.from(live)) {
      if (record.el !== el && contains(el, record.el)) teardown(record);
    }
    const record = {
      el,
      name: getAttr(el, ISLAND),
      state: null,
      handle: null,
      bundle: null,
      cancel: null,
      fallback: null,
    };
    records.set(el, record);
    live.add(record);
    const when = getAttr(el, MOUNT);
    if (when === 'idle' || when === 'visible') {
      setState(record, 'waiting');
      if (when === 'idle') waitForIdle(record);
      else observeVisible(record);
    } else {
      activate(record);
    }
  }

  // Checks `el` and each island in it. A live island that moved into an
  // ignored region or into another island is torn down.
  function scan(el) {
    const islands = Array.from(findAll(el, ISLAND_SELECTOR));
    if (matches(el, ISLAND_SELECTOR)) islands.unshift(el);
    for (const island of islands) {
      const record = records.get(island);
      if (!record) consider(island);
      else if (skipped(island)) teardown(record);
    }
  }

  function failBundle(bundle, error) {
    bundle.state = 'failed';
    bundle.error = error;
    console.error(PREFIX + 'bundle ' + bundle.url + ' did not load:', error);
    dispatch(document, 'error', { name: null, element: null, bundle: bundle.url, error });
  }

  // Registers the names of a loaded bundle. Mounts the waiting islands.
  function ready(bundle, mod) {
    if (typeof mod.autumn_leptos_abi !== 'function' || mod.autumn_leptos_abi() !== ABI) {
      throw new Error(
        'the bundle ABI is not ' + ABI + '; use matching versions of ' +
          'autumn-plugin-leptos and autumn-plugin-leptos-client',
      );
    }
    const names = mod.autumn_leptos_start(bundle.url);
    if (!Array.isArray(names)) throw new TypeError('autumn_leptos_start must return names');
    bundle.mod = mod;
    bundle.state = 'ready';
    const added = new Set();
    for (const name of names) {
      if (typeof name !== 'string') continue;
      if (registry.has(name)) {
        console.error(PREFIX + 'component "' + name + '" is already registered; the first one stays');
        continue;
      }
      registry.set(name, bundle);
      added.add(name);
    }
    for (const record of Array.from(live)) {
      if (record.state === 'pending' && added.has(record.name)) activate(record);
    }
  }

  // Imports the glue module and starts it with an SRI-checked wasm fetch.
  function load(link) {
    const href = getAttr(link, 'href');
    const wasm = getAttr(link, WASM);
    if (!href || !wasm) return;
    let url;
    try {
      url = absolute(href);
    } catch (error) {
      return;
    }
    if (bundles.has(url)) return;
    const bundle = { url, state: 'loading', mod: null, error: null };
    bundles.set(url, bundle);
    const integrity = getAttr(link, WASM_INTEGRITY);
    const options = { credentials: 'same-origin' };
    if (integrity) options.integrity = integrity;
    import(bundle.url)
      .then((mod) => {
        const response = fetch(absolute(wasm), options);
        return Promise.resolve(mod.default({ module_or_path: response })).then(() => mod);
      })
      .then((mod) => ready(bundle, mod))
      .catch((error) => failBundle(bundle, error));
  }

  function loadWithin(el) {
    if (matches(el, BUNDLE_SELECTOR)) load(el);
    for (const link of Array.from(findAll(el, BUNDLE_SELECTOR))) load(link);
  }

  // A mutation inside an island belongs to Leptos (or to the fallback).
  function insideIsland(node) {
    const el = isElement(node) ? node : parentElement.call(node);
    return el !== null && closest(el, ISLAND_SELECTOR) !== null;
  }

  // `fresh` holds elements that got a record in this batch. They already
  // use their current attributes.
  function attributeChanged(el, attribute, fresh) {
    if (!connected(el) || fresh.has(el)) return;
    if (attribute === IGNORE) {
      if (getAttr(el, IGNORE) !== null) teardownWithin(el);
      else scan(el);
      return;
    }
    const record = records.get(el);
    if (!record) {
      if (getAttr(el, ISLAND) !== null) {
        consider(el);
        fresh.add(el);
      }
      return;
    }
    if (attribute === PROPS && record.handle) {
      update(record);
      return;
    }
    const restart =
      attribute === ISLAND ||
      (attribute === PROPS && record.state === 'error') ||
      (attribute === MOUNT && record.state === 'waiting');
    if (!restart) return;
    teardown(record);
    fresh.add(el);
    // Not an island now: the islands in it can mount.
    scan(el);
  }

  function observe(mutations) {
    // Unmount removed islands first. A moved island is connected again by
    // now, so it keeps its instance.
    if (mutations.some((m) => m.removedNodes.length > 0 && !insideIsland(m.target))) {
      for (const record of Array.from(live)) {
        if (!connected(record.el)) teardown(record);
      }
    }
    const fresh = new Set();
    for (const m of mutations) {
      if (m.type === 'attributes') {
        attributeChanged(m.target, m.attributeName, fresh);
      } else if (!insideIsland(m.target)) {
        for (const node of m.addedNodes) {
          if (!isElement(node)) continue;
          loadWithin(node);
          scan(node);
        }
      }
    }
  }

  // Sets new props from one `{ target, props }` update. `origin` is the
  // event target. It is the island when the update has no `target`.
  function applyProps(change, origin) {
    if (!isPlainObject(change)) {
      console.error(PREFIX + 'a props update must be an object', change);
      return;
    }
    let el = origin;
    if (typeof change.target === 'string') {
      try {
        el = D.querySelector.call(document, change.target);
      } catch (error) {
        el = null;
      }
    }
    if (!el || !isElement(el) || getAttr(el, ISLAND) === null) {
      console.error(PREFIX + 'no island for the props update', change.target);
      return;
    }
    if (!isPlainObject(change.props)) {
      console.error(PREFIX + 'props update for "' + getAttr(el, ISLAND) + '" is not an object');
      return;
    }
    // The observer sees the attribute change and updates the island.
    setAttr(el, PROPS, JSON.stringify(change.props));
  }

  // htmx sends `HX-Trigger: {"autumn:leptos:props": [...]}` as
  // `detail.value`. App code can send `detail: { target?, props }`.
  on(document, 'autumn:leptos:props', (event) => {
    const detail = event.detail;
    const changes = detail && Array.isArray(detail.value) ? detail.value : [detail];
    for (const change of changes) applyProps(change, event.target);
  });

  // The client panic hook sends this event before the wasm instance stops.
  on(document, 'autumn:leptos:panic', (event) => {
    const detail = event.detail;
    if (!detail || typeof detail.bundle !== 'string') return;
    crash(bundles.get(detail.bundle), detail.message);
  });

  win.autumnLeptos = {
    loader: true,
    /** The registered component names, sorted. */
    names() {
      return Array.from(registry.keys()).sort();
    },
    /** Each bundle: `{ url, state }`. State: loading, ready, failed or crashed. */
    bundles() {
      return Array.from(bundles.values(), (b) => ({ url: b.url, state: b.state }));
    },
  };

  const root = documentElement.call(document);
  new MutationObserver(observe).observe(root, {
    childList: true,
    subtree: true,
    attributes: true,
    attributeFilter: [ISLAND, PROPS, MOUNT, IGNORE],
  });
  loadWithin(root);
  if (readyState.call(document) === 'loading') {
    on(document, 'DOMContentLoaded', () => {
      const el = documentElement.call(document);
      loadWithin(el);
      scan(el);
    });
  } else {
    scan(root);
  }
})();
