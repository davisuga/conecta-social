from __future__ import annotations

from datetime import datetime, timedelta, timezone
from uuid import UUID, uuid4

from app.models import (
    Appointment,
    AppointmentCreateRequest,
    AppointmentStatus,
    Channel,
    Message,
    MessageStatus,
    Profile,
    ServiceType,
    TriggerInfo,
    TriggerType,
    TriageAnswer,
    TriageResult,
    TriagemSession,
)
from app.store import store
from app.utils import today_count, utc_now_iso


class ApiError(Exception):
    def __init__(self, code: str, message: str, status_code: int = 400) -> None:
        self.code = code
        self.message = message
        self.status_code = status_code
        super().__init__(message)


TRIGGERS: list[TriggerInfo] = [
    TriggerInfo(
        type=TriggerType.BOLSA_FAMILIA_ELEGIVEL,
        label="Bolsa Familia elegivel",
        description="Familia com renda per capita compativel e sem beneficio ativo.",
    ),
    TriggerInfo(
        type=TriggerType.RISCO_CONDICIONALIDADE,
        label="Risco de condicionalidade",
        description="Perfil com acompanhamento preventivo para educacao ou saude.",
    ),
    TriggerInfo(
        type=TriggerType.RECADASTRAMENTO_PROXIMO,
        label="Recadastramento proximo",
        description="Cadastro ou visita socioassistencial precisa ser atualizado.",
    ),
    TriggerInfo(
        type=TriggerType.BPC_NAO_REQUERIDO,
        label="BPC nao requerido",
        description="Pessoa idosa no perfil familiar com renda compativel e sem BPC ativo.",
    ),
    TriggerInfo(
        type=TriggerType.PERFIL_SCFV,
        label="Perfil SCFV",
        description="Familia com criancas ou adolescentes indicada ao SCFV.",
    ),
]


def get_profile_or_404(nis: str) -> Profile:
    profile = store.profiles.get(nis)
    if not profile:
        raise ApiError("PROFILE_NOT_FOUND", "Profile not found.", 404)
    return profile


def get_unit_or_404(unit_id: str):
    unit = store.units.get(unit_id)
    if not unit:
        raise ApiError("UNIT_NOT_FOUND", "Unit not found.", 404)
    return unit


def get_summary() -> dict:
    total_profiles = len(store.profiles)
    granted = sum(1 for profile in store.profiles.values() if profile.opt_in)
    message_dates = [message.created_at for message in store.messages.values()]
    appointment_dates = [appointment.created_at for appointment in store.appointments.values()]

    return {
        "messages": {"total": len(store.messages), "today": today_count(message_dates)},
        "appointments": {
            "total": len(store.appointments),
            "today": today_count(appointment_dates),
        },
        "profiles": {"active": total_profiles},
        "opt_in": {
            "rate": round(granted / total_profiles, 4) if total_profiles else 0,
            "granted": granted,
            "total": total_profiles,
        },
    }


def dispatch_message(nis: str, trigger: TriggerType) -> Message:
    profile = get_profile_or_404(nis)
    if not profile.opt_in:
        raise ApiError("OPT_IN_REQUIRED", "Profile has not granted opt-in.", 403)

    now = utc_now_iso()
    message = Message(
        id=uuid4(),
        nis=profile.nis,
        trigger=trigger,
        channel=Channel.whatsapp if profile.phone else Channel.sms,
        status=MessageStatus.sent,
        body=build_message_body(profile, trigger),
        sent_at=now,
        created_at=now,
    )
    store.messages[message.id] = message
    return message


def build_message_body(profile: Profile, trigger: TriggerType) -> str:
    first_name = profile.name.split(" ")[0]
    templates = {
        TriggerType.BOLSA_FAMILIA_ELEGIVEL: (
            f"{first_name}, identificamos possivel elegibilidade ao Bolsa Familia. "
            "Procure o CRAS com documentos da familia."
        ),
        TriggerType.RISCO_CONDICIONALIDADE: (
            f"{first_name}, ha um prazo de condicionalidade para acompanhar. "
            "Regularize a situacao para evitar bloqueios."
        ),
        TriggerType.RECADASTRAMENTO_PROXIMO: (
            f"{first_name}, seu recadastramento esta proximo. "
            "Agende atualizacao no CRAS de referencia."
        ),
        TriggerType.BPC_NAO_REQUERIDO: (
            f"{first_name}, seu perfil pode ter direito ao BPC. "
            "Procure orientacao no CRAS."
        ),
        TriggerType.PERFIL_SCFV: (
            f"{first_name}, ha indicacao para o SCFV. "
            "Procure o CRAS para confirmar participacao."
        ),
    }
    return templates[trigger]


def evaluate_profile(profile: Profile) -> list[TriggerType]:
    triggers: list[TriggerType] = []
    if profile.per_capita_income <= 218 and "bolsa_familia" not in profile.active_benefits:
        triggers.append(TriggerType.BOLSA_FAMILIA_ELEGIVEL)
    if profile.nis.endswith("033"):
        triggers.append(TriggerType.RISCO_CONDICIONALIDADE)
    if profile.last_visit_at and profile.last_visit_at < "2026-01-01T00:00:00Z":
        triggers.append(TriggerType.RECADASTRAMENTO_PROXIMO)
    if profile.family.elderly > 0 and profile.per_capita_income <= 353 and "bpc" not in profile.active_benefits:
        triggers.append(TriggerType.BPC_NAO_REQUERIDO)
    if profile.family.children >= 2:
        triggers.append(TriggerType.PERFIL_SCFV)
    return triggers


