import asyncio
import csv
import os
from concurrent.futures import ThreadPoolExecutor
from typing import Any
from fastapi import FastAPI, Request
from gliner import GLiNER
import uvicorn

app = FastAPI()

WEIGHTS_DIR = os.environ.get("GLINER_WEIGHTS_DIR", "/var/lib/local-gliner/weights")
MODEL_NAME = os.environ.get("GLINER_MODEL", "urchade/gliner_medium-v2.1")

# Load model once at startup; cache to WEIGHTS_DIR to avoid re-download
os.environ["TRANSFORMERS_CACHE"] = WEIGHTS_DIR
model = GLiNER.from_pretrained(MODEL_NAME)
model.eval()

# Thread pool for synchronous GLiNER inference.
# DeBERTa releases the Python GIL during C++ forward pass, so four threads
# give real parallelism with a single model copy (~769 MB) in memory.
# Do NOT use uvicorn --workers N: that forks N processes, each loading the
# full model (769 MB × N).
_pool = ThreadPoolExecutor(max_workers=4)

DEFAULT_DOMAIN = "projects"

# Same ontology dir as service-content (SERVICE_CONTENT_ONTOLOGY_DIR) — both
# services read entity_types.csv as the single source of truth for entity
# type labels, per the COA-driven entity type direction (operator, 2026-06-28).
ONTOLOGY_DIR = os.environ.get(
    "GLINER_ONTOLOGY_DIR",
    "/srv/foundry/clones/project-totebox/service-content/ontology",
)

_DOMAIN_COLUMNS = {
    "projects": "description_projects",
    "corporate": "description_corporate",
    "documentation": "description_documentation",
}

# Fallback label descriptions — used only if entity_types.csv is missing or
# malformed, so a misconfigured ontology dir degrades rather than crashes.
# Adding a new entity type going forward = update entity_types.csv only;
# this table should not need further edits.
_FALLBACK_DOMAIN_LABELS: dict[str, dict[str, str]] = {
    "projects": {
        "Person":   "a named human individual — executive, broker, developer, or professional",
        "Company":  "a named company, fund, REIT, or investment firm",
        "Project":  "a named real estate development, building, property, or investment fund",
        "Location": "a named city, address, district, or country",
        "Account":  "a named financial account, lease, or contract",
    },
    "corporate": {
        "Person":   "a named human individual",
        "Company":  "a named company or organisation",
        "Project":  "a named business initiative, building, or fund",
        "Location": "a named city or country",
        "Account":  "a named financial account or contract",
    },
    "documentation": {
        "Person":   "a named developer, engineer, or contributor",
        "Company":  "a named company or technology organisation",
        "Project":  "a named software project, service, crate, or library",
        "Account":  "a named running service, system account, or API endpoint",
        "Location": "a named server, deployment environment, or infrastructure location",
    },
}


def _load_domain_labels() -> dict[str, dict[str, str]]:
    """Load entity type labels from ontology/entity_types.csv.

    Falls back to _FALLBACK_DOMAIN_LABELS if the CSV is missing, unreadable,
    or doesn't cover every domain — a misconfigured deployment should degrade
    to the known-good table rather than crash at startup.
    """
    csv_path = os.path.join(ONTOLOGY_DIR, "entity_types.csv")
    domains: dict[str, dict[str, str]] = {d: {} for d in _DOMAIN_COLUMNS}
    try:
        with open(csv_path, newline="", encoding="utf-8") as f:
            reader = csv.DictReader(f)
            for row in reader:
                label = (row.get("label") or "").strip()
                if not label:
                    continue
                for domain_id, column in _DOMAIN_COLUMNS.items():
                    desc = (row.get(column) or "").strip()
                    if desc:
                        domains[domain_id][label] = desc
        if all(domains[d] for d in _DOMAIN_COLUMNS):
            print(f"[GLINER] loaded entity type labels from {csv_path}")
            return domains
        print(
            f"[GLINER] {csv_path} missing rows for one or more domains; using fallback labels"
        )
    except (OSError, csv.Error, KeyError) as e:
        print(f"[GLINER] failed to load {csv_path}: {e}; using fallback labels")
    return _FALLBACK_DOMAIN_LABELS


