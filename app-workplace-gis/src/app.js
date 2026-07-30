// Workplace GIS — MapLibre GL frontend
//
// Real (non-stub) implementation: initializes a MapLibre GL map against a
// configurable tile-source endpoint, renders a toggleable T1/T2/T3 cluster
// overlay loaded from a local GeoJSON file, wires click-for-details popups,
// and exports the current view as a PNG. This has NOT been exercised in a
// browser in this session — this is a headless Linux VM with no WebView/GPU
// surface — so treat this as hand-reviewed code, not smoke-tested code.

// Tauri v2: invoke moved from __TAURI__.tauri to __TAURI__.core.
const { invoke } = window.__TAURI__.core;

const overlay = document.getElementById('overlay');
const overlayPanel = document.getElementById('overlay-panel');
const endpointLabel = document.getElementById('endpoint-label');
const layerListEl = document.getElementById('layer-list');
const loadGeojsonBtn = document.getElementById('load-geojson-btn');
const exportBtn = document.getElementById('export-btn');
const changeEndpointBtn = document.getElementById('change-endpoint-btn');

/** @type {import('maplibre-gl').Map | null} */
let map = null;

// Tiers visible on the cluster overlay layer; all on by default.
const visibleTiers = new Set(['t1', 't2', 't3']);

function showOverlay(html) {
  overlayPanel.innerHTML = html;
  overlay.classList.remove('hidden');
}

function hideOverlay() {
  overlay.classList.add('hidden');
}

function renderSetupForm(defaultEndpoint) {
  showOverlay(`
    <div class="setup-form">
      <p><strong>First-run setup</strong></p>
      <label for="endpoint-input">Tile server endpoint (gis.woodfinegroup.com or a PPN address)</label>
      <input id="endpoint-input" type="text" value="${defaultEndpoint}" />
      <div class="actions" style="flex-direction: row; justify-content: center;">
        <button class="primary" id="save-endpoint-btn">Save &amp; Connect</button>
      </div>
    </div>
  `);
  document.getElementById('save-endpoint-btn').addEventListener('click', async () => {
    const endpoint = document.getElementById('endpoint-input').value.trim();
    if (!endpoint) return;
    await invoke('set_tile_endpoint', { endpoint });
    await bootstrap();
  });
}

function renderError(detail) {
  showOverlay(`
    <p>⚠ Could not initialize the map.</p>
    <p class="error-detail">${detail}</p>
    <div class="actions" style="flex-direction: row; justify-content: center;">
      <button class="primary" id="retry-btn">Retry</button>
    </div>
  `);
  document.getElementById('retry-btn').addEventListener('click', bootstrap);
}

/** Builds a minimal MapLibre style: a raster XYZ base layer against the
 * configured endpoint, plus an empty GeoJSON source ready to receive a
 * loaded cluster overlay. The exact tile schema served by the configured
 * endpoint is not yet documented (Wave 2 scope names T1/T2/T3 layers with
 * no further contract) — this raster XYZ convention is the most common
 * default and is meant to be replaced once the real endpoint's style.json
 * (or tile contract) is known. */
function buildStyle(endpoint) {
  const base = endpoint.replace(/\/$/, '');
  return {
    version: 8,
    sources: {
      'base-tiles': {
        type: 'raster',
        tiles: [`${base}/tiles/{z}/{x}/{y}.png`],
        tileSize: 256,
        attribution: 'Sovereign GIS tile source',
      },
      clusters: {
        type: 'geojson',
        data: { type: 'FeatureCollection', features: [] },
      },
    },
    layers: [
      {
        id: 'base-tiles-layer',
        type: 'raster',
        source: 'base-tiles',
      },
      {
        id: 'clusters-layer',
        type: 'circle',
        source: 'clusters',
        paint: {
          'circle-radius': 6,
          'circle-stroke-width': 1,
          'circle-stroke-color': '#1e1e2e',
          'circle-color': [
            'match',
            ['get', 'tier'],
            't1', '#f38ba8',
            't2', '#fab387',
            't3', '#89b4fa',
            /* default */ '#a6adc8',
          ],
        },
        filter: ['in', ['get', 'tier'], ['literal', Array.from(visibleTiers)]],
      },
    ],
  };
}

