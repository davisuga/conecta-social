use sqlx::PgPool;

use crate::services::agent::TriagemAgent;
use crate::services::whatsapp::WhatsappService;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub whatsapp: WhatsappService,
    pub agent: TriagemAgent,
}
