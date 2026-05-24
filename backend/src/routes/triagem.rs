use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::{Duration, Utc};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    error::{ApiError, ApiResult},
    models::{
        Appointment, AppointmentRow, ListResponse, ServiceType, TriagemAnswer, TriagemAnswerRow,
        TriagemChannel, TriagemResult, TriagemSession, TriagemSessionRow, APPOINTMENT_SELECT,
    },
    state::AppState,
};

fn default_limit() -> i64 {
    50
}

fn default_offset() -> i64 {
    0
}

fn is_valid_nis(s: &str) -> bool {
    s.len() == 11 && s.bytes().all(|b| b.is_ascii_digit())
}

fn generate_code() -> String {
    let n: u32 = rand::thread_rng().gen_range(0..100_000);
    format!("AG-{:05}", n)
}

fn documents_for(service: ServiceType) -> Vec<String> {
    match service {
        ServiceType::BolsaFamilia => vec![
            "RG".into(),
            "CPF".into(),
            "Comprovante de residência".into(),
            "Carteira de trabalho".into(),
        ],
        ServiceType::CadastroUnico => vec![
            "RG".into(),
            "CPF".into(),
            "Comprovante de residência".into(),
            "Certidão de nascimento dos filhos".into(),
        ],
        ServiceType::Bpc => vec![
            "RG".into(),
            "CPF".into(),
            "Laudo médico".into(),
            "Comprovante de residência".into(),
        ],
        ServiceType::OutroAtendimento => vec!["RG".into(), "CPF".into()],
    }
}

async fn load_session(db: &PgPool, id: Uuid) -> ApiResult<TriagemSession> {
    let row: TriagemSessionRow = sqlx::query_as::<_, TriagemSessionRow>(
        "SELECT id, channel, nis, started_at, completed_at, \
                result_service, result_unit_id, result_documents, result_appointment_id \
         FROM triagem_sessions WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(db)
    .await?
    .ok_or(ApiError::NotFound)?;

    let answer_rows: Vec<TriagemAnswerRow> = sqlx::query_as::<_, TriagemAnswerRow>(
        "SELECT question_id, value FROM triagem_answers \
         WHERE session_id = $1 ORDER BY answered_at",
    )
    .bind(id)
    .fetch_all(db)
    .await?;

    let answers: Vec<TriagemAnswer> = answer_rows
        .into_iter()
        .map(|a| TriagemAnswer {
            question_id: a.question_id,
            value: a.value,
        })
        .collect();

    let result = match (row.result_service, &row.result_unit_id) {
        (Some(service), Some(unit_id)) => Some(TriagemResult {
            service,
            unit_id: unit_id.clone(),
            appointment_id: row.result_appointment_id,
            documents: row.result_documents,
        }),
        _ => None,
    };

    Ok(TriagemSession {
        id: row.id,
        channel: row.channel,
        nis: row.nis.map(|n| n.trim().to_string()),
        started_at: row.started_at,
        completed_at: row.completed_at,
        answers,
        result,
    })
}

#[derive(Debug, Deserialize)]
pub struct SessionsQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default = "default_offset")]
    pub offset: i64,
}

