#!/usr/bin/env python3
"""
tool-pdf-interactive.py — Bencal Entities proforma interactive PDF binder.

Input:  Pre-rendered PDFs from inputs/Bencal_Proforma/ (project-documents archive).
Output: outputs/COMPLIANCE_MCorp_YYYY_MM_DD_Interactive_Bencal_Entities_V2.pdf

Slip-sheet navigation style matches the Bencal Offering Documentation binder:
  - Portrait letter nav page before each section (header, rule, item list)
  - Active section highlighted with left accent bar
  - Other sections clickable as blue links
  - Dark navy INDEX button stamped on every content page

Usage:
    cd ~/Foundry/clones/project-proforma
    python3 tool-pdf-interactive.py
"""
from __future__ import annotations

import io
import os
import sys
from datetime import date
from pathlib import Path

from pypdf import PdfReader, PdfWriter
from pypdf.generic import (
    ArrayObject, BooleanObject, DictionaryObject,
    FloatObject, NameObject, NullObject, NumberObject,
)
from reportlab.lib.pagesizes import letter
from reportlab.pdfgen import canvas

ARCHIVE_ROOT = Path(__file__).resolve().parent
SOURCE_DIR   = Path("/srv/foundry/clones/project-documents/inputs/Bencal_Proforma")
OUT_DIR      = ARCHIVE_ROOT / "outputs"

TODAY_ISO    = date.today().isoformat()
TODAY_FMT    = TODAY_ISO.replace("-", "_")
OUT_FILENAME = f"COMPLIANCE_MCorp_{TODAY_FMT}_Interactive_Bencal_Entities_V2.pdf"
DRAFT_LABEL  = f"DRAFT — {TODAY_ISO} — V2"

# ---------------------------------------------------------------------------
# Sections — source PDFs and display metadata
# ---------------------------------------------------------------------------
SECTIONS = [
    {
        "label":    "Bencal Management Corp.",
        "label2":   None,
        "subtitle": "Proforma V2 — IFRS 10.27 Investment Entity",
        "pdf":      "COMPLIANCE_MCorp_2026_06_05_Proforma_Bencal_Management_V2.pdf",
        "outline":  "Bencal Management Corp.",
    },
    {
        "label":    "Bencal Special Purpose 1 Inc.",
        "label2":   None,
        "subtitle": "Proforma V2 — IFRS 9 FVTPL",
        "pdf":      "COMPLIANCE_MCorp_2026_06_05_Proforma_Bencal_SPV1_V2.pdf",
        "outline":  "Bencal Special Purpose 1 Inc.",
    },
    {
        "label":    "Bencal Special Purpose 2 Inc. and",
        "label2":   "Bencal Special Purpose Limited Partnership",
        "subtitle": "Proforma V2 — IFRS 9 FVTPL (GP + LP)",
        "pdf":      "COMPLIANCE_MCorp_2026_06_05_Proforma_Bencal_SPV2_V2.pdf",
        "outline":  "Bencal Special Purpose 2 Inc. and Bencal Special Purpose Limited Partnership",
    },
    {
        "label":    "Commissions and Rebates",
        "label2":   None,
        "subtitle": "SPV Operating Budget — All Entities",
        "pdf":      "COMPLIANCE_MCorp_2026_05_27_Proforma_Bencal_Commissions_JW2.pdf",
        "outline":  "Commissions and Rebates",
    },
]

# ---------------------------------------------------------------------------
# Slip sheet layout — portrait letter (612 × 792 pt)
# Mirrors the NAV_LAYOUT geometry of the Offering Documentation binder.
#
#   y   = label baseline
#   h   = rect height (pt)
#   ro  = rect offset below label baseline  →  rect = [y-ro, y-ro+h]
# ---------------------------------------------------------------------------
NAV_LEFT  = 72
NAV_RIGHT = 540
NAV_WIDTH = NAV_RIGHT - NAV_LEFT

NAV_ITEMS = [
    {"idx": 0, "y": 644, "h": 34, "ro": 22},   # Bencal Management Corp.
    {"idx": 1, "y": 602, "h": 34, "ro": 22},   # Bencal Special Purpose 1 Inc.
    {"idx": 2, "y": 548, "h": 48, "ro": 22},   # Bencal SPV2 (two-line label)
    {"idx": 3, "y": 486, "h": 34, "ro": 22},   # Commissions and Rebates
]

