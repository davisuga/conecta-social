use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// ===== enums =====

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "channel", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    Whatsapp,
    Sms,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "message_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum MessageStatus {
    Queued,
    Sent,
    Delivered,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq, Hash)]
#[sqlx(type_name = "trigger_type", rename_all = "SCREAMING_SNAKE_CASE")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TriggerType {
    BolsaFamiliaElegivel,
    RiscoCondicionalidade,
    RecadastramentoProximo,
    BpcNaoRequerido,
    PerfilScfv,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "service_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ServiceType {
    BolsaFamilia,
    CadastroUnico,
    Bpc,
    OutroAtendimento,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "appointment_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum AppointmentStatus {
    Confirmado,
    Cancelado,
    Concluido,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "unit_type")]
pub enum UnitType {
    #[serde(rename = "CRAS")]
    #[sqlx(rename = "CRAS")]
    Cras,
    #[serde(rename = "CREAS")]
    #[sqlx(rename = "CREAS")]
    Creas,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "triagem_channel", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum TriagemChannel {
    Whatsapp,
    Web,
}

// ===== unit =====

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Unit {
    pub id: String,
    pub name: String,
    pub address: String,
    #[serde(rename = "type")]
    #[sqlx(rename = "type")]
    pub r#type: UnitType,
}

// ===== profile =====

#[derive(Debug, Clone, Serialize)]
pub struct Family {
    pub adults: i32,
    pub children: i32,
    pub elderly: i32,
    pub total: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Profile {
    pub nis: String,
    pub cpf: Option<String>,
    pub name: String,
    pub phone: Option<String>,
    pub family: Family,
    pub per_capita_income: f64,
    pub active_benefits: Vec<String>,
    pub opt_in: bool,
    pub opt_in_at: Option<DateTime<Utc>>,
    pub last_visit_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// Row shape returned by profile queries (per_capita_income cast to float8).
#[derive(Debug, FromRow)]
pub struct ProfileRow {
    pub nis: String,
    pub cpf: Option<String>,
    pub name: String,
    pub phone: Option<String>,
    pub family_adults: i32,
    pub family_children: i32,
    pub family_elderly: i32,
    pub family_total: i32,
    pub per_capita_income: f64,
    pub active_benefits: Vec<String>,
    pub opt_in: bool,
    pub opt_in_at: Option<DateTime<Utc>>,
    pub last_visit_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<ProfileRow> for Profile {
    fn from(r: ProfileRow) -> Self {
        Profile {
            nis: r.nis.trim().to_string(),
            cpf: r.cpf.map(|s| s.trim().to_string()),
            name: r.name,
            phone: r.phone,
            family: Family {
                adults: r.family_adults,
                children: r.family_children,
                elderly: r.family_elderly,
                total: r.family_total,
            },
            per_capita_income: r.per_capita_income,
            active_benefits: r.active_benefits,
            opt_in: r.opt_in,
            opt_in_at: r.opt_in_at,
            last_visit_at: r.last_visit_at,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

pub const PROFILE_SELECT: &str = "
    nis,
    cpf,
    name,
    phone,
    family_adults,
    family_children,
    family_elderly,
    family_total,
    per_capita_income::float8 AS per_capita_income,
    active_benefits,
    opt_in,
    opt_in_at,
    last_visit_at,
    created_at,
    updated_at
";

// ===== message =====

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Message {
    pub id: Uuid,
    pub nis: String,
    pub trigger: TriggerType,
    pub channel: Channel,
    pub status: MessageStatus,
    pub body: String,
    pub sent_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

// ===== appointment =====

#[derive(Debug, Clone, Serialize)]
pub struct Appointment {
    pub id: Uuid,
    pub code: String,
    pub nis: String,
    pub service: ServiceType,
    pub unit: Unit,
    pub scheduled_at: DateTime<Utc>,
    pub required_documents: Vec<String>,
    pub status: AppointmentStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
pub struct AppointmentRow {
    pub id: Uuid,
    pub code: String,
    pub nis: String,
    pub service: ServiceType,
    pub scheduled_at: DateTime<Utc>,
    pub required_documents: Vec<String>,
    pub status: AppointmentStatus,
    pub created_at: DateTime<Utc>,
    pub unit_id: String,
    pub unit_name: String,
    pub unit_address: String,
    pub unit_type: UnitType,
}

impl From<AppointmentRow> for Appointment {
    fn from(r: AppointmentRow) -> Self {
        Appointment {
            id: r.id,
            code: r.code,
            nis: r.nis.trim().to_string(),
            service: r.service,
            unit: Unit {
                id: r.unit_id,
                name: r.unit_name,
                address: r.unit_address,
                r#type: r.unit_type,
            },
            scheduled_at: r.scheduled_at,
            required_documents: r.required_documents,
            status: r.status,
            created_at: r.created_at,
        }
    }
}

pub const APPOINTMENT_SELECT: &str = "
    a.id,
    a.code,
    a.nis,
    a.service,
    a.scheduled_at,
    a.required_documents,
    a.status,
    a.created_at,
    u.id   AS unit_id,
    u.name AS unit_name,
    u.address AS unit_address,
    u.type AS unit_type
";

// ===== triagem =====

#[derive(Debug, Clone, Serialize)]
pub struct TriagemAnswer {
    pub question_id: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TriagemResult {
    pub service: ServiceType,
    pub unit_id: String,
    pub appointment_id: Option<Uuid>,
    pub documents: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TriagemSession {
    pub id: Uuid,
    pub channel: TriagemChannel,
    pub nis: Option<String>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub answers: Vec<TriagemAnswer>,
    pub result: Option<TriagemResult>,
}

#[derive(Debug, FromRow)]
pub struct TriagemSessionRow {
    pub id: Uuid,
    pub channel: TriagemChannel,
    pub nis: Option<String>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub result_service: Option<ServiceType>,
    pub result_unit_id: Option<String>,
    pub result_documents: Vec<String>,
    pub result_appointment_id: Option<Uuid>,
}

#[derive(Debug, FromRow)]
pub struct TriagemAnswerRow {
    pub question_id: String,
    pub value: String,
}

// ===== list envelope =====

#[derive(Debug, Serialize)]
pub struct ListResponse<T> {
    pub items: Vec<T>,
    pub total: i64,
}
