"""Sourdough Tracker — backend server.

Serves the PWA static files and provides a single /api/data endpoint
that stores the entire loaves array as a JSON file on disk.

Run: uvicorn server:app --host 127.0.0.1 --port 9099
"""
import json
import os
from pathlib import Path

from fastapi import FastAPI, HTTPException, Request
from fastapi.responses import FileResponse, JSONResponse
from fastapi.staticfiles import StaticFiles
from fastapi.middleware.cors import CORSMiddleware

DATA_FILE = Path(os.environ.get("BREAD_DATA_FILE", "/var/lib/local-bread/loaves.json"))
STATIC_DIR = Path(os.environ.get("BREAD_STATIC_DIR", Path(__file__).parent))

app = FastAPI(docs_url=None, redoc_url=None)

app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_methods=["GET", "POST"],
    allow_headers=["*"],
)


@app.get("/api/data")
def get_data():
    if not DATA_FILE.exists():
        return JSONResponse([])
    try:
        return JSONResponse(json.loads(DATA_FILE.read_text()))
    except (json.JSONDecodeError, OSError) as exc:
        raise HTTPException(status_code=500, detail=str(exc))


@app.post("/api/data")
async def save_data(request: Request):
    body = await request.body()
    try:
        loaves = json.loads(body)
        if not isinstance(loaves, list):
            raise ValueError("expected a JSON array")
    except (json.JSONDecodeError, ValueError) as exc:
        raise HTTPException(status_code=400, detail=str(exc))

    # Atomic write: temp file → rename
    tmp = DATA_FILE.with_suffix(".tmp")
    try:
        DATA_FILE.parent.mkdir(parents=True, exist_ok=True)
        tmp.write_text(json.dumps(loaves))
        os.replace(tmp, DATA_FILE)
    except OSError as exc:
        raise HTTPException(status_code=500, detail=str(exc))
    finally:
        if tmp.exists():
            tmp.unlink(missing_ok=True)

    return JSONResponse({"ok": True, "count": len(loaves)})


# Serve the PWA static files at root (must be last)
app.mount("/", StaticFiles(directory=str(STATIC_DIR), html=True), name="static")
