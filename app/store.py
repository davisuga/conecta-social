from __future__ import annotations

from dataclasses import dataclass, field
from uuid import UUID

from app.models import Appointment, Message, Profile, TriagemSession, Unit
from app.utils import utc_now_iso


@dataclass
class MemoryStore:
    profiles: dict[str, Profile] = field(default_factory=dict)
    units: dict[str, Unit] = field(default_factory=dict)
    messages: dict[UUID, Message] = field(default_factory=dict)
    appointments: dict[UUID, Appointment] = field(default_factory=dict)
    triagem_sessions: dict[UUID, TriagemSession] = field(default_factory=dict)
    appointment_sequence: int = 12340


store = MemoryStore()


def seed_store() -> None:
    now = utc_now_iso()
    store.units = {
        "cras-centro": Unit(
            id="cras-centro",
            name="CRAS Centro",
            address="Rua das Flores, 120 - Centro",
            type="CRAS",
        ),
        "cras-norte": Unit(
            id="cras-norte",
            name="CRAS Norte",
            address="Av. Esperanca, 880 - Jardim Norte",
            type="CRAS",
        ),
        "creas-municipal": Unit(
            id="creas-municipal",
            name="CREAS Municipal",
            address="Rua Cidadania, 300 - Centro",
            type="CREAS",
        ),
    }
    store.profiles = {
        "16450319210": Profile(
            nis="16450319210",
            cpf="32165498709",
            name="Ana Paula Santos",
            phone="+5511988881001",
            family={"adults": 2, "children": 2, "elderly": 0, "total": 4},
            per_capita_income=180,
            active_benefits=[],
            opt_in=True,
            opt_in_at=now,
            last_visit_at="2026-02-12T13:00:00Z",
            created_at=now,
            updated_at=now,
        ),
        "20144587033": Profile(
            nis="20144587033",
            cpf="45678912300",
            name="Bruno Almeida",
            phone="+5511977772033",
            family={"adults": 1, "children": 2, "elderly": 0, "total": 3},
            per_capita_income=210,
            active_benefits=["bolsa_familia"],
            opt_in=True,
            opt_in_at=now,
            last_visit_at="2026-01-29T13:00:00Z",
            created_at=now,
            updated_at=now,
        ),
        "33412098765": Profile(
            nis="33412098765",
            cpf="74185296310",
            name="Carla Nascimento",
            phone="+5511966663065",
            family={"adults": 2, "children": 0, "elderly": 0, "total": 2},
            per_capita_income=390,
            active_benefits=[],
            opt_in=True,
            opt_in_at=now,
            last_visit_at="2025-12-02T13:00:00Z",
            created_at=now,
            updated_at=now,
        ),
        "44881230019": Profile(
            nis="44881230019",
            cpf="96325874105",
            name="Elza Ferreira",
            phone="+5511955554019",
            family={"adults": 0, "children": 0, "elderly": 1, "total": 1},
            per_capita_income=0,
            active_benefits=[],
            opt_in=True,
            opt_in_at=now,
            last_visit_at="2026-03-03T13:00:00Z",
            created_at=now,
            updated_at=now,
        ),
        "55331984077": Profile(
            nis="55331984077",
            cpf="15975348620",
            name="Familia Moura",
            phone="+5511944445077",
            family={"adults": 2, "children": 3, "elderly": 0, "total": 5},
            per_capita_income=260,
            active_benefits=["bolsa_familia"],
            opt_in=True,
            opt_in_at=now,
            last_visit_at="2025-10-18T13:00:00Z",
            created_at=now,
            updated_at=now,
        ),
        "66001234580": Profile(
            nis="66001234580",
            cpf="85274196309",
            name="Joao Batista",
            phone="+5511933336080",
            family={"adults": 2, "children": 1, "elderly": 0, "total": 3},
            per_capita_income=140,
            active_benefits=[],
            opt_in=False,
            opt_in_at=None,
            last_visit_at="2026-02-22T13:00:00Z",
            created_at=now,
            updated_at=now,
        ),
    }
    store.messages = {}
    store.appointments = {}
    store.triagem_sessions = {}
    store.appointment_sequence = 12340


seed_store()
