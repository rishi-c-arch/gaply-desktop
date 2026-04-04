const isLocalhost = Boolean(
  window.location.hostname === 'localhost' ||
  window.location.hostname === '[::1]' ||
  window.location.hostname.match(/^127(?:\.(?:25[0-5]|2[0-4]\d|[01]?\d\d?)){3}$/)
);

export function register(): void {
  if ('serviceWorker' in navigator) {
    const publicUrl = new URL(process.env.PUBLIC_URL || '', window.location.href);
    if (publicUrl.origin !== window.location.origin) return;

    window.addEventListener('load', () => {
      const swUrl = `${process.env.PUBLIC_URL}/service-worker.js`;

      if (isLocalhost) {
        checkValidServiceWorker(swUrl);
      } else {
        registerValidSW(swUrl);
      }
    });
  }
}

function attachRegistrationHandlers(registration: ServiceWorkerRegistration): void {
  registration.onupdatefound = () => {
    const installingWorker = registration.installing;
    if (!installingWorker) return;

    installingWorker.onstatechange = () => {
      if (installingWorker.state === 'installed') {
        if (navigator.serviceWorker.controller) {
          console.log('New content available; please refresh.');
        } else {
          console.log('Content cached for offline use.');
        }
      }
    };
  };
}

/** Rich Results Test / some crawlers mock SW and reject with "Rejected"; privacy tools block SW — all optional. */
function registerValidSW(swUrl: string): void {
  navigator.serviceWorker
    .register(swUrl)
    .then((registration) => {
      attachRegistrationHandlers(registration);
    })
    .catch((error) => {
      if (process.env.NODE_ENV === 'development') {
        console.debug('[service-worker] Registration unavailable (expected in some tools/browsers):', error);
      }
    });
}

function checkValidServiceWorker(swUrl: string): void {
  fetch(swUrl, { headers: { 'Service-Worker': 'script' } })
    .then((response) => {
      const contentType = response.headers.get('content-type');
      if (
        response.status === 404 ||
        (contentType != null && contentType.indexOf('javascript') === -1)
      ) {
        navigator.serviceWorker.ready.then((registration) => {
          registration.unregister();
        });
      } else {
        registerValidSW(swUrl);
      }
    })
    .catch(() => {
      console.log('No internet connection. App running in offline mode.');
    });
}

export function unregister(): void {
  if ('serviceWorker' in navigator) {
    navigator.serviceWorker.ready
      .then((registration) => {
        registration.unregister();
      })
      .catch(() => {
        /* No active registration */
      });
  }
}