# Domain-specific label descriptions — plain English, GLiNER reads these literally.
# Concrete examples in descriptions act as KoGNER-style entity hints.
DOMAIN_LABELS: dict[str, dict[str, str]] = _load_domain_labels()


def _labels_with_hints(
    domain_id: str, entity_hints: dict[str, list[str]] | None
) -> tuple[list[str], dict[str, str]]:
    """Build the GLiNER label list for a domain, appending KoGNER-style concrete
    entity-name examples to each label's description when hints are available.
    Concrete examples in descriptions act as free quality improvements — GLiNER
    reads them literally, no model change required.

    Returns (labels, desc_to_key); desc_to_key maps the (possibly hint-augmented)
    description string back to its canonical classification key, since GLiNER
    returns the description text as the predicted "label".
    """
    label_map = DOMAIN_LABELS.get(domain_id, DOMAIN_LABELS[DEFAULT_DOMAIN])
    augmented: dict[str, str] = {}
    for key, desc in label_map.items():
        hints = (entity_hints or {}).get(key)
        if hints:
            augmented[key] = f"{desc} (examples: {', '.join(hints)})"
        else:
            augmented[key] = desc
    labels = list(augmented.values())
    desc_to_key = {v: k for k, v in augmented.items()}
    return labels, desc_to_key


def _sync_predict(
    text: str, domain_id: str, entity_hints: dict[str, list[str]] | None = None
) -> list[dict[str, str]]:
    """Blocking GLiNER call — runs in _pool thread, not the event loop."""
    labels, desc_to_key = _labels_with_hints(domain_id, entity_hints)
    raw = model.predict_entities(text, labels, threshold=0.5)
    return [
        {
            "entity_name": e["text"],
            "classification": desc_to_key.get(e["label"], e["label"]),
        }
        for e in raw
    ]


def _sync_batch(
    texts: list[str], domain_id: str, entity_hints: dict[str, list[str]] | None = None
) -> list[dict[str, str]]:
    """Blocking GLiNER batch inference — runs in _pool thread."""
    labels, desc_to_key = _labels_with_hints(domain_id, entity_hints)
    raw_batched = model.inference(texts, labels, threshold=0.5)
    entities = []
    for chunk_entities in raw_batched:
        for e in chunk_entities:
            entities.append({
                "entity_name": e["text"],
                "classification": desc_to_key.get(e["label"], e["label"]),
            })
    return entities


@app.get("/healthz")
async def health() -> dict[str, str]:
    return {"status": "ok", "model": MODEL_NAME}


@app.post("/v1/batch-extract")
async def batch_extract(request: Request) -> dict[str, list]:
    """Accept multiple text chunks in one call; process in one forward pass."""
    body: dict[str, Any] = await request.json()
    texts: list[str] = body.get("texts", [])
    domain_id: str = body.get("domain_id", DEFAULT_DOMAIN)
    entity_hints: dict[str, list[str]] | None = body.get("entity_hints")

    non_empty = [t for t in texts if t.strip()]
    if not non_empty:
        return {"entities": []}

    loop = asyncio.get_running_loop()
    entities = await loop.run_in_executor(
        _pool, _sync_batch, non_empty, domain_id, entity_hints
    )
    return {"entities": entities}


@app.post("/v1/extract")
async def extract(request: Request) -> dict[str, list]:
    body: dict[str, Any] = await request.json()
    text: str = body.get("text", "")
    domain_id: str = body.get("domain_id", DEFAULT_DOMAIN)
    entity_hints: dict[str, list[str]] | None = body.get("entity_hints")

    if not text.strip():
        return {"entities": []}

    loop = asyncio.get_running_loop()
    entities = await loop.run_in_executor(
        _pool, _sync_predict, text, domain_id, entity_hints
    )
    return {"entities": entities}


if __name__ == "__main__":
    host = os.environ.get("GLINER_HOST", "127.0.0.1")
    port = int(os.environ.get("GLINER_PORT", "9085"))
    uvicorn.run(app, host=host, port=port, log_level="info")