# INDEX button dimensions (applied to content pages)
BTN_W      = 64
BTN_H      = 20
BTN_MARGIN = 24
# Slightly larger inset for rotated (landscape-displayed) pages so the button
# clears any landscape footer rule. Portrait (rotation 0/180) is unchanged.
BTN_MARGIN_ROTATED = 28

# ---------------------------------------------------------------------------
# Home-anchor hook (reserved-zone targeting)
# ---------------------------------------------------------------------------
# Convention: documents MAY embed an invisible registration marker that reserves
# a clean zone for the INDEX button, so the button never collides with a page
# border or footer rule. The HTML/CSS side renders an invisible element carrying
# the class `pdf-home-anchor`, positioned at the page's visual bottom-right.
# When such a marker is present the script lands the button on it; otherwise it
# falls back to geometric placement (rotation-aware — see button_geometry()).
HOME_ANCHOR_CLASS = "pdf-home-anchor"

# Invisible registration sentinel embedded by the CSS author as white-on-white
# text at the page's visual bottom-right. The literal run is U+2302 (HOUSE) +
# "PDFHOME" + U+2302. We match on the ASCII core because the U+2302 glyphs may
# extract as spaces or be dropped depending on the font's ToUnicode map.
HOME_ANCHOR_SENTINEL = "PDFHOME"

# pdfplumber gives reliable per-fragment bounding boxes; pypdf's visitor_text
# transformation matrix is unscaled text-space here (not composed with the CTM)
# and yields coordinates outside the mediabox, so it cannot be trusted for the
# bbox. Prefer pdfplumber when present; fall back to the pypdf visitor only as a
# best-effort last resort.
try:
    import pdfplumber  # type: ignore
    _HAVE_PDFPLUMBER = True
except Exception:  # pragma: no cover - pdfplumber optional
    pdfplumber = None  # type: ignore
    _HAVE_PDFPLUMBER = False


def _sentinel_bbox_pdfplumber(page):  # noqa: ANN001 - pypdf PageObject
    """Locate the sentinel bbox via pdfplumber.

    The pypdf ``page`` is serialized to a one-page in-memory PDF and re-opened
    with pdfplumber (which needs a file/stream, not a pypdf PageObject). Returns
    ``(bx0, by0, bx1, by1)`` in PDF points (origin bottom-left), or ``None``.
    pdfplumber reports word boxes with a top-left origin, so y is flipped about
    the page height.
    """
    writer = PdfWriter()
    writer.add_page(page)
    buf = io.BytesIO()
    writer.write(buf)
    buf.seek(0)

    with pdfplumber.open(buf) as pdf:  # type: ignore[union-attr]
        plpage = pdf.pages[0]
        ph = float(plpage.height)
        candidates = []
        # 1) Word-level match — the sentinel usually extracts as one word.
        for w in plpage.extract_words(use_text_flow=False):
            if HOME_ANCHOR_SENTINEL in w["text"]:
                candidates.append((float(w["x0"]), float(w["x1"]),
                                   float(w["top"]), float(w["bottom"])))
        if not candidates:
            # 2) Char-run fallback: scan the ASCII letters of the sentinel in
            #    reading order and assemble a run that spells PDFHOME.
            run = []
            target = HOME_ANCHOR_SENTINEL
            for ch in plpage.chars:
                t = ch.get("text", "")
                if len(run) < len(target) and t == target[len(run)]:
                    run.append(ch)
                    if len(run) == len(target):
                        x0 = min(float(c["x0"]) for c in run)
                        x1 = max(float(c["x1"]) for c in run)
                        top = min(float(c["top"]) for c in run)
                        bot = max(float(c["bottom"]) for c in run)
                        candidates.append((x0, x1, top, bot))
                        run = []
                elif t and target.startswith(t):
                    run = [ch]
                else:
                    run = []
        if not candidates:
            return None
        # If multiple, prefer the lowest (visually bottom-most) instance.
        bx0, bx1, top, bot = max(candidates, key=lambda c: c[3])
        by0 = ph - bot   # bbox bottom edge in bottom-left coordinates
        by1 = ph - top   # bbox top edge
        return (bx0, by0, bx1, by1)


