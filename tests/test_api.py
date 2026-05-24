from __future__ import annotations

from fastapi.testclient import TestClient

from app.main import app
from app.store import seed_store


def client() -> TestClient:
    seed_store()
    return TestClient(app)


def test_health_and_cors() -> None:
    api = client()

    response = api.get("/health", headers={"origin": "http://localhost:5173"})

    assert response.status_code == 200
    assert response.json() == {"status": "ok"}
    assert response.headers["access-control-allow-origin"] == "http://localhost:5173"


def test_error_shape_for_missing_profile() -> None:
    api = client()

    response = api.get("/api/profiles/00000000000")

    assert response.status_code == 404
    assert response.json() == {
        "error": {"code": "PROFILE_NOT_FOUND", "message": "Profile not found."}
    }


def test_profiles_pagination() -> None:
    api = client()

    response = api.get("/api/profiles?limit=2&offset=1")
    body = response.json()

    assert response.status_code == 200
    assert body["total"] >= 6
    assert len(body["items"]) == 2
    assert "nis" in body["items"][0]


def test_dispatch_message_with_opt_in_and_filters() -> None:
    api = client()

    created = api.post(
        "/api/messages/dispatch",
        json={"nis": "16450319210", "trigger": "BOLSA_FAMILIA_ELEGIVEL"},
    )
    assert created.status_code == 200
    message = created.json()
    assert message["nis"] == "16450319210"
    assert message["channel"] == "whatsapp"
    assert message["status"] == "sent"
    assert message["sent_at"].endswith("Z")

    filtered = api.get("/api/messages?trigger=BOLSA_FAMILIA_ELEGIVEL&channel=whatsapp&status=sent")
    assert filtered.status_code == 200
    assert filtered.json()["total"] == 1

    recent = api.get("/api/messages/recent?limit=5")
    assert recent.status_code == 200
    assert len(recent.json()) == 1


def test_dispatch_blocks_profiles_without_opt_in() -> None:
    api = client()

    response = api.post(
        "/api/messages/dispatch",
        json={"nis": "66001234580", "trigger": "BOLSA_FAMILIA_ELEGIVEL"},
    )

    assert response.status_code == 403
    assert response.json()["error"]["code"] == "OPT_IN_REQUIRED"


def test_evaluate_triggers_by_nis_and_globally() -> None:
    api = client()

    by_nis = api.post("/api/triggers/evaluate", json={"nis": "16450319210"})
    assert by_nis.status_code == 200
    assert {item["trigger"] for item in by_nis.json()} >= {
        "BOLSA_FAMILIA_ELEGIVEL",
        "PERFIL_SCFV",
    }

    all_profiles = api.post("/api/triggers/evaluate", json={})
    assert all_profiles.status_code == 200
    assert len(all_profiles.json()) >= 5
    assert all(item["nis"] != "66001234580" for item in all_profiles.json())


def test_appointments_create_list_recent_and_patch() -> None:
    api = client()

    created = api.post(
        "/api/appointments",
        json={
            "nis": "16450319210",
            "service": "bolsa_familia",
            "unit_id": "cras-centro",
            "required_documents": ["CPF"],
        },
    )
    assert created.status_code == 200
    appointment = created.json()
    assert appointment["code"].startswith("AG-")
    assert appointment["status"] == "confirmado"
    assert appointment["unit"]["id"] == "cras-centro"

    listed = api.get("/api/appointments?service=bolsa_familia&status=confirmado&unit_id=cras-centro")
    assert listed.status_code == 200
    assert listed.json()["total"] == 1

    recent = api.get("/api/appointments/recent?limit=5")
    assert recent.status_code == 200
    assert recent.json()[0]["id"] == appointment["id"]

    patched = api.patch(f"/api/appointments/{appointment['id']}", json={"status": "concluido"})
    assert patched.status_code == 200
    assert patched.json()["status"] == "concluido"


def test_patch_appointment_rejects_invalid_status() -> None:
    api = client()
    created = api.post("/api/appointments", json={"nis": "16450319210"})
    appointment_id = created.json()["id"]

    response = api.patch(f"/api/appointments/{appointment_id}", json={"status": "pendente"})

    assert response.status_code == 422
    assert response.json()["error"]["code"] == "VALIDATION_ERROR"


def test_opt_in_update() -> None:
    api = client()

    response = api.post("/api/profiles/66001234580/opt-in", json={"opt_in": True})

    assert response.status_code == 200
    assert response.json()["opt_in"] is True
    assert response.json()["opt_in_at"].endswith("Z")


def test_units_and_triggers() -> None:
    api = client()

    units = api.get("/api/units")
    triggers = api.get("/api/triggers")

    assert units.status_code == 200
    assert any(item["type"] == "CRAS" for item in units.json())
    assert triggers.status_code == 200
    assert {item["type"] for item in triggers.json()} == {
        "BOLSA_FAMILIA_ELEGIVEL",
        "RISCO_CONDICIONALIDADE",
        "RECADASTRAMENTO_PROXIMO",
        "BPC_NAO_REQUERIDO",
        "PERFIL_SCFV",
    }


def test_triagem_full_flow_creates_appointment() -> None:
    api = client()

    started = api.post("/api/triagem/start", json={"channel": "web"})
    assert started.status_code == 200
    session_id = started.json()["id"]

    api.post(f"/api/triagem/{session_id}/answer", json={"question_id": "servico", "value": "bpc"})
    api.post(
        f"/api/triagem/{session_id}/answer",
        json={"question_id": "cadastro_unico", "value": "sim"},
    )
    answered = api.post(
        f"/api/triagem/{session_id}/answer",
        json={"question_id": "nis", "value": "44881230019"},
    )
    assert answered.status_code == 200
    assert answered.json()["nis"] == "44881230019"

    finalized = api.post(f"/api/triagem/{session_id}/finalize")
    body = finalized.json()

    assert finalized.status_code == 200
    assert body["session"]["completed_at"].endswith("Z")
    assert body["session"]["result"]["service"] == "bpc"
    assert body["session"]["result"]["appointment_id"] == body["appointment"]["id"]
    assert body["appointment"]["status"] == "confirmado"

    sessions = api.get("/api/triagem/sessions?limit=10&offset=0")
    assert sessions.status_code == 200
    assert sessions.json()["total"] == 1
