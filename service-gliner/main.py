import os
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


@app.get("/healthz")
async def health() -> dict[str, str]:
    return {"status": "ok", "model": MODEL_NAME}


@app.post("/v1/extract")
async def extract(request: Request) -> dict[str, list]:
    body: dict[str, Any] = await request.json()
    text: str = body.get("text", "")
    domain_id: str = body.get("domain_id", DEFAULT_DOMAIN)

    if not text.strip():
        return {"entities": []}

    label_map = DOMAIN_LABELS.get(domain_id, DOMAIN_LABELS[DEFAULT_DOMAIN])
    labels = list(label_map.values())

    raw = model.predict_entities(text, labels, threshold=0.5)

    # Map description back to classification key (Person, Company, etc.)
    desc_to_key = {v: k for k, v in label_map.items()}
    entities = [
        {
            "entity_name": e["text"],
            "classification": desc_to_key.get(e["label"], e["label"]),
        }
        for e in raw
    ]
    return {"entities": entities}


if __name__ == "__main__":
    host = os.environ.get("GLINER_HOST", "127.0.0.1")
    port = int(os.environ.get("GLINER_PORT", "9085"))
    uvicorn.run(app, host=host, port=port, log_level="info")