def _sentinel_bbox_pypdf(page):  # noqa: ANN001 - pypdf PageObject
    """Best-effort sentinel bbox via the pypdf visitor (fallback only).

    The visitor's transformation matrix is text-space and may not be composed
    with the page CTM, so coordinates can fall outside the mediabox. Used only
    when pdfplumber is unavailable; the result is returned but may be unreliable.
    """
    frags = []

    def _visitor(text, cm, tm, font_dict, font_size):  # noqa: ANN001
        if text and HOME_ANCHOR_SENTINEL in text:
            x = float(tm[4])
            y = float(tm[5])
            try:
                fs = float(font_size)
            except (TypeError, ValueError):
                fs = 6.0
            # Crude width estimate; the visitor does not give a true box.
            w = 0.5 * fs * len(HOME_ANCHOR_SENTINEL)
            frags.append((x, y, x + w, y + fs))

    try:
        page.extract_text(visitor_text=_visitor)
    except Exception:
        return None
    if not frags:
        return None
    return frags[-1]


def find_home_anchor(page):  # noqa: ANN001 - pypdf PageObject
    """Locate the reserved-zone home anchor on *page* and return its button rect.

    The document embeds an invisible registration sentinel — the literal run
    ``⌂PDFHOME⌂`` (U+2302 + ``PDFHOME`` + U+2302) rendered white-on-white in a
    tiny font at the page's visual bottom-right (CSS class
    ``HOME_ANCHOR_CLASS``). This function extracts the sentinel's bounding box
    ``(bx0, by0, bx1, by1)`` in PDF points (origin bottom-left) and maps it to
    the INDEX/Home button rect per the CSS author's convention::

        button = (bx1 - BTN_W, by0 - BTN_H, bx1, by0)

    i.e. the marker bbox's bottom-right corner is the button's top-right corner,
    so the button sits in the clean reserved margin just below/left of the
    sentinel. Returns ``(x0, y0, x1, y1)`` suitable for both the overlay draw
    origin and the ``/Link`` annotation ``/Rect``, or ``None`` if the sentinel
    is not present on this page (callers then use the geometric fallback).

    Matching is on the ASCII core ``PDFHOME`` — the U+2302 glyphs may extract as
    spaces or be dropped. Bbox extraction prefers pdfplumber (reliable boxes);
    the pypdf visitor is a best-effort fallback when pdfplumber is absent.
    """
    bbox = None
    if _HAVE_PDFPLUMBER:
        try:
            bbox = _sentinel_bbox_pdfplumber(page)
        except Exception:
            bbox = None
    if bbox is None:
        bbox = _sentinel_bbox_pypdf(page)
    if bbox is None:
        return None

    bx0, by0, bx1, by1 = bbox
    # Marker bbox bottom-right corner == button top-right corner.
    return (bx1 - BTN_W, by0 - BTN_H, bx1, by0)


# ---------------------------------------------------------------------------
# Rotation-aware button geometry
# ---------------------------------------------------------------------------

def _page_rotation(pg) -> int:  # noqa: ANN001 - pypdf PageObject
    """Return the page's clockwise rotation normalized to {0, 90, 180, 270}."""
    rot = None
    # pypdf exposes a convenience property; fall back to the raw /Rotate entry.
    try:
        rot = pg.rotation  # type: ignore[attr-defined]
    except Exception:
        rot = None
    if rot is None:
        rot = pg.get("/Rotate", 0)
    try:
        rot = int(rot)
    except (TypeError, ValueError):
        rot = 0
    return rot % 360


def effective_size(mb_w: float, mb_h: float, rotation: int):
    """Return the VISUAL (displayed) (width, height) for a page.

    Swaps width/height when the page is displayed rotated 90° or 270°.
    """
    return (mb_h, mb_w) if rotation % 360 in (90, 270) else (mb_w, mb_h)


