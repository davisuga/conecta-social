use chrono::{Duration, Utc};

use crate::models::{ProfileRow, TriggerType};

pub struct TriggerMeta {
    pub r#type: TriggerType,
    pub label: &'static str,
    pub description: &'static str,
}

pub fn catalog() -> Vec<TriggerMeta> {
    vec![
        TriggerMeta {
            r#type: TriggerType::BolsaFamiliaElegivel,
            label: "Bolsa Família — elegível",
            description:
                "Famílias com renda per capita até a linha de pobreza ainda não inscritas no programa.",
        },
        TriggerMeta {
            r#type: TriggerType::RiscoCondicionalidade,
            label: "Risco de condicionalidade",
            description:
                "Famílias beneficiárias com risco de descumprimento de condicionalidades de saúde ou educação.",
        },
        TriggerMeta {
            r#type: TriggerType::RecadastramentoProximo,
            label: "Recadastramento próximo",
            description:
                "Famílias com cadastro próximo do prazo de atualização (24 meses).",
        },
        TriggerMeta {
            r#type: TriggerType::BpcNaoRequerido,
            label: "BPC não requerido",
            description:
                "Idosos ou pessoas com deficiência potencialmente elegíveis ao BPC sem benefício ativo.",
        },
        TriggerMeta {
            r#type: TriggerType::PerfilScfv,
            label: "Perfil SCFV",
            description:
                "Crianças, jovens ou idosos com perfil para o Serviço de Convivência e Fortalecimento de Vínculos.",
        },
    ]
}

pub fn label(t: TriggerType) -> &'static str {
    match t {
        TriggerType::BolsaFamiliaElegivel => "Bolsa Família — elegível",
        TriggerType::RiscoCondicionalidade => "Risco de condicionalidade",
        TriggerType::RecadastramentoProximo => "Recadastramento próximo",
        TriggerType::BpcNaoRequerido => "BPC não requerido",
        TriggerType::PerfilScfv => "Perfil SCFV",
    }
}

fn first_name(profile_name: &str) -> &str {
    profile_name
        .split_whitespace()
        .next()
        .unwrap_or(profile_name)
}

pub fn message_body(t: TriggerType, profile_name: &str) -> String {
    let first = first_name(profile_name);
    match t {
        TriggerType::BolsaFamiliaElegivel => format!(
            "Olá, {first}! Identificamos que sua família pode ter direito ao Bolsa Família. Compareça ao CRAS mais próximo para atualizar seu cadastro."
        ),
        TriggerType::RiscoCondicionalidade => format!(
            "Olá, {first}! Sua família corre risco de perder benefícios. Procure o CRAS para regularizar saúde e educação das crianças."
        ),
        TriggerType::RecadastramentoProximo => format!(
            "Olá, {first}! Seu Cadastro Único precisa ser atualizado em breve. Procure o CRAS para evitar a suspensão dos benefícios."
        ),
        TriggerType::BpcNaoRequerido => format!(
            "Olá, {first}! Há pessoas na sua família que podem ter direito ao BPC. Procure o CRAS para avaliar o benefício."
        ),
        TriggerType::PerfilScfv => format!(
            "Olá, {first}! Sua família tem perfil para o SCFV. Procure o CRAS para conhecer as atividades de convivência disponíveis."
        ),
    }
}

// Renda per capita máxima do Bolsa Família (regra atual: até R$ 218/mês).
const BOLSA_FAMILIA_PER_CAPITA_MAX: f64 = 218.00;
// Limite BPC: 1/4 do salário mínimo de 2026 (R$ 1.518).
const BPC_PER_CAPITA_MAX: f64 = 379.50;

/// Prioridade dos triggers — menor número = mais urgente.
/// Ordem definida na especificação do produto.
pub fn priority(t: TriggerType) -> u8 {
    match t {
        TriggerType::RecadastramentoProximo => 0,
        TriggerType::RiscoCondicionalidade => 1,
        TriggerType::BpcNaoRequerido => 2,
        TriggerType::BolsaFamiliaElegivel => 3,
        TriggerType::PerfilScfv => 4,
    }
}

pub fn applicable_triggers(p: &ProfileRow) -> Vec<TriggerType> {
    let mut out = Vec::new();
    let now = Utc::now();

    let has_bolsa = p.active_benefits.iter().any(|b| b == "bolsa_familia");
    let has_bpc = p.active_benefits.iter().any(|b| b == "bpc");

    if p.per_capita_income <= BOLSA_FAMILIA_PER_CAPITA_MAX && !has_bolsa {
        out.push(TriggerType::BolsaFamiliaElegivel);
    }

    let months_since_update = (now - p.updated_at).num_days() / 30;
    let cadastro_vencido = months_since_update >= 24;
    let cadastro_proximo_bolsa = has_bolsa && months_since_update >= 18;
    if cadastro_vencido || cadastro_proximo_bolsa {
        out.push(TriggerType::RecadastramentoProximo);
    }

    if p.family_elderly > 0 && p.per_capita_income <= BPC_PER_CAPITA_MAX && !has_bpc {
        out.push(TriggerType::BpcNaoRequerido);
    }

    let visit_stale = match p.last_visit_at {
        None => true,
        Some(ts) => ts < now - Duration::days(9 * 30),
    };
    if has_bolsa && p.family_children > 0 && visit_stale {
        out.push(TriggerType::RiscoCondicionalidade);
    }

    let vulnerabilidade = p.per_capita_income <= BPC_PER_CAPITA_MAX
        && (p.family_children > 0 || p.family_elderly > 0);
    if vulnerabilidade {
        out.push(TriggerType::PerfilScfv);
    }

    out
}

/// Pick the highest-priority applicable trigger for a profile.
/// Used by the daily cron to send a single message per family.
pub fn pick_one(p: &ProfileRow) -> Option<TriggerType> {
    applicable_triggers(p).into_iter().min_by_key(|t| priority(*t))
}