pub async fn sessions(
    State(state): State<AppState>,
    Query(q): Query<SessionsQuery>,
) -> ApiResult<Json<ListResponse<TriagemSession>>> {
    let limit = q.limit.clamp(1, 200);
    let offset = q.offset.max(0);

    let id_rows: Vec<(Uuid,)> = sqlx::query_as::<_, (Uuid,)>(
        "SELECT id FROM triagem_sessions ORDER BY started_at DESC LIMIT $1 OFFSET $2",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await?;

    let mut items: Vec<TriagemSession> = Vec::with_capacity(id_rows.len());
    for (id,) in id_rows {
        items.push(load_session(&state.db, id).await?);
    }

    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM triagem_sessions")
        .fetch_one(&state.db)
        .await?;

    Ok(Json(ListResponse { items, total }))
}

#[derive(Debug, Deserialize)]
pub struct StartReq {
    pub channel: TriagemChannel,
    pub nis: Option<String>,
}

pub async fn start(
    State(state): State<AppState>,
    Json(body): Json<StartReq>,
) -> ApiResult<Json<TriagemSession>> {
    if let Some(ref nis) = body.nis {
        if !is_valid_nis(nis) {
            return Err(ApiError::BadRequest(
                "nis must contain exactly 11 digits".into(),
            ));
        }
        let exists: Option<i32> = sqlx::query_scalar("SELECT 1 FROM profiles WHERE nis = $1")
            .bind(nis)
            .fetch_optional(&state.db)
            .await?;
        if exists.is_none() {
            return Err(ApiError::NotFound);
        }
    }

    let new_id: Uuid = sqlx::query_scalar(
        "INSERT INTO triagem_sessions (channel, nis) VALUES ($1::triagem_channel, $2) RETURNING id",
    )
    .bind(body.channel)
    .bind(body.nis.as_deref())
    .fetch_one(&state.db)
    .await?;

    let session = load_session(&state.db, new_id).await?;
    Ok(Json(session))
}

#[derive(Debug, Deserialize)]
pub struct AnswerReq {
    pub question_id: String,
    pub value: String,
}

pub async fn answer(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<AnswerReq>,
) -> ApiResult<Json<TriagemSession>> {
    let row: Option<(Option<chrono::DateTime<Utc>>,)> =
        sqlx::query_as::<_, (Option<chrono::DateTime<Utc>>,)>(
            "SELECT completed_at FROM triagem_sessions WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&state.db)
        .await?;

    let completed_at = match row {
        Some((c,)) => c,
        None => return Err(ApiError::NotFound),
    };
    if completed_at.is_some() {
        return Err(ApiError::Conflict("session already completed".into()));
    }

    sqlx::query(
        "INSERT INTO triagem_answers (session_id, question_id, value) \
         VALUES ($1, $2, $3) \
         ON CONFLICT (session_id, question_id) \
         DO UPDATE SET value = EXCLUDED.value, answered_at = now()",
    )
    .bind(id)
    .bind(&body.question_id)
    .bind(&body.value)
    .execute(&state.db)
    .await?;

    let session = load_session(&state.db, id).await?;
    Ok(Json(session))
}

#[derive(Debug, Serialize)]
pub struct FinalizeResponse {
    pub session: TriagemSession,
    pub appointment: Appointment,
}

pub async fn finalize(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<FinalizeResponse>> {
    let mut tx = state.db.begin().await?;

    let row: TriagemSessionRow = sqlx::query_as::<_, TriagemSessionRow>(
        "SELECT id, channel, nis, started_at, completed_at, \
                result_service, result_unit_id, result_documents, result_appointment_id \
         FROM triagem_sessions WHERE id = $1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::NotFound)?;

    if row.completed_at.is_some() {
        return Err(ApiError::Conflict("session already completed".into()));
    }

    let session_nis = row.nis.as_ref().map(|n| n.trim().to_string());

    let answer_rows: Vec<TriagemAnswerRow> = sqlx::query_as::<_, TriagemAnswerRow>(
        "SELECT question_id, value FROM triagem_answers \
         WHERE session_id = $1 ORDER BY answered_at",
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await?;

    let needs_today = answer_rows.iter().any(|a| {
        a.question_id == "precisa_hoje" && a.value.to_lowercase().contains("sim")
    });
    let no_cadastro = answer_rows.iter().any(|a| {
        a.question_id == "tem_cadastro"
            && {
                let v = a.value.to_lowercase();
                v.contains("nao") || v.contains("não")
            }
    });

    let service = if needs_today {
        ServiceType::OutroAtendimento
    } else if no_cadastro {
        ServiceType::CadastroUnico
    } else if session_nis.is_some() {
        ServiceType::BolsaFamilia
    } else {
        ServiceType::CadastroUnico
    };

    let unit_id = "cras-centro".to_string();
    let documents = documents_for(service);

    let appointment_nis = match session_nis.as_ref() {
        Some(n) => n.clone(),
        None => {
            return Err(ApiError::Conflict(
                "triagem session has no associated NIS".into(),
            ))
        }
    };

    let code = generate_code();
    let scheduled_at = Utc::now() + Duration::days(2);

    let appointment_id: Uuid = sqlx::query_scalar(
        "INSERT INTO appointments (code, nis, service, unit_id, scheduled_at, required_documents) \
         VALUES ($1, $2, $3::service_type, $4, $5, $6) RETURNING id",
    )
    .bind(&code)
    .bind(&appointment_nis)
    .bind(service)
    .bind(&unit_id)
    .bind(scheduled_at)
    .bind(&documents)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query(
        "UPDATE triagem_sessions SET completed_at = now(), \
                result_service = $1::service_type, \
                result_unit_id = $2, \
                result_documents = $3, \
                result_appointment_id = $4 \
         WHERE id = $5",
    )
    .bind(service)
    .bind(&unit_id)
    .bind(&documents)
    .bind(appointment_id)
    .bind(id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let session = load_session(&state.db, id).await?;

    let sql = format!(
        "SELECT {APPOINTMENT_SELECT} FROM appointments a \
         JOIN units u ON u.id = a.unit_id WHERE a.id = $1"
    );
    let appt_row: AppointmentRow = sqlx::query_as::<_, AppointmentRow>(&sql)
        .bind(appointment_id)
        .fetch_one(&state.db)
        .await?;

    Ok(Json(FinalizeResponse {
        session,
        appointment: Appointment::from(appt_row),
    }))
}