def button_geometry(mb_w: float, mb_h: float, rotation: int):
    """Compute the INDEX button's unrotated bounding box for *rotation*.

    ``merge_page`` composes the overlay in the page's *unrotated* mediabox
    coordinate space, and the viewer then applies ``/Rotate`` for display. We
    want the button to appear upright at the VISUAL bottom-right in all four
    rotations. An upright button is BTN_W wide × BTN_H tall *in display space*;
    in unrotated space those extents swap for 90/270.

    Args:
        mb_w, mb_h:  mediabox width/height (unrotated page dimensions).
        rotation:    normalized page rotation in {0, 90, 180, 270}.

    Returns:
        ``(x0, y0, rect)`` where ``(x0, y0)`` is the unrotated lower-left of the
        button's bounding box (the overlay draw anchor) and ``rect`` is the
        full unrotated bounding box ``(x0, y0, x1, y1)`` used for the ``/Link``
        annotation ``/Rect``. The box is BTN_W×BTN_H for rotation 0/180 and
        BTN_H×BTN_W for rotation 90/270.
    """
    rotation = rotation % 360
    margin = BTN_MARGIN if rotation in (0, 180) else BTN_MARGIN_ROTATED

    if rotation == 0:
        # Visual bottom-right == unrotated bottom-right; box is BTN_W×BTN_H.
        x0 = mb_w - margin - BTN_W
        y0 = margin
        bw, bh = BTN_W, BTN_H
    elif rotation == 90:
        # Page rotated 90° CW for display. The visual bottom-right corner maps
        # to the unrotated top-right corner; upright button occupies BTN_H wide
        # × BTN_W tall in unrotated space.
        bw, bh = BTN_H, BTN_W
        x0 = mb_w - margin - bw
        y0 = mb_h - margin - bh
    elif rotation == 180:
        # Visual bottom-right == unrotated top-left; box is BTN_W×BTN_H.
        bw, bh = BTN_W, BTN_H
        x0 = margin
        y0 = mb_h - margin - bh
    else:  # rotation == 270
        # Page rotated 270° CW (== 90° CCW). Visual bottom-right maps to the
        # unrotated bottom-left corner; box is BTN_H wide × BTN_W tall.
        bw, bh = BTN_H, BTN_W
        x0 = margin
        y0 = margin

    rect = (x0, y0, x0 + bw, y0 + bh)
    return x0, y0, rect


# ---------------------------------------------------------------------------
# Drawing helpers
# ---------------------------------------------------------------------------

def draw_slip_sheet(c: canvas.Canvas, active_idx: int) -> None:
    """Render a navigation slip sheet onto canvas c for section active_idx."""

    # ── Header ──────────────────────────────────────────────────────────────
    c.setFont("Helvetica-Bold", 15)
    c.setFillColorRGB(0, 0, 0)
    c.drawString(NAV_LEFT, 730, "BENCAL ENTITIES — PROFORMA PACKAGE")

    c.setFont("Helvetica", 9.5)
    c.setFillColorRGB(0.3, 0.3, 0.3)
    c.drawString(
        NAV_LEFT, 714,
        f"Woodfine Management Corp.  |  Bencal SPV Programme  |  {DRAFT_LABEL}",
    )

    c.setLineWidth(1.0)
    c.setStrokeColorRGB(0, 0, 0)
    c.line(NAV_LEFT, 702, NAV_RIGHT, 702)

    # ── Section group label ──────────────────────────────────────────────────
    c.setFont("Helvetica-Bold", 8.5)
    c.setFillColorRGB(0.45, 0.45, 0.45)
    c.drawString(NAV_LEFT + 4, 672, "PROFORMA SECTIONS")
    c.setLineWidth(0.4)
    c.setStrokeColorRGB(0.72, 0.72, 0.72)
    c.line(NAV_LEFT + 4, 667, NAV_RIGHT, 667)

    # ── Nav items ────────────────────────────────────────────────────────────
    for item in NAV_ITEMS:
        i        = item["idx"]
        y        = item["y"]
        h        = item["h"]
        ro       = item["ro"]
        sec      = SECTIONS[i]
        is_active = (i == active_idx)
        rect_bot  = y - ro

        if is_active:
            # Highlighted row: light grey fill + dark navy left accent bar
            c.setFillColorRGB(0.95, 0.95, 0.95)
            c.rect(NAV_LEFT, rect_bot, NAV_WIDTH, h, fill=1, stroke=0)
            c.setFillColorRGB(0.0, 0.18, 0.39)
            c.rect(NAV_LEFT, rect_bot, 4, h, fill=1, stroke=0)
            title_rgb = (0.0, 0.0, 0.0)
            sub_rgb   = (0.25, 0.25, 0.25)
        else:
            # Inactive row: navy blue link text, no background
            title_rgb = (0.0, 0.18, 0.39)
            sub_rgb   = (0.40, 0.40, 0.50)

        tx = NAV_LEFT + 12
        c.setFillColorRGB(*title_rgb)
        c.setFont("Helvetica-Bold", 10)
        c.drawString(tx, y, sec["label"])

        if sec["label2"]:
            c.drawString(tx, y - 12, sec["label2"])
            sub_y = y - 26
        else:
            sub_y = y - 14

        if sec["subtitle"]:
            c.setFont("Helvetica", 9)
            c.setFillColorRGB(*sub_rgb)
            c.drawString(tx, sub_y, sec["subtitle"])

    # ── Footer ───────────────────────────────────────────────────────────────
    c.setFont("Helvetica-Oblique", 8.5)
    c.setFillColorRGB(0.40, 0.40, 0.40)
    c.drawString(
        NAV_LEFT, 72,
        "Interactive Index: Click a section title above to navigate directly to that section.",
    )


