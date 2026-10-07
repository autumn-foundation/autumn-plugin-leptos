/* Test probe. It loads before the loader. It holds idle callbacks until the
 * test runs them, counts IntersectionObserver calls, and writes each loader
 * event to window.__leptosLog as "type:name:text". */
(function () {
  'use strict';
  var log = (window.__leptosLog = window.__leptosLog || []);
  ['mount', 'update', 'unmount', 'error'].forEach(function (type) {
    document.addEventListener('autumn:leptos:' + type, function (event) {
      var detail = event.detail || {};
      var name = detail.name || (detail.bundle ? 'bundle' : '?');
      var el = detail.element;
      log.push(type + ':' + name + ':' + (el ? el.textContent : ''));
    });
  });
  var queue = [];
  window.__idleCancels = 0;
  window.requestIdleCallback = function (callback) {
    queue.push(callback);
    return queue.length;
  };
  window.cancelIdleCallback = function (id) {
    window.__idleCancels++;
    queue[id - 1] = null;
  };
  window.__runIdle = function () {
    var callbacks = queue;
    queue = [];
    callbacks.forEach(function (callback) {
      if (callback) callback({ didTimeout: false, timeRemaining: function () { return 50; } });
    });
  };
  var unobserve = IntersectionObserver.prototype.unobserve;
  window.__unobserves = 0;
  IntersectionObserver.prototype.unobserve = function () {
    window.__unobserves++;
    return unobserve.apply(this, arguments);
  };
})();