def evaluate_and_dispatch(nis: str | None = None) -> list[Message]:
    profiles = [get_profile_or_404(nis)] if nis else list(store.profiles.values())
    dispatched: list[Message] = []
    for profile in profiles:
        if not profile.opt_in:
            continue
        for trigger in evaluate_profile(profile):
            dispatched.append(dispatch_message(profile.nis, trigger))
    return dispatched


def update_profile_opt_in(nis: str, opt_in: bool) -> Profile:
    profile = get_profile_or_404(nis)
    updated = profile.model_copy(
        update={
            "opt_in": opt_in,
            "opt_in_at": utc_now_iso() if opt_in else None,
            "updated_at": utc_now_iso(),
        }
    )
    store.profiles[nis] = updated
    return updated


def next_appointment_code() -> str:
    store.appointment_sequence += 1
    return f"AG-{store.appointment_sequence}"


def create_appointment(payload: AppointmentCreateRequest) -> Appointment:
    nis = payload.nis or "16450319210"
    get_profile_or_404(nis)
    unit = payload.unit or get_unit_or_404(payload.unit_id or "cras-centro")
    service = payload.service or ServiceType.outro_atendimento
    documents = payload.required_documents or documents_for_service(service)
    now = utc_now_iso()

    appointment = Appointment(
        id=uuid4(),
        code=next_appointment_code(),
        nis=nis,
        service=service,
        unit=unit,
        scheduled_at=payload.scheduled_at or default_schedule_iso(),
        required_documents=documents,
        status=payload.status or AppointmentStatus.confirmado,
        created_at=now,
    )
    store.appointments[appointment.id] = appointment
    return appointment


def patch_appointment_status(appointment_id: UUID, status: AppointmentStatus) -> Appointment:
    appointment = store.appointments.get(appointment_id)
    if not appointment:
        raise ApiError("APPOINTMENT_NOT_FOUND", "Appointment not found.", 404)
    updated = appointment.model_copy(update={"status": status})
    store.appointments[appointment_id] = updated
    return updated


def start_triagem(channel: str, nis: str | None = None) -> TriagemSession:
    if nis:
        get_profile_or_404(nis)
    session = TriagemSession(
        id=uuid4(),
        channel=channel,
        nis=nis,
        started_at=utc_now_iso(),
        completed_at=None,
        answers=[],
        result=None,
    )
    store.triagem_sessions[session.id] = session
    return session


def add_triagem_answer(session_id: UUID, question_id: str, value: str) -> TriagemSession:
    session = get_session_or_404(session_id)
    answers = [answer for answer in session.answers if answer.question_id != question_id]
    answers.append(TriageAnswer(question_id=question_id, value=value))
    updated = session.model_copy(update={"answers": answers, "nis": value if question_id == "nis" else session.nis})
    store.triagem_sessions[session_id] = updated
    return updated


def finalize_triagem(session_id: UUID) -> tuple[TriagemSession, Appointment]:
    session = get_session_or_404(session_id)
    answers = {answer.question_id: answer.value for answer in session.answers}
    nis = session.nis or answers.get("nis")
    if not nis:
        raise ApiError("TRIAGEM_INCOMPLETE", "Triagem session requires a nis answer.", 400)
    get_profile_or_404(nis)

    service = map_service(answers.get("servico"))
    unit_id = "cras-centro"
    documents = documents_for_service(service)
    appointment = create_appointment(
        AppointmentCreateRequest(
            nis=nis,
            service=service,
            unit_id=unit_id,
            required_documents=documents,
            status=AppointmentStatus.confirmado,
        )
    )
    result = TriageResult(
        service=service,
        unit_id=unit_id,
        appointment_id=str(appointment.id),
        documents=documents,
    )
    completed = session.model_copy(
        update={
            "nis": nis,
            "completed_at": utc_now_iso(),
            "result": result,
        }
    )
    store.triagem_sessions[session_id] = completed
    return completed, appointment


def get_session_or_404(session_id: UUID) -> TriagemSession:
    session = store.triagem_sessions.get(session_id)
    if not session:
        raise ApiError("TRIAGEM_SESSION_NOT_FOUND", "Triagem session not found.", 404)
    return session


def documents_for_service(service: ServiceType) -> list[str]:
    common = ["Documento com foto", "CPF", "Comprovante de endereco"]
    if service == ServiceType.bolsa_familia:
        return [*common, "Comprovante de renda quando houver"]
    if service == ServiceType.cadastro_unico:
        return [*common, "Documentos de todos da familia"]
    if service == ServiceType.bpc:
        return [*common, "Comprovante de renda familiar", "Laudo medico quando houver"]
    return [*common, "Documentos relacionados ao atendimento"]


def map_service(value: str | None) -> ServiceType:
    if value in {item.value for item in ServiceType}:
        return ServiceType(value)
    if value == "nao_sei" or not value:
        return ServiceType.outro_atendimento
    return ServiceType.outro_atendimento


def default_schedule_iso() -> str:
    return (datetime.now(timezone.utc).replace(microsecond=0) + timedelta(days=2)).isoformat().replace("+00:00", "Z")
