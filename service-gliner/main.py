import asyncio
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

# Domain-specific label descriptions — plain English, GLiNER reads these literally.
# Concrete examples in descriptions act as KoGNER-style entity hints.
DOMAIN_LABELS: dict[str, dict[str, str]] = {
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
    # Documentation domain: engineering sessions, code reviews, architecture docs,
    # build logs, git commits. Same five entity types; descriptions tuned for
    # technical prose rather than CRE or corporate content.
    "documentation": {
        "Person":   "a named developer, engineer, or contributor",
        "Company":  "a named company or technology organisation",
        "Project":  "a named software project, service, crate, or library",
        "Account":  "a named running service, system account, or API endpoint",
        "Location": "a named server, deployment environment, or infrastructure location",
    },
}

DEFAULT_DOMAIN = "projects"


def _sync_predict(text: str, domain_id: str) -> list[dict[str, str]]:
    """Blocking GLiNER call — runs in _pool thread, not the event loop."""
    label_map = DOMAIN_LABELS.get(domain_id, DOMAIN_LABELS[DEFAULT_DOMAIN])
    labels = list(label_map.values())
    desc_to_key = {v: k for k, v in label_map.items()}
    raw = model.predict_entities(text, labels, threshold=0.5)
    return [
        {
            "entity_name": e["text"],
            "classification": desc_to_key.get(e["label"], e["label"]),
        }
        for e in raw
    ]


def _sync_batch(texts: list[str], domain_id: str) -> list[dict[str, str]]:
    """Blocking GLiNER batch inference — runs in _pool thread."""
    label_map = DOMAIN_LABELS.get(domain_id, DOMAIN_LABELS[DEFAULT_DOMAIN])
    labels = list(label_map.values())
    desc_to_key = {v: k for k, v in label_map.items()}
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

    non_empty = [t for t in texts if t.strip()]
    if not non_empty:
        return {"entities": []}

    loop = asyncio.get_running_loop()
    entities = await loop.run_in_executor(_pool, _sync_batch, non_empty, domain_id)
    return {"entities": entities}


@app.post("/v1/extract")
async def extract(request: Request) -> dict[str, list]:
    body: dict[str, Any] = await request.json()
    text: str = body.get("text", "")
    domain_id: str = body.get("domain_id", DEFAULT_DOMAIN)

    if not text.strip():
        return {"entities": []}

    loop = asyncio.get_running_loop()
    entities = await loop.run_in_executor(_pool, _sync_predict, text, domain_id)
    return {"entities": entities}


if __name__ == "__main__":
    host = os.environ.get("GLINER_HOST", "127.0.0.1")
    port = int(os.environ.get("GLINER_PORT", "9085"))
    uvicorn.run(app, host=host, port=port, log_level="info")
