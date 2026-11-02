/*
 * Basalt remote shell.
 *
 * The phone client is useless without the desktop, so this is not an offline
 * app: the worker only keeps hashed build assets around so a transient
 * disconnect does not blank the screen mid-task.
 */
const CACHE = "basalt-shell-v1";

self.addEventListener("install", () => {
  self.skipWaiting();
});

self.addEventListener("activate", (event) => {
  event.waitUntil(self.clients.claim());
});

self.addEventListener("fetch", (event) => {
  const request = event.request;
  if (request.method !== "GET") return;

  const url = new URL(request.url);
  if (url.origin !== self.location.origin) return;
  // Never intercept the gateway itself.
  if (url.pathname.startsWith("/api/")) return;

  if (url.pathname.startsWith("/assets/")) {
    event.respondWith(
      caches.open(CACHE).then(async (cache) => {
        const hit = await cache.match(request);
        if (hit) return hit;
        const response = await fetch(request);
        if (response.ok) cache.put(request, response.clone());
        return response;
      }),
    );
    return;
  }

  event.respondWith(
    fetch(request)
      .then((response) => {
        if (response.ok && request.mode === "navigate") {
          const copy = response.clone();
          cachePut("/", copy);
        }
        return response;
      })
      .catch(async () => {
        const cached = await caches.match(request);
        if (cached) return cached;
        const shell = await caches.match("/");
        return shell || Response.error();
      }),
  );
});

function cachePut(key, response) {
  caches
    .open(CACHE)
    .then((cache) => cache.put(key, response))
    .catch(() => {});
}
