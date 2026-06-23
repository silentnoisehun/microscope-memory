// Mermaid PWA service worker -- hot-swap tamogatassal
// Stale-while-revalidate a modulokra, network-first a shell-re

const CACHE = 'mermaid-pwa-v1';
const ASSETS = [
  './',
  './index.html',
  './manifest.json',
  './icon.svg',
  './styles/core.css',
  './styles/chat.css',
  './styles/voice.css',
  './styles/diff.css',
  './styles/dashboard.css',
  './styles/settings.css',
  './modules/rongyasz.js',
  './modules/skills.js',
  './modules/core.js',
  './modules/chat.js',
  './modules/voice.js',
  './modules/diff.js',
  './modules/dashboard.js',
  './modules/pipeline.js',
  './modules/settings.js'
];

self.addEventListener('install', function (e) {
  self.skipWaiting();
  e.waitUntil(
    caches.open(CACHE).then(function (c) { return c.addAll(ASSETS).catch(function () {}); }).catch(function () {})
  );
});

self.addEventListener('activate', function (e) {
  e.waitUntil(
    caches.keys().then(function (ks) { return Promise.all(ks.map(function (k) { if (k !== CACHE) return caches.delete(k); })); }).then(function () { return self.clients.claim(); })
  );
});

// Network-first for HTML, stale-while-revalidate for modules, cache-first otherwise
self.addEventListener('fetch', function (e) {
  var url = new URL(e.request.url);
  var isModule = url.pathname.indexOf('/modules/') >= 0;
  var isCSS = url.pathname.indexOf('/styles/') >= 0;
  var isHTML = url.pathname.endsWith('/index.html') || url.pathname === '/' || url.pathname.endsWith('/mermaid/');

  if (isHTML) {
    e.respondWith(
      fetch(e.request).then(function (r) {
        var clone = r.clone();
        caches.open(CACHE).then(function (c) { c.put(e.request, clone); });
        return r;
      }).catch(function () { return caches.match(e.request); })
    );
    return;
  }

  if (isModule || isCSS) {
    e.respondWith(
      caches.open(CACHE).then(function (cache) {
        return cache.match(e.request).then(function (cached) {
          var networkFetch = fetch(e.request).then(function (r) {
            if (r && r.ok) {
              var cloneForCache = r.clone();
              cache.put(e.request, cloneForCache);
              if (cached) {
                return r.text().then(function (newText) {
                  return cached.text().then(function (oldText) {
                    if (newText !== oldText) {
                      var modName = url.pathname.split('/').pop().replace('.js', '').replace('.css', '');
                      return self.clients.matchAll().then(function (clients) {
                        clients.forEach(function (c) {
                          c.postMessage({ type: 'module-updated', module: modName });
                        });
                        return new Response(newText, { headers: r.headers });
                      });
                    }
                    return new Response(newText, { headers: r.headers });
                  });
                });
              }
              return r;
            }
            return cached || new Response('offline', { status: 503 });
          }).catch(function () { return cached || new Response('offline', { status: 503 }); });
          return cached || networkFetch;
        });
      })
    );
    return;
  }

  e.respondWith(
    caches.match(e.request).then(function (r) { return r || fetch(e.request).catch(function () { return new Response('offline', { status: 503 }); }); })
  );
});