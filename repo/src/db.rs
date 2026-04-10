use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use tracing::{info, warn};

pub type DbPool = Pool<ConnectionManager<PgConnection>>;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

pub fn init_pool(database_url: &str) -> DbPool {
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    Pool::builder()
        // Large enough to handle 50 concurrent users with headroom
        .max_size(50)
        // Start with 1 idle connection; pool grows on demand
        .min_idle(Some(1))
        // Allow up to 30 s to acquire a connection before failing
        .connection_timeout(std::time::Duration::from_secs(30))
        // Recycle idle connections to avoid stale sockets after network blips
        .idle_timeout(Some(std::time::Duration::from_secs(600)))
        .build(manager)
        .expect("Failed to create database connection pool")
}

pub fn run_migrations(pool: &DbPool) {
    const MAX_ATTEMPTS: u32 = 20;
    const RETRY_DELAY_SECS: u64 = 3;

    for attempt in 1..=MAX_ATTEMPTS {
        match pool.get() {
            Ok(mut conn) => {
                conn.run_pending_migrations(MIGRATIONS)
                    .expect("Failed to run migrations");
                info!("Database migrations applied successfully");
                return;
            }
            Err(e) if attempt < MAX_ATTEMPTS => {
                warn!(
                    attempt,
                    max_attempts = MAX_ATTEMPTS,
                    retry_delay_secs = RETRY_DELAY_SECS,
                    error = %e,
                    "DB not ready — retrying"
                );
                std::thread::sleep(std::time::Duration::from_secs(RETRY_DELAY_SECS));
            }
            Err(e) => {
                panic!(
                    "Failed to connect to database after {} attempts: {}",
                    MAX_ATTEMPTS, e
                );
            }
        }
    }
}

/// Idempotent seed: inserts the minimum set of development/test users on first
/// startup.  Subsequent runs detect the existing `admin` user and skip.
///
/// Seeded credentials (change or rotate before any production use):
///   admin    / Admin1234!    — role: administrator
///   coach    / Coach1234!    — role: care_coach
///   approver / Approver1234! — role: approver
///   member   / Member1234!   — role: member  (+ member record)
pub fn seed_initial_data(pool: &DbPool) {
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(e) => {
            warn!("seed_initial_data: cannot get connection — {e}");
            return;
        }
    };

    // ── Guard: skip if already seeded ────────────────────────
    let already_seeded: bool = diesel::select(diesel::dsl::exists(
        crate::schema::users::table
            .filter(crate::schema::users::username.eq("admin")),
    ))
    .get_result(&mut conn)
    .unwrap_or(false);

    if already_seeded {
        info!("Seed data already present — skipping");
        return;
    }

    info!("Seeding initial users…");

    // Hash passwords using the app's Argon2id implementation.
    // Argon2 output contains only: $, alphanumeric, /, +, = — safe in SQL literals.
    let admin_hash    = crate::auth::passwords::hash("Admin1234!")
        .expect("seed: hash admin password");
    let coach_hash    = crate::auth::passwords::hash("Coach1234!")
        .expect("seed: hash coach password");
    let approver_hash = crate::auth::passwords::hash("Approver1234!")
        .expect("seed: hash approver password");
    let member_hash   = crate::auth::passwords::hash("Member1234!")
        .expect("seed: hash member password");

    let stmts: &[String] = &[
        // ── Org unit ─────────────────────────────────────────
        "INSERT INTO org_units (id, name, created_at, updated_at)
         VALUES ('10000000-0000-0000-0000-000000000001', 'VitalPath HQ', NOW(), NOW())
         ON CONFLICT (id) DO NOTHING"
            .to_string(),

        // ── Admin user ───────────────────────────────────────
        format!(
            "INSERT INTO users \
             (id, username, password_hash, role_id, org_unit_id, is_active, created_at, updated_at)
             VALUES ('20000000-0000-0000-0000-000000000001', 'admin', '{}',
                     '00000000-0000-0000-0000-000000000001',
                     '10000000-0000-0000-0000-000000000001', true, NOW(), NOW())
             ON CONFLICT (username) DO NOTHING",
            admin_hash
        ),

        // ── Care-coach user ──────────────────────────────────
        format!(
            "INSERT INTO users \
             (id, username, password_hash, role_id, org_unit_id, is_active, created_at, updated_at)
             VALUES ('20000000-0000-0000-0000-000000000002', 'coach', '{}',
                     '00000000-0000-0000-0000-000000000002',
                     '10000000-0000-0000-0000-000000000001', true, NOW(), NOW())
             ON CONFLICT (username) DO NOTHING",
            coach_hash
        ),

        // ── Approver user ─────────────────────────────────────
        format!(
            "INSERT INTO users \
             (id, username, password_hash, role_id, org_unit_id, is_active, created_at, updated_at)
             VALUES ('20000000-0000-0000-0000-000000000004', 'approver', '{}',
                     '00000000-0000-0000-0000-000000000003',
                     '10000000-0000-0000-0000-000000000001', true, NOW(), NOW())
             ON CONFLICT (username) DO NOTHING",
            approver_hash
        ),

        // ── Member user ──────────────────────────────────────
        format!(
            "INSERT INTO users \
             (id, username, password_hash, role_id, org_unit_id, is_active, created_at, updated_at)
             VALUES ('20000000-0000-0000-0000-000000000003', 'member', '{}',
                     '00000000-0000-0000-0000-000000000004',
                     '10000000-0000-0000-0000-000000000001', true, NOW(), NOW())
             ON CONFLICT (username) DO NOTHING",
            member_hash
        ),

        // ── Member record (links user → member for health/metric endpoints) ──
        "INSERT INTO members \
         (id, user_id, org_unit_id, first_name, last_name, date_of_birth, created_at, updated_at)
         VALUES ('30000000-0000-0000-0000-000000000001',
                 '20000000-0000-0000-0000-000000000003',
                 '10000000-0000-0000-0000-000000000001',
                 'Test', 'Member', '1990-01-15', NOW(), NOW())
         ON CONFLICT (user_id) DO NOTHING"
            .to_string(),
    ];

    for sql in stmts {
        if let Err(e) = diesel::sql_query(sql.as_str()).execute(&mut conn) {
            warn!("seed_initial_data: statement failed — {e}\nSQL: {sql}");
        }
    }

    info!(
        "Seed complete — admin=Admin1234!  coach=Coach1234!  approver=Approver1234!  member=Member1234! \
         (rotate these credentials before production use)"
    );
}
