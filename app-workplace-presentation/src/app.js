// Workplace*Presentation — editor logic
// Copyright © 2026 PointSav Digital Systems
// Licensed under the European Union Public Licence v1.2 (EUPL-1.2)
//
// Pure-frontend implementation: no custom Tauri IPC commands exist yet
// (src-tauri/src/main.rs is still the unmodified default Builder — see NEXT.md).
// All file/dialog access goes through Tauri v1's built-in JS surface, exposed as
// window.__TAURI__ by tauri.conf.json's `build.withGlobalTauri: true`. There is no
// bundler in this project (tauri.conf.json's devPath/distDir both point at `../src`
// directly), so this file does not `import` from '@tauri-apps/api' — it reads the
// global at call time instead.

const CANVAS_W = 960;
const CANVAS_H = 540;

const DEFAULT_TEXT_BLOCK = { x: 60, y: 60, w: 420, h: 90 };
const DEFAULT_IMAGE_BLOCK = { x: 330, y: 170, w: 300, h: 200 };

/** Cheap, dependency-free unique id — avoids crypto.randomUUID(), which is not
 * guaranteed present in the older WebKitGTK shipped for macOS 10.13 targets. */
function genId() {
  return `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;
}

function blankPresentation() {
  return {
    schema: "workplace-presentation-v1",
    slides: [blankSlide()],
  };
}

function blankSlide() {
  return { id: genId(), blocks: [] };
}

function escapeHtml(s) {
  return String(s)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

function mimeFromPath(path) {
  const ext = (path.split(".").pop() || "").toLowerCase();
  const map = {
    png: "image/png",
    jpg: "image/jpeg",
    jpeg: "image/jpeg",
    gif: "image/gif",
    svg: "image/svg+xml",
    webp: "image/webp",
    bmp: "image/bmp",
  };
  return map[ext] || "application/octet-stream";
}

/** Uint8Array -> base64, chunked to stay under call-stack / arg-count limits
 * that String.fromCharCode.apply hits on large typed arrays. */
function bytesToBase64(bytes) {
  let binary = "";
  const chunkSize = 0x8000;
  for (let i = 0; i < bytes.length; i += chunkSize) {
    const chunk = bytes.subarray(i, i + chunkSize);
    binary += String.fromCharCode.apply(null, chunk);
  }
  return btoa(binary);
}

function tauri() {
  return window.__TAURI__ || null;
}

function requireTauri() {
  const t = tauri();
  if (!t) {
    showToast("This action requires the Tauri desktop shell (not available in a plain browser).");
    return null;
  }
  return t;
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

const state = {
  filePath: null,
  presentation: blankPresentation(),
  selectedSlideId: null,
  selectedBlockId: null,
  drag: null, // { blockId, startX, startY, origX, origY, pointerStartX, pointerStartY }
  dirty: false, // unsaved changes since last open/save/new?
};
state.selectedSlideId = state.presentation.slides[0].id;

function currentSlide() {
  return state.presentation.slides.find((s) => s.id === state.selectedSlideId) || null;
}

function selectedBlock() {
  const slide = currentSlide();
  if (!slide) return null;
  return slide.blocks.find((b) => b.id === state.selectedBlockId) || null;
}

// ---------------------------------------------------------------------------
// Dirty-state tracking (unsaved-changes guard)
// ---------------------------------------------------------------------------

function markDirty() {
  state.dirty = true;
}

function markClean() {
  state.dirty = false;
}

// ---------------------------------------------------------------------------
// Slide operations (c)
// ---------------------------------------------------------------------------

function addSlide() {
  const slide = blankSlide();
  const idx = state.presentation.slides.findIndex((s) => s.id === state.selectedSlideId);
  const insertAt = idx === -1 ? state.presentation.slides.length : idx + 1;
  state.presentation.slides.splice(insertAt, 0, slide);
  state.selectedSlideId = slide.id;
  state.selectedBlockId = null;
  markDirty();
  render();
}

function deleteSlide(slideId) {
  if (state.presentation.slides.length <= 1) {
    showToast("A presentation needs at least one slide.");
    return;
  }
  const idx = state.presentation.slides.findIndex((s) => s.id === slideId);
  if (idx === -1) return;
  state.presentation.slides.splice(idx, 1);
  if (state.selectedSlideId === slideId) {
    const next = state.presentation.slides[Math.max(0, idx - 1)];
    state.selectedSlideId = next.id;
    state.selectedBlockId = null;
  }
  markDirty();
  render();
}

function moveSlide(slideId, direction) {
  const slides = state.presentation.slides;
  const idx = slides.findIndex((s) => s.id === slideId);
  const target = idx + direction;
  if (idx === -1 || target < 0 || target >= slides.length) return;
  const [slide] = slides.splice(idx, 1);
  slides.splice(target, 0, slide);
  markDirty();
  render();
}

function selectSlide(slideId) {
  state.selectedSlideId = slideId;
  state.selectedBlockId = null;
  render();
}

// ---------------------------------------------------------------------------
// Block operations (d, e)
// ---------------------------------------------------------------------------

function addTextBlock() {
  const slide = currentSlide();
  if (!slide) return;
  const block = {
    id: genId(),
    type: "text",
    ...DEFAULT_TEXT_BLOCK,
    content: "New text",
    bold: false,
    italic: false,
    size: 24,
  };
  slide.blocks.push(block);
  state.selectedBlockId = block.id;
  markDirty();
  render();
}

async function addImageBlock() {
  const t = requireTauri();
  if (!t) return;
  const slide = currentSlide();
  if (!slide) return;
  let path;
  try {
    path = await t.dialog.open({
      multiple: false,
      filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "gif", "svg", "webp", "bmp"] }],
    });
  } catch (e) {
    showToast(`Could not open file dialog: ${e}`);
    return;
  }
  if (!path || Array.isArray(path)) return; // cancelled, or multi-select (disabled above)

  let bytes;
  try {
    bytes = await t.fs.readFile(path); // v2: fs.readBinaryFile was renamed to fs.readFile (returns Uint8Array)
  } catch (e) {
    showToast(`Could not read image: ${e}`);
    return;
  }
  const dataUrl = `data:${mimeFromPath(path)};base64,${bytesToBase64(bytes)}`;
  const block = {
    id: genId(),
    type: "image",
    ...DEFAULT_IMAGE_BLOCK,
    dataUrl,
    sourceName: path.split(/[\\/]/).pop(),
  };
  slide.blocks.push(block);
  state.selectedBlockId = block.id;
  markDirty();
  render();
}

function deleteBlock(blockId) {
  const slide = currentSlide();
  if (!slide) return;
  const idx = slide.blocks.findIndex((b) => b.id === blockId);
  if (idx === -1) return;
  slide.blocks.splice(idx, 1);
  if (state.selectedBlockId === blockId) state.selectedBlockId = null;
  markDirty();
  render();
}

function selectBlock(blockId) {
  state.selectedBlockId = blockId;
  render();
}

function updateBlock(blockId, patch) {
  const slide = currentSlide();
  if (!slide) return;
  const block = slide.blocks.find((b) => b.id === blockId);
  if (!block) return;
  Object.assign(block, patch);
  markDirty();
  render();
}

// ---------------------------------------------------------------------------
// Drag-to-move (block canvas positioning — part of the slide canvas design, b)
// ---------------------------------------------------------------------------

function startDrag(e, block) {
  if (e.button !== 0) return;
  e.preventDefault();
  e.stopPropagation();
  selectBlock(block.id);
  state.drag = {
    blockId: block.id,
    origX: block.x,
    origY: block.y,
    pointerStartX: e.clientX,
    pointerStartY: e.clientY,
  };
  window.addEventListener("mousemove", onDragMove);
  window.addEventListener("mouseup", onDragEnd);
}

function onDragMove(e) {
  if (!state.drag) return;
  const slide = currentSlide();
  if (!slide) return;
  const block = slide.blocks.find((b) => b.id === state.drag.blockId);
  if (!block) return;
  const dx = e.clientX - state.drag.pointerStartX;
  const dy = e.clientY - state.drag.pointerStartY;
  block.x = Math.max(0, Math.min(CANVAS_W - block.w, state.drag.origX + dx));
  block.y = Math.max(0, Math.min(CANVAS_H - block.h, state.drag.origY + dy));
  renderCanvasOnly();
}

function onDragEnd() {
  state.drag = null;
  window.removeEventListener("mousemove", onDragMove);
  window.removeEventListener("mouseup", onDragEnd);
  markDirty();
  render();
}

// ---------------------------------------------------------------------------
// Open / Save (a)
// ---------------------------------------------------------------------------

function isValidPresentation(obj) {
  return (
    !!obj &&
    Array.isArray(obj.slides) &&
    obj.slides.length > 0 &&
    obj.slides.every(
      (s) => !!s && typeof s.id === "string" && Array.isArray(s.blocks)
    )
  );
}

async function newPresentation() {
  if (state.dirty && !window.confirm("Start a new presentation? Unsaved changes will be lost.")) return;
  state.presentation = blankPresentation();
  state.filePath = null;
  state.selectedSlideId = state.presentation.slides[0].id;
  state.selectedBlockId = null;
  markClean();
  render();
}

async function openPresentation() {
  const t = requireTauri();
  if (!t) return;
  let path;
  try {
    path = await t.dialog.open({
      multiple: false,
      filters: [{ name: "Presentation", extensions: ["json"] }],
    });
  } catch (e) {
    showToast(`Could not open file dialog: ${e}`);
    return;
  }
  if (!path || Array.isArray(path)) return;
  let text;
  try {
    text = await t.fs.readTextFile(path);
  } catch (e) {
    showToast(`Could not read file: ${e}`);
    return;
  }
  let parsed;
  try {
    parsed = JSON.parse(text);
  } catch (e) {
    showToast("That file is not valid presentation JSON.");
    return;
  }
  if (!isValidPresentation(parsed)) {
    showToast("That file does not look like a workplace-presentation document.");
    return;
  }
  state.presentation = parsed;
  state.filePath = path;
  state.selectedSlideId = parsed.slides[0].id;
  state.selectedBlockId = null;
  markClean();
  render();
  showToast(`Opened ${path.split(/[\\/]/).pop()}`);
}

async function savePresentation() {
  if (state.filePath) {
    await writePresentationTo(state.filePath);
  } else {
    await savePresentationAs();
  }
}

async function savePresentationAs() {
  const t = requireTauri();
  if (!t) return;
  let path;
  try {
    path = await t.dialog.save({
      defaultPath: "presentation.json",
      filters: [{ name: "Presentation", extensions: ["json"] }],
    });
  } catch (e) {
    showToast(`Could not open save dialog: ${e}`);
    return;
  }
  if (!path) return;
  await writePresentationTo(path);
}

async function writePresentationTo(path) {
  const t = requireTauri();
  if (!t) return;
  try {
    await t.fs.writeTextFile(path, JSON.stringify(state.presentation, null, 2));
  } catch (e) {
    showToast(`Could not save file: ${e}`);
    return;
  }
  state.filePath = path;
  markClean();
  render();
  showToast(`Saved ${path.split(/[\\/]/).pop()}`);
}

// ---------------------------------------------------------------------------
// Export to self-contained HTML (f)
// ---------------------------------------------------------------------------

function blockToHtml(block) {
  const style = `left:${block.x}px;top:${block.y}px;width:${block.w}px;height:${block.h}px;`;
  if (block.type === "image") {
    return `<div class="block image" style="${style}"><img src="${block.dataUrl}" alt="" /></div>`;
  }
  const textStyle =
    style +
    `font-weight:${block.bold ? "bold" : "normal"};` +
    `font-style:${block.italic ? "italic" : "normal"};` +
    `font-size:${block.size || 24}px;`;
  const content = escapeHtml(block.content || "").replace(/\n/g, "<br>");
  return `<div class="block text" style="${textStyle}">${content}</div>`;
}

function buildStandaloneHtml(presentation) {
  const slidesHtml = presentation.slides
    .map((slide, i) => {
      const blocks = slide.blocks.map(blockToHtml).join("\n      ");
      return `    <section class="slide"${i === 0 ? ' data-current="true"' : ""}>\n      ${blocks}\n    </section>`;
    })
    .join("\n");

  return `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8" />
<meta name="viewport" content="width=device-width, initial-scale=1.0" />
<title>Presentation</title>
<style>
  html, body {
    margin: 0;
    height: 100%;
    background: #11111b;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  }
  .deck {
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .slide {
    position: relative;
    width: 960px;
    height: 540px;
    background: #ffffff;
    color: #111;
    box-shadow: 0 6px 28px rgba(0,0,0,0.45);
    display: none;
    flex-shrink: 0;
  }
  .slide.current { display: block; }
  .block { position: absolute; overflow: hidden; white-space: pre-wrap; word-break: break-word; }
  .block.image img { width: 100%; height: 100%; object-fit: contain; display: block; }
  .nav {
    position: fixed;
    bottom: 16px;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    gap: 8px;
    font-family: inherit;
  }
  .nav button {
    padding: 6px 14px;
    border-radius: 6px;
    border: 1px solid #45475a;
    background: #313244;
    color: #cdd6f4;
    cursor: pointer;
  }
  .nav .count { color: #a6adc8; font-size: 0.85rem; align-self: center; }
  @media print {
    body { background: #fff; }
    .deck { display: block; height: auto; }
    .nav { display: none; }
    .slide {
      display: block !important;
      box-shadow: none;
      page-break-after: always;
      margin: 0 auto;
    }
  }
</style>
</head>
<body>
  <div class="deck" id="deck">
${slidesHtml}
  </div>
  <div class="nav">
    <button id="prevBtn" type="button">&larr; Prev</button>
    <span class="count" id="count"></span>
    <button id="nextBtn" type="button">Next &rarr;</button>
  </div>
  <script>
    (function () {
      var slides = Array.prototype.slice.call(document.querySelectorAll('.slide'));
      var idx = 0;
      function show(i) {
        idx = Math.max(0, Math.min(slides.length - 1, i));
        slides.forEach(function (s, n) { s.classList.toggle('current', n === idx); });
        document.getElementById('count').textContent = (idx + 1) + ' / ' + slides.length;
      }
      document.getElementById('prevBtn').addEventListener('click', function () { show(idx - 1); });
      document.getElementById('nextBtn').addEventListener('click', function () { show(idx + 1); });
      document.addEventListener('keydown', function (e) {
        if (e.key === 'ArrowRight' || e.key === ' ') show(idx + 1);
        if (e.key === 'ArrowLeft') show(idx - 1);
      });
      show(0);
    })();
  <\/script>
</body>
</html>
`;
}

async function exportHtml() {
  const t = requireTauri();
  if (!t) return;
  let path;
  try {
    path = await t.dialog.save({
      defaultPath: "presentation.html",
      filters: [{ name: "HTML", extensions: ["html"] }],
    });
  } catch (e) {
    showToast(`Could not open save dialog: ${e}`);
    return;
  }
  if (!path) return;
  const html = buildStandaloneHtml(state.presentation);
  try {
    await t.fs.writeTextFile(path, html);
  } catch (e) {
    showToast(`Could not write export: ${e}`);
    return;
  }
  showToast(`Exported ${path.split(/[\\/]/).pop()}`);
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

let toastTimer = null;
function showToast(message) {
  const el = document.getElementById("toast");
  if (!el) return;
  el.textContent = message;
  el.classList.add("visible");
  if (toastTimer) clearTimeout(toastTimer);
  toastTimer = setTimeout(() => el.classList.remove("visible"), 2600);
}

function renderSidebar() {
  const list = document.getElementById("slideList");
  list.innerHTML = "";
  state.presentation.slides.forEach((slide, i) => {
    const wrap = document.createElement("div");

    const thumb = document.createElement("div");
    thumb.className = "slide-thumb" + (slide.id === state.selectedSlideId ? " selected" : "");
    thumb.addEventListener("click", () => selectSlide(slide.id));
    const firstText = slide.blocks.find((b) => b.type === "text");
    thumb.textContent = firstText ? firstText.content.slice(0, 60) : "";
    const num = document.createElement("span");
    num.className = "thumb-num";
    num.textContent = String(i + 1);
    thumb.appendChild(num);
    wrap.appendChild(thumb);

    const controls = document.createElement("div");
    controls.className = "slide-controls";
    const up = document.createElement("button");
    up.className = "small";
    up.textContent = "↑";
    up.title = "Move up";
    up.disabled = i === 0;
    up.addEventListener("click", () => moveSlide(slide.id, -1));
    const down = document.createElement("button");
    down.className = "small";
    down.textContent = "↓";
    down.title = "Move down";
    down.disabled = i === state.presentation.slides.length - 1;
    down.addEventListener("click", () => moveSlide(slide.id, 1));
    const del = document.createElement("button");
    del.className = "small danger";
    del.textContent = "×";
    del.title = "Delete slide";
    del.addEventListener("click", () => deleteSlide(slide.id));
    controls.appendChild(up);
    controls.appendChild(down);
    controls.appendChild(del);
    wrap.appendChild(controls);

    list.appendChild(wrap);
  });
}

function renderCanvasOnly() {
  const canvas = document.getElementById("slideCanvas");
  canvas.innerHTML = "";
  const slide = currentSlide();
  if (!slide) {
    const empty = document.createElement("div");
    empty.className = "canvas-empty";
    empty.textContent = "No slide selected.";
    canvas.appendChild(empty);
    return;
  }
  slide.blocks.forEach((block) => {
    const el = document.createElement("div");
    el.className = `block ${block.type}` + (block.id === state.selectedBlockId ? " selected" : "");
    el.style.left = `${block.x}px`;
    el.style.top = `${block.y}px`;
    el.style.width = `${block.w}px`;
    el.style.height = `${block.h}px`;
    if (block.type === "text") {
      el.style.fontWeight = block.bold ? "bold" : "normal";
      el.style.fontStyle = block.italic ? "italic" : "normal";
      el.style.fontSize = `${block.size || 24}px`;
      el.textContent = block.content;
    } else if (block.type === "image") {
      const img = document.createElement("img");
      img.src = block.dataUrl;
      img.alt = block.sourceName || "";
      el.appendChild(img);
    }
    el.addEventListener("mousedown", (e) => startDrag(e, block));
    el.addEventListener("click", (e) => {
      e.stopPropagation();
      selectBlock(block.id);
    });
    canvas.appendChild(el);
  });
}

function renderProperties() {
  const panel = document.getElementById("propertiesBody");
  panel.innerHTML = "";
  const block = selectedBlock();
  if (!block) {
    const hint = document.createElement("p");
    hint.className = "empty-hint";
    hint.textContent = "Select a block on the canvas to edit its properties.";
    panel.appendChild(hint);
    return;
  }

  if (block.type === "text") {
    const contentField = document.createElement("div");
    contentField.className = "field";
    const label = document.createElement("label");
    label.textContent = "Text";
    const textarea = document.createElement("textarea");
    textarea.value = block.content;
    textarea.addEventListener("input", () => updateBlock(block.id, { content: textarea.value }));
    contentField.appendChild(label);
    contentField.appendChild(textarea);
    panel.appendChild(contentField);

    const checksField = document.createElement("div");
    checksField.className = "field checks";
    const boldLabel = document.createElement("label");
    const boldInput = document.createElement("input");
    boldInput.type = "checkbox";
    boldInput.checked = !!block.bold;
    boldInput.addEventListener("change", () => updateBlock(block.id, { bold: boldInput.checked }));
    boldLabel.appendChild(boldInput);
    boldLabel.appendChild(document.createTextNode("Bold"));
    const italicLabel = document.createElement("label");
    const italicInput = document.createElement("input");
    italicInput.type = "checkbox";
    italicInput.checked = !!block.italic;
    italicInput.addEventListener("change", () => updateBlock(block.id, { italic: italicInput.checked }));
    italicLabel.appendChild(italicInput);
    italicLabel.appendChild(document.createTextNode("Italic"));
    checksField.appendChild(boldLabel);
    checksField.appendChild(italicLabel);
    panel.appendChild(checksField);

    const sizeField = document.createElement("div");
    sizeField.className = "field";
    const sizeLabel = document.createElement("label");
    sizeLabel.textContent = "Font size (px)";
    const sizeInput = document.createElement("input");
    sizeInput.type = "number";
    sizeInput.min = "8";
    sizeInput.max = "200";
    sizeInput.value = block.size || 24;
    sizeInput.addEventListener("input", () => {
      const v = parseInt(sizeInput.value, 10);
      if (!Number.isNaN(v) && v > 0) updateBlock(block.id, { size: v });
    });
    sizeField.appendChild(sizeLabel);
    sizeField.appendChild(sizeInput);
    panel.appendChild(sizeField);
  } else if (block.type === "image") {
    const previewField = document.createElement("div");
    previewField.className = "field";
    const label = document.createElement("label");
    label.textContent = block.sourceName || "Image";
    const preview = document.createElement("img");
    preview.src = block.dataUrl;
    preview.style.maxWidth = "100%";
    preview.style.borderRadius = "6px";
    previewField.appendChild(label);
    previewField.appendChild(preview);
    panel.appendChild(previewField);
  }

  // Shared position/size fields for any block type.
  const dimsField = document.createElement("div");
  dimsField.className = "field two-col";
  [
    ["x", "X"],
    ["y", "Y"],
  ].forEach(([key, text]) => {
    const div = document.createElement("div");
    const label = document.createElement("label");
    label.textContent = text;
    const input = document.createElement("input");
    input.type = "number";
    input.value = Math.round(block[key]);
    input.addEventListener("input", () => {
      const v = parseInt(input.value, 10);
      if (!Number.isNaN(v)) updateBlock(block.id, { [key]: v });
    });
    div.appendChild(label);
    div.appendChild(input);
    dimsField.appendChild(div);
  });
  panel.appendChild(dimsField);

  const sizeFieldWH = document.createElement("div");
  sizeFieldWH.className = "field two-col";
  [
    ["w", "Width"],
    ["h", "Height"],
  ].forEach(([key, text]) => {
    const div = document.createElement("div");
    const label = document.createElement("label");
    label.textContent = text;
    const input = document.createElement("input");
    input.type = "number";
    input.min = "10";
    input.value = Math.round(block[key]);
    input.addEventListener("input", () => {
      const v = parseInt(input.value, 10);
      if (!Number.isNaN(v) && v > 0) updateBlock(block.id, { [key]: v });
    });
    div.appendChild(label);
    div.appendChild(input);
    sizeFieldWH.appendChild(div);
  });
  panel.appendChild(sizeFieldWH);

  const deleteBtn = document.createElement("button");
  deleteBtn.className = "danger";
  deleteBtn.textContent = "Delete block";
  deleteBtn.addEventListener("click", () => deleteBlock(block.id));
  panel.appendChild(deleteBtn);
}

function renderToolbarState() {
  const filenameEl = document.getElementById("filename");
  const baseName = state.filePath ? state.filePath.split(/[\\/]/).pop() : "Untitled presentation";
  filenameEl.textContent = state.dirty ? `${baseName} • Unsaved changes` : baseName;
  document.getElementById("deleteSlideBtn").disabled = state.presentation.slides.length <= 1;
}

function render() {
  renderSidebar();
  renderCanvasOnly();
  renderProperties();
  renderToolbarState();
}

// ---------------------------------------------------------------------------
// Keyboard slide navigation
// ---------------------------------------------------------------------------

function navigateSlide(direction) {
  const slides = state.presentation.slides;
  const idx = slides.findIndex((s) => s.id === state.selectedSlideId);
  const target = idx + direction;
  if (idx === -1 || target < 0 || target >= slides.length) return;
  selectSlide(slides[target].id);
}

/** True if the event target is a form control already consuming arrow/Escape
 * keys for its own editing (text field, number field, properties textarea). */
function isEditableTarget(el) {
  if (!el) return false;
  const tag = el.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || !!el.isContentEditable;
}

function handleKeydown(e) {
  if (isEditableTarget(e.target)) return;

  if (e.key === "ArrowRight" || e.key === "ArrowDown") {
    e.preventDefault();
    navigateSlide(1);
  } else if (e.key === "ArrowLeft" || e.key === "ArrowUp") {
    e.preventDefault();
    navigateSlide(-1);
  } else if (e.key === "Escape" && state.selectedBlockId !== null) {
    e.preventDefault();
    state.selectedBlockId = null;
    render();
  }
}

// ---------------------------------------------------------------------------
// Wire up
// ---------------------------------------------------------------------------

function init() {
  document.getElementById("newBtn").addEventListener("click", newPresentation);
  document.getElementById("openBtn").addEventListener("click", openPresentation);
  document.getElementById("saveBtn").addEventListener("click", savePresentation);
  document.getElementById("saveAsBtn").addEventListener("click", savePresentationAs);
  document.getElementById("exportBtn").addEventListener("click", exportHtml);
  document.getElementById("addSlideBtn").addEventListener("click", addSlide);
  document.getElementById("deleteSlideBtn").addEventListener("click", () => deleteSlide(state.selectedSlideId));
  document.getElementById("addTextBtn").addEventListener("click", addTextBlock);
  document.getElementById("addImageBtn").addEventListener("click", addImageBlock);
  document.getElementById("slideCanvas").addEventListener("click", () => {
    state.selectedBlockId = null;
    render();
  });

  document.addEventListener("keydown", handleKeydown);
  window.addEventListener("beforeunload", (e) => {
    if (state.dirty) {
      e.preventDefault();
      e.returnValue = "";
    }
  });

  if (!tauri()) {
    showToast("Running outside the Tauri shell — open/save/export/image-insert are disabled.");
  }

  render();
}

document.addEventListener("DOMContentLoaded", init);
