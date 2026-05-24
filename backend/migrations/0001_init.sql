-- conecta-social — postgres schema

CREATE EXTENSION IF NOT EXISTS "pgcrypto";  -- gen_random_uuid()
CREATE EXTENSION IF NOT EXISTS "citext";    -- case-insens text (optional)

-- ===== enums =====

CREATE TYPE channel        AS ENUM ('whatsapp','sms');
CREATE TYPE message_status AS ENUM ('queued','sent','delivered','failed');
CREATE TYPE trigger_type   AS ENUM (
  'BOLSA_FAMILIA_ELEGIVEL',
  'RISCO_CONDICIONALIDADE',
  'RECADASTRAMENTO_PROXIMO',
  'BPC_NAO_REQUERIDO',
  'PERFIL_SCFV'
);
CREATE TYPE service_type      AS ENUM ('bolsa_familia','cadastro_unico','bpc','outro_atendimento');
CREATE TYPE appointment_status AS ENUM ('confirmado','cancelado','concluido');
CREATE TYPE unit_type         AS ENUM ('CRAS','CREAS');
CREATE TYPE triagem_channel   AS ENUM ('whatsapp','web');

-- ===== updated_at trigger =====

CREATE OR REPLACE FUNCTION set_updated_at() RETURNS trigger AS $$
BEGIN NEW.updated_at = now(); RETURN NEW; END;
$$ LANGUAGE plpgsql;

-- ===== units =====

CREATE TABLE units (
  id          text PRIMARY KEY,
  name        text NOT NULL,
  address     text NOT NULL,
  type        unit_type NOT NULL,
  created_at  timestamptz NOT NULL DEFAULT now()
);

-- ===== profiles (mock cadúnico) =====

CREATE TABLE profiles (
  nis                 char(11) PRIMARY KEY CHECK (nis ~ '^[0-9]{11}$'),
  cpf                 char(11)   CHECK (cpf ~ '^[0-9]{11}$'),
  name                text       NOT NULL,
  phone               text,
  family_adults       int  NOT NULL DEFAULT 0 CHECK (family_adults  >= 0),
  family_children     int  NOT NULL DEFAULT 0 CHECK (family_children>= 0),
  family_elderly      int  NOT NULL DEFAULT 0 CHECK (family_elderly >= 0),
  family_total        int  GENERATED ALWAYS AS (family_adults + family_children + family_elderly) STORED,
  per_capita_income   numeric(10,2) NOT NULL DEFAULT 0 CHECK (per_capita_income >= 0),
  active_benefits     text[] NOT NULL DEFAULT '{}',
  opt_in              boolean NOT NULL DEFAULT false,
  opt_in_at           timestamptz,
  last_visit_at       timestamptz,
  created_at          timestamptz NOT NULL DEFAULT now(),
  updated_at          timestamptz NOT NULL DEFAULT now()
);
CREATE TRIGGER profiles_set_updated BEFORE UPDATE ON profiles
  FOR EACH ROW EXECUTE FUNCTION set_updated_at();
CREATE INDEX profiles_opt_in_idx        ON profiles (opt_in);
CREATE INDEX profiles_active_benefits_gin ON profiles USING gin (active_benefits);

-- ===== messages =====

