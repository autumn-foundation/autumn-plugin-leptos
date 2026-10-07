/* Test probe for old browsers: no requestIdleCallback and no
 * IntersectionObserver. It loads before the loader. */
delete window.requestIdleCallback;
delete window.cancelIdleCallback;
delete window.IntersectionObserver;