def make_index_overlay(page_w: float, page_h: float, rotation: int = 0) -> bytes:
    """Return a one-page PDF (unrotated mediabox page_w × page_h) with a dark
    navy INDEX button placed so it appears at the VISUAL bottom-right after the
    viewer applies *rotation*.

    The overlay is always drawn in the page's unrotated coordinate space (that
    is how ``merge_page`` composes it). For rotation 0/180 this is a plain
    bottom-right / top-left placement. For 90/270 we additionally rotate the
    canvas about the button's lower-left so the button glyph itself is upright
    in the displayed (rotated) frame — otherwise "INDEX" would read sideways.
    """
    rotation = rotation % 360
    x0, y0, _rect = button_geometry(page_w, page_h, rotation)

    buf = io.BytesIO()
    oc = canvas.Canvas(buf, pagesize=(page_w, page_h))
    oc.saveState()

    # Orient the drawing so the button reads upright after the page is rotated
    # for display. The page is rotated clockwise by `rotation`; counter-rotate
    # the button content by the same amount about its anchor so it ends upright.
    oc.translate(x0, y0)
    if rotation == 90:
        # After a 90° CW page rotation, draw the button rotated +90° here.
        oc.rotate(90)
        oc.translate(0, -BTN_H)
    elif rotation == 180:
        oc.rotate(180)
        oc.translate(-BTN_W, -BTN_H)
    elif rotation == 270:
        oc.rotate(270)
        oc.translate(-BTN_W, 0)

    # From here the local origin's (0,0)→(BTN_W,BTN_H) box draws the upright
    # button regardless of page rotation.
    oc.setFillColorRGB(0.0, 0.18, 0.39)
    oc.roundRect(0, 0, BTN_W, BTN_H, 4, fill=1, stroke=0)
    oc.setFillColorRGB(1, 1, 1)
    oc.setFont("Helvetica-Bold", 8)
    tw = oc.stringWidth("INDEX", "Helvetica-Bold", 8)
    oc.drawString((BTN_W - tw) / 2, 6, "INDEX")

    oc.restoreState()
    oc.save()
    buf.seek(0)
    return buf.read()


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main() -> None:
    print("=" * 62)
    print("BENCAL ENTITIES — INTERACTIVE PROFORMA BINDER  V2")
    print("=" * 62)

    # ── Step 1: Verify source PDFs ───────────────────────────────────────────
    print("\n[1/5] Verifying source PDFs...")
    pdf_paths: list[Path] = []
    for sec in SECTIONS:
        path = SOURCE_DIR / sec["pdf"]
        if not path.exists():
            print(f"  ERROR: {path} not found", file=sys.stderr)
            sys.exit(1)
        n = len(PdfReader(str(path)).pages)
        print(f"  [ok]  {sec['label'][:52]:52s} {n:3d} pp")
        pdf_paths.append(path)

    # ── Step 2: Compute page layout ──────────────────────────────────────────
    print("\n[2/5] Computing page layout...")
    page_counts = [len(PdfReader(str(p)).pages) for p in pdf_paths]
    slip_indices: list[int] = []
    cursor = 0
    for n in page_counts:
        slip_indices.append(cursor)
        cursor += 1 + n
    total_pages = cursor

    for i, sec in enumerate(SECTIONS):
        print(f"  p{slip_indices[i]:3d}  slip + {page_counts[i]:2d} content  →  {sec['label']}")
    print(f"  Total: {total_pages} pages")

    # ── Step 3: Assemble PDF ─────────────────────────────────────────────────
    print("\n[3/5] Assembling binder...")
    slip_temps: list[str] = []
    writer = PdfWriter()
    writer.add_metadata({
        "/Title":   "Bencal Entities — Proforma Package",
        "/Author":  "Woodfine Management Corp.",
        "/Subject": "Bencal SPV Programme | Proforma Package",
        "/Creator": "PointSav Digital Systems",
    })

    outline_parent = None
    for i, sec in enumerate(SECTIONS):
        # Generate slip sheet
        tmp = str(ARCHIVE_ROOT / f"_temp_slip_{i}.pdf")
        slip_temps.append(tmp)
        c = canvas.Canvas(tmp, pagesize=letter)
        draw_slip_sheet(c, active_idx=i)
        c.save()
        writer.add_page(PdfReader(tmp).pages[0])

        # Append content pages
        for pg in PdfReader(str(pdf_paths[i])).pages:
            writer.add_page(pg)

        # PDF outline bookmark
        if outline_parent is None:
            outline_parent = writer.add_outline_item(
                "Proforma Sections", slip_indices[0]
            )
        writer.add_outline_item(sec["outline"], slip_indices[i], parent=outline_parent)
        print(f"  [{i + 1}/{len(SECTIONS)}] {sec['label']}")

    # ── Step 4: Navigation links on slip sheets ──────────────────────────────
    print("\n[4/5] Binding navigation links on slip sheets...")
    for i in range(len(SECTIONS)):
        slip_pg = slip_indices[i]
        for item in NAV_ITEMS:
            j = item["idx"]
            if j == i:
                continue   # active — no self-link
            y    = item["y"]
            h    = item["h"]
            ro   = item["ro"]
            rect = (NAV_LEFT, y - ro, NAV_RIGHT, y - ro + h)

            dest_ref = writer.pages[slip_indices[j]].indirect_reference
            assert dest_ref is not None, f"slip page {j} has no indirect_reference"

            ann = DictionaryObject({
                NameObject("/Type"):    NameObject("/Annot"),
                NameObject("/Subtype"): NameObject("/Link"),
                NameObject("/Rect"):    ArrayObject([FloatObject(v) for v in rect]),
                NameObject("/Border"):  ArrayObject(
                    [NumberObject(0), NumberObject(0), NumberObject(0)]
                ),
                NameObject("/A"): DictionaryObject({
                    NameObject("/S"): NameObject("/GoTo"),
                    NameObject("/D"): ArrayObject([
                        dest_ref,
                        NameObject("/XYZ"),
                        NullObject(), NumberObject(792), NullObject(),
                    ]),
                }),
            })
            ann_ref = writer._add_object(ann)
            pg_obj = writer.pages[slip_pg]
            if "/Annots" not in pg_obj:
                pg_obj[NameObject("/Annots")] = ArrayObject()
            pg_obj[NameObject("/Annots")].append(ann_ref)

    # ── Step 5: INDEX button on content pages ────────────────────────────────
    print("\n[5/5] Stamping INDEX button on content pages...")
    slip_set = set(slip_indices)
    home_ref = writer.pages[0].indirect_reference
    assert home_ref is not None, "page 0 has no indirect_reference"

    # Cache overlays by (page size, rotation) — most pages share one key.
    overlay_cache: dict[tuple[float, float, int], bytes] = {}

    for p in range(len(writer.pages)):
        if p in slip_set:
            continue
        pg = writer.pages[p]
        mb = pg.mediabox
        pw, ph = float(mb.width), float(mb.height)
        rot = _page_rotation(pg)   # honour /Rotate so landscape lands correctly

        # Reserved-zone anchor takes priority over geometric placement. When a
        # document embeds the `pdf-home-anchor` sentinel (⌂PDFHOME⌂) this lands
        # the button in the reserved clean zone; otherwise find_home_anchor()
        # returns None and we fall through to the rotation-aware geometry.
        anchor_rect = find_home_anchor(pg)
        if anchor_rect is not None:
            btn_rect = tuple(float(v) for v in anchor_rect)
            # Anchor placement: draw a plain (unrotated) overlay at the rect.
            ax0, ay0 = btn_rect[0], btn_rect[1]
            key = ("anchor", round(pw, 2), round(ph, 2), round(ax0, 2), round(ay0, 2))
            if key not in overlay_cache:
                ob = io.BytesIO()
                oc = canvas.Canvas(ob, pagesize=(pw, ph))
                oc.setFillColorRGB(0.0, 0.18, 0.39)
                oc.roundRect(ax0, ay0, BTN_W, BTN_H, 4, fill=1, stroke=0)
                oc.setFillColorRGB(1, 1, 1)
                oc.setFont("Helvetica-Bold", 8)
                tw = oc.stringWidth("INDEX", "Helvetica-Bold", 8)
                oc.drawString(ax0 + (BTN_W - tw) / 2, ay0 + 6, "INDEX")
                oc.save()
                ob.seek(0)
                overlay_cache[key] = ob.read()
            overlay_pg = PdfReader(io.BytesIO(overlay_cache[key])).pages[0]
            pg.merge_page(overlay_pg)
        else:
            # Geometric fallback (rotation-aware — handles 0/90/180/270).
            _bx0, _by0, btn_rect = button_geometry(pw, ph, rot)
            key = (round(pw, 2), round(ph, 2), rot)
            if key not in overlay_cache:
                overlay_cache[key] = make_index_overlay(pw, ph, rot)
            overlay_pg = PdfReader(io.BytesIO(overlay_cache[key])).pages[0]
            pg.merge_page(overlay_pg)

        ann = DictionaryObject({
            NameObject("/Type"):    NameObject("/Annot"),
            NameObject("/Subtype"): NameObject("/Link"),
            NameObject("/Rect"):    ArrayObject([FloatObject(v) for v in btn_rect]),
            NameObject("/Border"):  ArrayObject(
                [NumberObject(0), NumberObject(0), NumberObject(0)]
            ),
            NameObject("/A"): DictionaryObject({
                NameObject("/S"): NameObject("/GoTo"),
                NameObject("/D"): ArrayObject([
                    home_ref,
                    NameObject("/XYZ"),
                    NullObject(), NumberObject(792), NullObject(),
                ]),
            }),
        })
        ann_ref = writer._add_object(ann)
        if "/Annots" not in pg:
            pg[NameObject("/Annots")] = ArrayObject()
        pg[NameObject("/Annots")].append(ann_ref)

    # ── Viewer preferences ────────────────────────────────────────────────────
    vp = DictionaryObject()
    vp.update({NameObject("/DisplayDocTitle"): BooleanObject(True)})
    writer.root_object.update({
        NameObject("/OpenAction"): ArrayObject([
            writer.pages[0].indirect_reference, NameObject("/Fit")
        ]),
        NameObject("/PageMode"):   NameObject("/UseOutlines"),
        NameObject("/PageLayout"): NameObject("/SinglePage"),
        NameObject("/ViewerPreferences"): vp,
    })

    # ── Write output ──────────────────────────────────────────────────────────
    out_path = OUT_DIR / OUT_FILENAME
    out_path.parent.mkdir(parents=True, exist_ok=True)
    with open(out_path, "wb") as f:
        writer.write(f)

    print(f"\n{'=' * 62}")
    print(f"SUCCESS: {OUT_FILENAME}")
    print(f"Path:    {out_path}")
    print(f"Pages:   {total_pages}")
    print(f"{'=' * 62}")


if __name__ == "__main__":
    try:
        main()
    finally:
        import glob
        for tmp in glob.glob(str(ARCHIVE_ROOT / "_temp_slip_*.pdf")):
            try:
                os.remove(tmp)
            except OSError:
                pass