CREATE TABLE messages (
  id          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  nis         char(11) NOT NULL REFERENCES profiles(nis) ON DELETE CASCADE,
  trigger     trigger_type   NOT NULL,
  channel     channel        NOT NULL,
  status      message_status NOT NULL DEFAULT 'queued',
  body        text NOT NULL,
  sent_at     timestamptz,
  delivered_at timestamptz,
  error       text,
  created_at  timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX messages_created_idx        ON messages (created_at DESC);
CREATE INDEX messages_nis_created_idx    ON messages (nis, created_at DESC);
CREATE INDEX messages_status_idx         ON messages (status);
CREATE INDEX messages_trigger_idx        ON messages (trigger);
CREATE INDEX messages_sent_today_idx     ON messages (sent_at)
  WHERE status IN ('sent','delivered');

-- ===== appointments =====

CREATE TABLE appointments (
  id                 uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  code               text UNIQUE NOT NULL,
  nis                char(11) NOT NULL REFERENCES profiles(nis) ON DELETE CASCADE,
  service            service_type NOT NULL,
  unit_id            text NOT NULL REFERENCES units(id),
  scheduled_at       timestamptz NOT NULL,
  required_documents text[] NOT NULL DEFAULT '{}',
  status             appointment_status NOT NULL DEFAULT 'confirmado',
  created_at         timestamptz NOT NULL DEFAULT now(),
  updated_at         timestamptz NOT NULL DEFAULT now()
);
CREATE TRIGGER appointments_set_updated BEFORE UPDATE ON appointments
  FOR EACH ROW EXECUTE FUNCTION set_updated_at();
CREATE INDEX appointments_created_idx       ON appointments (created_at DESC);
CREATE INDEX appointments_nis_idx           ON appointments (nis, created_at DESC);
CREATE INDEX appointments_unit_idx          ON appointments (unit_id, scheduled_at);
CREATE INDEX appointments_scheduled_idx     ON appointments (scheduled_at);
CREATE INDEX appointments_status_idx        ON appointments (status);

-- ===== triagem sessions =====

CREATE TABLE triagem_sessions (
  id            uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  channel       triagem_channel NOT NULL,
  nis           char(11) REFERENCES profiles(nis) ON DELETE SET NULL,
  started_at    timestamptz NOT NULL DEFAULT now(),
  completed_at  timestamptz,
  result_service       service_type,
  result_unit_id       text REFERENCES units(id),
  result_documents     text[] NOT NULL DEFAULT '{}',
  result_appointment_id uuid REFERENCES appointments(id) ON DELETE SET NULL
);
CREATE INDEX triagem_started_idx  ON triagem_sessions (started_at DESC);
CREATE INDEX triagem_nis_idx      ON triagem_sessions (nis);

CREATE TABLE triagem_answers (
  session_id   uuid NOT NULL REFERENCES triagem_sessions(id) ON DELETE CASCADE,
  question_id  text NOT NULL,
  value        text NOT NULL,
  answered_at  timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (session_id, question_id)
);

-- ===== opt-in audit (LGPD trail) =====

CREATE TABLE opt_in_log (
  id          bigserial PRIMARY KEY,
  nis         char(11) NOT NULL REFERENCES profiles(nis) ON DELETE CASCADE,
  opt_in      boolean NOT NULL,
  source      text    NOT NULL DEFAULT 'cras_presencial',
  operator    text,
  created_at  timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX opt_in_log_nis_idx ON opt_in_log (nis, created_at DESC);

-- ===== views the API can read directly =====

CREATE OR REPLACE VIEW v_stats_summary AS
SELECT
  (SELECT count(*) FROM messages WHERE status IN ('sent','delivered'))                                AS messages_total,
  (SELECT count(*) FROM messages WHERE status IN ('sent','delivered') AND sent_at::date = current_date) AS messages_today,
  (SELECT count(*) FROM appointments)                                                                 AS appointments_total,
  (SELECT count(*) FROM appointments WHERE created_at::date = current_date)                           AS appointments_today,
  (SELECT count(*) FROM profiles)                                                                     AS profiles_active,
  (SELECT count(*) FILTER (WHERE opt_in) FROM profiles)                                               AS opt_in_granted,
  (SELECT count(*) FROM profiles)                                                                     AS opt_in_total;

-- ===== seed (optional, demo) =====

INSERT INTO units (id, name, address, type) VALUES
  ('cras-centro', 'CRAS Centro', 'Rua das Flores, 100 — Centro',  'CRAS'),
  ('cras-norte',  'CRAS Norte',  'Av. Brasil, 2000 — Zona Norte', 'CRAS'),
  ('cras-sul',    'CRAS Sul',    'Rua do Sol, 50 — Zona Sul',     'CRAS'),
  ('creas',       'CREAS',       'Praça Central, 1 — Centro',     'CREAS')
ON CONFLICT (id) DO NOTHING;
