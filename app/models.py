from __future__ import annotations

from enum import Enum
from typing import Literal
from uuid import UUID

from pydantic import BaseModel, ConfigDict, Field, field_validator


class Channel(str, Enum):
    whatsapp = "whatsapp"
    sms = "sms"


class MessageStatus(str, Enum):
    queued = "queued"
    sent = "sent"
    delivered = "delivered"
    failed = "failed"


class TriggerType(str, Enum):
    BOLSA_FAMILIA_ELEGIVEL = "BOLSA_FAMILIA_ELEGIVEL"
    RISCO_CONDICIONALIDADE = "RISCO_CONDICIONALIDADE"
    RECADASTRAMENTO_PROXIMO = "RECADASTRAMENTO_PROXIMO"
    BPC_NAO_REQUERIDO = "BPC_NAO_REQUERIDO"
    PERFIL_SCFV = "PERFIL_SCFV"


class ServiceType(str, Enum):
    bolsa_familia = "bolsa_familia"
    cadastro_unico = "cadastro_unico"
    bpc = "bpc"
    outro_atendimento = "outro_atendimento"


class AppointmentStatus(str, Enum):
    confirmado = "confirmado"
    cancelado = "cancelado"
    concluido = "concluido"


class Family(BaseModel):
    adults: int = Field(ge=0)
    children: int = Field(ge=0)
    elderly: int = Field(ge=0)
    total: int = Field(ge=1)


class Profile(BaseModel):
    nis: str
    cpf: str | None = None
    name: str
    phone: str | None = None
    family: Family
    per_capita_income: float
    active_benefits: list[str]
    opt_in: bool
    opt_in_at: str | None = None
    last_visit_at: str | None = None
    created_at: str
    updated_at: str

    @field_validator("nis")
    @classmethod
    def validate_nis(cls, value: str) -> str:
        if len(value) != 11 or not value.isdigit():
            raise ValueError("nis must contain 11 digits")
        return value


class Message(BaseModel):
    id: UUID
    nis: str
    trigger: TriggerType
    channel: Channel
    status: MessageStatus
    body: str
    sent_at: str | None = None
    created_at: str


class Unit(BaseModel):
    id: str
    name: str
    address: str
    type: Literal["CRAS", "CREAS"]


class Appointment(BaseModel):
    id: UUID
    code: str
    nis: str
    service: ServiceType
    unit: Unit
    scheduled_at: str
    required_documents: list[str]
    status: AppointmentStatus
    created_at: str


class TriageAnswer(BaseModel):
    question_id: str
    value: str


class TriageResult(BaseModel):
    service: ServiceType
    unit_id: str
    appointment_id: str | None = None
    documents: list[str]


class TriagemSession(BaseModel):
    id: UUID
    channel: Literal["whatsapp", "web"]
    nis: str | None = None
    started_at: str
    completed_at: str | None = None
    answers: list[TriageAnswer]
    result: TriageResult | None = None


class ErrorDetail(BaseModel):
    code: str
    message: str


class ErrorResponse(BaseModel):
    error: ErrorDetail


class ListResponse(BaseModel):
    model_config = ConfigDict(arbitrary_types_allowed=True)

    items: list
    total: int


class StatsSummary(BaseModel):
    messages: dict[str, int]
    appointments: dict[str, int]
    profiles: dict[str, int]
    opt_in: dict[str, float | int]


class MessageDispatchRequest(BaseModel):
    nis: str
    trigger: TriggerType


class AppointmentCreateRequest(BaseModel):
    nis: str | None = None
    service: ServiceType | None = None
    unit: Unit | None = None
    unit_id: str | None = None
    scheduled_at: str | None = None
    required_documents: list[str] | None = None
    status: AppointmentStatus | None = None


class AppointmentPatchRequest(BaseModel):
    status: AppointmentStatus


class OptInRequest(BaseModel):
    opt_in: bool


class TriagemStartRequest(BaseModel):
    channel: Literal["whatsapp", "web"]
    nis: str | None = None


class TriagemAnswerRequest(BaseModel):
    question_id: str
    value: str


class TriggerEvaluateRequest(BaseModel):
    nis: str | None = None


class TriggerInfo(BaseModel):
    type: TriggerType
    label: str
    description: str
