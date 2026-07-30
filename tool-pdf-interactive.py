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


def make_index_overlay(page_w: float, page_h: float) -> bytes:
    """Return a one-page PDF with a dark navy INDEX button at bottom-right."""
    btn_x = page_w - BTN_MARGIN - BTN_W
    btn_y = BTN_MARGIN
    buf = io.BytesIO()
    oc = canvas.Canvas(buf, pagesize=(page_w, page_h))
    oc.setFillColorRGB(0.0, 0.18, 0.39)
    oc.roundRect(btn_x, btn_y, BTN_W, BTN_H, 4, fill=1, stroke=0)
    oc.setFillColorRGB(1, 1, 1)
    oc.setFont("Helvetica-Bold", 8)
    tw = oc.stringWidth("INDEX", "Helvetica-Bold", 8)
    oc.drawString(btn_x + (BTN_W - tw) / 2, btn_y + 6, "INDEX")
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

    # Cache overlays by page size (most pages share one size)
    overlay_cache: dict[tuple[float, float], bytes] = {}

    for p in range(len(writer.pages)):
        if p in slip_set:
            continue
        pg = writer.pages[p]
        mb = pg.mediabox
        pw, ph = float(mb.width), float(mb.height)

        key = (pw, ph)
        if key not in overlay_cache:
            overlay_cache[key] = make_index_overlay(pw, ph)
        overlay_pg = PdfReader(io.BytesIO(overlay_cache[key])).pages[0]
        pg.merge_page(overlay_pg)

        btn_x0 = pw - BTN_MARGIN - BTN_W
        btn_y0 = BTN_MARGIN
        btn_rect = (btn_x0, btn_y0, btn_x0 + BTN_W, btn_y0 + BTN_H)

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
