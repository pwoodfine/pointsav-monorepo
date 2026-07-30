// Phase 2 Step 7 — collab bundle entry.
//
// Re-exports yjs + y-codemirror.next + y-websocket as one global namespace
// `window.CMCOLLAB`. Loaded lazily by saa-init.js only when the server
// has set window.WIKI_COLLAB_ENABLED = true (--enable-collab CLI flag).
//
// Default-off path means production deploys without the flag never load
// any of this — keeps the SAA bundle lean.

export * as yjs from 'yjs';
export * as ycm from 'y-codemirror.next';
export * as ywebsocket from 'y-websocket';
export * as yprotocols_sync from 'y-protocols/sync';
export * as yprotocols_awareness from 'y-protocols/awareness';
