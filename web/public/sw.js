const CACHE_NAME = 'synapse-v1';
const STATIC_ASSETS = [
  '/',
  '/index.html',
  '/manifest.json',
];

self.addEventListener('install', (event) => {
  event.waitUntil(
    caches.open(CACHE_NAME).then((cache) => cache.addAll(STATIC_ASSETS)).catch(() => {})
  );
  self.skipWaiting();
});

self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches.keys().then((keys) =>
      Promise.all(keys.filter((k) => k !== CACHE_NAME).map((k) => caches.delete(k)))
    )
  );
  self.clients.claim();
});

self.addEventListener('fetch', (event) => {
  // Don't intercept navigation requests — let the browser handle them directly
  if (event.request.mode === 'navigate') {
    return;
  }

  // Don't intercept WebSocket upgrade or non-GET requests
  if (event.request.method !== 'GET') {
    return;
  }

  // Skip API and WS requests entirely — no caching for dynamic data
  if (event.request.url.includes('/api/') || event.request.url.includes('/ws/')) {
    return;
  }

  // Cache-first for static assets, with network fallback
  event.respondWith(
    caches.match(event.request).then((cached) => {
      if (cached) return cached;
      return fetch(event.request).catch(() => {
        // Return empty response instead of throwing
        return new Response('', { status: 503, statusText: 'Offline' });
      });
    })
  );
});

// Push notification handler
self.addEventListener('push', (event) => {
  const data = event.data ? event.data.json() : { title: 'Synapse', body: 'Task completed' };
  event.waitUntil(
    self.registration.showNotification(data.title || 'Synapse', {
      body: data.body || '',
      icon: '/icon-192.png',
      badge: '/icon-192.png',
    })
  );
});

self.addEventListener('notificationclick', (event) => {
  event.notification.close();
  event.waitUntil(clients.openWindow('/'));
});
