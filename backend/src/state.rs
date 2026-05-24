use sqlx::PgPool;

use crate::services::whatsapp::WhatsappService;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub whatsapp: WhatsappService,
}