function renderLayerToggles(layers) {
  layerListEl.innerHTML = layers
    .map(
      (layer) => `
    <div class="layer-row">
      <input type="checkbox" id="layer-${layer.id}" checked />
      <label for="layer-${layer.id}">${layer.label}</label>
    </div>
  `
    )
    .join('');

  layers.forEach((layer) => {
    document.getElementById(`layer-${layer.id}`).addEventListener('change', (e) => {
      if (e.target.checked) {
        visibleTiers.add(layer.id);
      } else {
        visibleTiers.delete(layer.id);
      }
      if (map && map.getLayer('clusters-layer')) {
        map.setFilter('clusters-layer', [
          'in',
          ['get', 'tier'],
          ['literal', Array.from(visibleTiers)],
        ]);
      }
    });
  });
}

function initMap(endpoint) {
  map = new window.maplibregl.Map({
    container: 'map',
    style: buildStyle(endpoint),
    center: [0, 0],
    zoom: 1,
  });

  map.addControl(new window.maplibregl.NavigationControl(), 'top-right');

  // Navigate/zoom/click-for-details (Wave 2 scope).
  map.on('click', 'clusters-layer', (e) => {
    const feature = e.features && e.features[0];
    if (!feature) return;
    const props = feature.properties || {};
    const rows = Object.entries(props)
      .map(([k, v]) => `<div><strong>${k}:</strong> ${v}</div>`)
      .join('');
    new window.maplibregl.Popup()
      .setLngLat(e.lngLat)
      .setHTML(rows || '<em>No properties</em>')
      .addTo(map);
  });

  map.on('mouseenter', 'clusters-layer', () => {
    map.getCanvas().style.cursor = 'pointer';
  });
  map.on('mouseleave', 'clusters-layer', () => {
    map.getCanvas().style.cursor = '';
  });

  map.on('error', (e) => {
    // Non-fatal: e.g. the configured tile endpoint is unreachable. Surface
    // it quietly in the sidebar rather than tearing down the whole map, so
    // navigation and the overlay-loading workflow keep working offline.
    endpointLabel.textContent = `${endpoint} (tile load error — check endpoint)`;
    // eslint-disable-next-line no-console
    console.warn('MapLibre error', e && e.error);
  });
}

async function loadGeojsonOverlay() {
  const result = await invoke('load_geojson_file');
  if (!result) return; // dialog cancelled
  let parsed;
  try {
    parsed = JSON.parse(result.contents);
  } catch (err) {
    showOverlay(`
      <p>⚠ Could not parse GeoJSON file.</p>
      <p class="error-detail">${result.path} — ${err.message}</p>
      <div class="actions" style="flex-direction: row; justify-content: center;">
        <button class="primary" id="dismiss-btn">Dismiss</button>
      </div>
    `);
    document.getElementById('dismiss-btn').addEventListener('click', hideOverlay);
    return;
  }
  if (map && map.getSource('clusters')) {
    map.getSource('clusters').setData(parsed);
  }
}

function exportCurrentView() {
  if (!map) return;
  const dataUrl = map.getCanvas().toDataURL('image/png');
  const link = document.createElement('a');
  link.href = dataUrl;
  link.download = `workplace-gis-export-${Date.now()}.png`;
  link.click();
}

async function bootstrap() {
  const hasConfig = await invoke('has_gis_config');
  if (!hasConfig) {
    renderSetupForm('https://gis.woodfinegroup.com');
    return;
  }

  const endpoint = await invoke('get_tile_endpoint');
  endpointLabel.textContent = endpoint;

  try {
    const layers = await invoke('get_available_layers');
    renderLayerToggles(layers);
    initMap(endpoint);
    hideOverlay();
  } catch (err) {
    renderError(err && err.message ? err.message : String(err));
  }
}

loadGeojsonBtn.addEventListener('click', loadGeojsonOverlay);
exportBtn.addEventListener('click', exportCurrentView);
changeEndpointBtn.addEventListener('click', async () => {
  const current = await invoke('get_tile_endpoint');
  renderSetupForm(current);
});

bootstrap();
