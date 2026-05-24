from __future__ import annotations

from uuid import UUID

from fastapi import FastAPI, HTTPException, Request, status
from fastapi.exceptions import RequestValidationError
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import JSONResponse

from app.models import (
    AppointmentPatchRequest,
    AppointmentCreateRequest,
    Channel,
    MessageDispatchRequest,
    MessageStatus,
    OptInRequest,
    ServiceType,
    TriagemAnswerRequest,
    TriagemStartRequest,
    TriggerEvaluateRequest,
    TriggerType,
    AppointmentStatus,
)
from app.services import (
    ApiError,
    add_triagem_answer,
    create_appointment,
    dispatch_message,
    evaluate_and_dispatch,
    finalize_triagem,
    get_profile_or_404,
    get_summary,
    patch_appointment_status,
    start_triagem,
    TRIGGERS,
    update_profile_opt_in,
)
from app.store import store
from app.utils import paginate


app = FastAPI(title="Experimenta Backend", version="0.1.0")

app.add_middleware(
    CORSMiddleware,
    allow_origins=["http://localhost:5173"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)


@app.exception_handler(ApiError)
async def api_error_handler(_: Request, exc: ApiError) -> JSONResponse:
    return JSONResponse(
        status_code=exc.status_code,
        content={"error": {"code": exc.code, "message": exc.message}},
    )


@app.exception_handler(RequestValidationError)
async def validation_error_handler(_: Request, exc: RequestValidationError) -> JSONResponse:
    return JSONResponse(
        status_code=422,
        content={
            "error": {
                "code": "VALIDATION_ERROR",
                "message": exc.errors()[0]["msg"] if exc.errors() else "Invalid request.",
            }
        },
    )


@app.exception_handler(HTTPException)
async def http_error_handler(_: Request, exc: HTTPException) -> JSONResponse:
    return JSONResponse(
        status_code=exc.status_code,
        content={"error": {"code": "HTTP_ERROR", "message": str(exc.detail)}},
    )


@app.exception_handler(Exception)
async def unexpected_error_handler(_: Request, exc: Exception) -> JSONResponse:
    return JSONResponse(
        status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
        content={"error": {"code": "INTERNAL_ERROR", "message": str(exc)}},
    )


@app.get("/health")
async def health() -> dict[str, str]:
    return {"status": "ok"}


@app.get("/api/stats/summary")
async def stats_summary() -> dict:
    return get_summary()


@app.get("/api/messages")
async def list_messages(
    limit: int = 20,
    offset: int = 0,
    trigger: TriggerType | None = None,
    channel: Channel | None = None,
    status: MessageStatus | None = None,
) -> dict:
    items = sorted(store.messages.values(), key=lambda item: item.created_at, reverse=True)
    if trigger:
        items = [item for item in items if item.trigger == trigger]
    if channel:
        items = [item for item in items if item.channel == channel]
    if status:
        items = [item for item in items if item.status == status]
    page, total = paginate(items, limit, offset)
    return {"items": page, "total": total}


@app.get("/api/messages/recent")
async def recent_messages(limit: int = 5) -> list:
    items = sorted(store.messages.values(), key=lambda item: item.created_at, reverse=True)
    return items[: max(1, min(limit, 100))]


@app.post("/api/messages/dispatch")
async def dispatch_message_route(payload: MessageDispatchRequest):
    return dispatch_message(payload.nis, payload.trigger)


@app.get("/api/appointments")
async def list_appointments(
    limit: int = 20,
    offset: int = 0,
    service: ServiceType | None = None,
    status: AppointmentStatus | None = None,
    unit_id: str | None = None,
) -> dict:
    items = sorted(store.appointments.values(), key=lambda item: item.created_at, reverse=True)
    if service:
        items = [item for item in items if item.service == service]
    if status:
        items = [item for item in items if item.status == status]
    if unit_id:
        items = [item for item in items if item.unit.id == unit_id]
    page, total = paginate(items, limit, offset)
    return {"items": page, "total": total}


@app.get("/api/appointments/recent")
async def recent_appointments(limit: int = 5) -> list:
    items = sorted(store.appointments.values(), key=lambda item: item.created_at, reverse=True)
    return items[: max(1, min(limit, 100))]


@app.post("/api/appointments")
async def create_appointment_route(payload: AppointmentCreateRequest):
    return create_appointment(payload)


@app.patch("/api/appointments/{appointment_id}")
async def patch_appointment_route(appointment_id: UUID, payload: AppointmentPatchRequest):
    return patch_appointment_status(appointment_id, payload.status)


@app.get("/api/profiles")
async def list_profiles(limit: int = 20, offset: int = 0) -> dict:
    items = sorted(store.profiles.values(), key=lambda item: item.created_at, reverse=True)
    page, total = paginate(items, limit, offset)
    return {"items": page, "total": total}


@app.get("/api/profiles/{nis}")
async def get_profile(nis: str):
    return get_profile_or_404(nis)


@app.post("/api/profiles/{nis}/opt-in")
async def set_profile_opt_in(nis: str, payload: OptInRequest):
    return update_profile_opt_in(nis, payload.opt_in)


@app.get("/api/units")
async def list_units() -> list:
    return list(store.units.values())


@app.get("/api/triagem/sessions")
async def list_triagem_sessions(limit: int = 20, offset: int = 0) -> dict:
    items = sorted(store.triagem_sessions.values(), key=lambda item: item.started_at, reverse=True)
    page, total = paginate(items, limit, offset)
    return {"items": page, "total": total}


@app.post("/api/triagem/start")
async def start_triagem_route(payload: TriagemStartRequest):
    return start_triagem(payload.channel, payload.nis)


@app.post("/api/triagem/{session_id}/answer")
async def answer_triagem_route(session_id: UUID, payload: TriagemAnswerRequest):
    return add_triagem_answer(session_id, payload.question_id, payload.value)


@app.post("/api/triagem/{session_id}/finalize")
async def finalize_triagem_route(session_id: UUID) -> dict:
    session, appointment = finalize_triagem(session_id)
    return {"session": session, "appointment": appointment}


@app.get("/api/triggers")
async def list_triggers() -> list:
    return TRIGGERS


@app.post("/api/triggers/evaluate")
async def evaluate_triggers(payload: TriggerEvaluateRequest) -> list:
    return evaluate_and_dispatch(payload.nis)
