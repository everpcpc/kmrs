# Using the original Komga web UI with kmrs

kmrs serves the API only — it does not bundle or host any web UI. If you want a
browser interface, you can host the official
[komga-webui](https://github.com/gotson/komga) yourself and point it at kmrs with a
reverse proxy. Everything the web UI uses is implemented in kmrs: basic and OAuth2
login, the divina/EPUB readers, live updates over SSE, and the admin pages.

## 1. Build the web UI

Build the webui from the komga source at kmrs's compatibility target:

```sh
git clone https://github.com/gotson/komga.git
cd komga
git checkout v1.27.0
cd komga-webui
npm ci
npm run build        # outputs dist/
```

Serve `dist/` at the root of its origin (e.g. `https://komga.example.com/`). The
built `index.html` resolves `window.resourceBaseUrl` to `/` on its own; hosting the
UI under a sub-path is not covered by this guide.

## 2. Reverse-proxy kmrs

The web UI calls the API on its own origin, so nginx must route the API prefixes to
kmrs and serve everything else from `dist/`:

```nginx
server {
    listen 443 ssl;
    server_name komga.example.com;

    root /var/www/komga-webui;   # the dist/ directory from step 1
    index index.html;

    # REST API: /api/v1, /api/v2, /api/logout
    location /api/ {
        proxy_pass http://127.0.0.1:25600;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_set_header X-Forwarded-Host $host;
    }

    # SSE live updates: buffering off, long read timeout
    location /sse/ {
        proxy_pass http://127.0.0.1:25600;
        proxy_http_version 1.1;
        proxy_set_header Connection "";
        proxy_buffering off;
        proxy_read_timeout 1h;
    }

    # OAuth2 login (only needed if you configure providers in kmrs)
    location /oauth2/       { proxy_pass http://127.0.0.1:25600; }
    location /login/oauth2/ { proxy_pass http://127.0.0.1:25600; }

    # admin pages: server management, metrics, updates
    location /actuator/ { proxy_pass http://127.0.0.1:25600; }

    # everything else is the SPA; history-mode routes fall back to index.html
    location / {
        try_files $uri $uri/ /index.html;
    }
}
```

`X-Forwarded-Proto`/`X-Forwarded-Host` matter: kmrs builds absolute URLs (the OAuth2
`redirect_uri`, OPDS links) from them, like Java's `forward-headers-strategy:
framework`.

## Prefix reference

kmrs listens on the following top-level prefixes. Only the first five are needed by
the web UI; add the others if you use the matching clients.

| Prefix | Purpose | Needed by |
| --- | --- | --- |
| `/api/` | REST API (`/api/v1`, `/api/v2`, `/api/logout`) | web UI |
| `/sse/` | SSE event stream (`/sse/v1/events`) | web UI |
| `/oauth2/` | OAuth2 authorization entry | web UI (with OAuth2 providers) |
| `/login/oauth2/` | OAuth2 callback | web UI (with OAuth2 providers) |
| `/actuator/` | info, shutdown, logfile, metrics | web UI admin pages |
| `/opds/` | OPDS v1.2/v2 catalogs | OPDS readers |
| `/koreader/` | KOReader progress sync | KOReader |
| `/kobo/` | Kobo sync | Kobo |
| `/v3/api-docs` | OpenAPI document | optional |
| `/debug/` | heap profiling endpoint | optional |

## Notes

- `/login` is a web UI route; only `/login/oauth2/` goes to kmrs. nginx matches the
  longest prefix, so the two `location` blocks coexist safely.
- Reader pages, thumbnails, and downloads are plain same-origin requests
  authenticated by the session cookie — no extra proxy rules needed.
- `GET /actuator/logfile` returns an empty body (kmrs logs to stderr and keeps no
  log file); the download button in Server Management still succeeds.
- Cross-origin hosting (web UI on a different origin than the API) is not supported:
  kmrs parses `KOMGA_CORS_ALLOWEDORIGINS` but does not apply CORS headers. Proxy
  same-origin as shown above.
