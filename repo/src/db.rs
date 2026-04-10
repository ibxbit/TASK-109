use argon2::{Algorithm, Argon2, Params, Version};
use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use tracing::{info, warn};

pub type DbPool = Pool<ConnectionManager<PgConnection>>;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

pub fn init_pool(database_url: &str) -> DbPool {
    let mut attempts = 0;
    loop {
        attempts += 1;
        let manager = ConnectionManager::<PgConnection>::new(database_url);
        let result = Pool::builder()
            .max_size(50)
            .min_idle(Some(1))
            .connection_timeout(std::time::Duration::from_secs(30))
            .idle_timeout(Some(std::time::Duration::from_secs(600)))
            .build(manager);

        match result {
            Ok(pool) => return pool,
            Err(e) => {
                if attempts >= 20 {
                    panic!("Failed to create database connection pool after 20 attempts: {}", e);
                }
                eprintln!("[vitalpath] pool build failed (attempt {}/20), retrying in 3s... ({})", attempts, e);
                std::thread::sleep(std::time::Duration::from_secs(3));
            }
        }
    }
}

/// Lightweight DB connectivity check — tries to acquire a connection from the
/// pool and verifies the database is reachable before migrations are attempted.
/// Retries up to MAX_ATTEMPTS times; panics if the database never responds.
pub fn wait_for_db(pool: &DbPool) {
    const MAX_ATTEMPTS: u32 = 20;
    const RETRY_DELAY_SECS: u64 = 3;

    for attempt in 1..=MAX_ATTEMPTS {
        match pool.get() {
            Ok(_) => {
                eprintln!("[vitalpath] database ready (attempt {attempt}/{MAX_ATTEMPTS})");
                info!("Database connectivity confirmed");
                return;
            }
            Err(e) if attempt < MAX_ATTEMPTS => {
                eprintln!("[vitalpath] DB not ready (attempt {attempt}/{MAX_ATTEMPTS}): {e}");
                warn!(
                    attempt,
                    max_attempts = MAX_ATTEMPTS,
                    error = %e,
                    "DB not ready — retrying"
                );
                std::thread::sleep(std::time::Duration::from_secs(RETRY_DELAY_SECS));
            }
            Err(e) => {
                eprintln!(
                    "[vitalpath] FATAL: database not available after {MAX_ATTEMPTS} attempts: {e}"
                );
                panic!("Database not available after {MAX_ATTEMPTS} attempts: {e}");
            }
        }
    }
}

pub fn run_migrations(pool: &DbPool) {
    const MAX_ATTEMPTS: u32 = 20;
    const RETRY_DELAY_SECS: u64 = 3;

    for attempt in 1..=MAX_ATTEMPTS {
        match pool.get() {
            Ok(mut conn) => {
                match conn.run_pending_migrations(MIGRATIONS) {
                    Ok(_) => {
                        info!("Database migrations applied successfully");
                        return;
                    }
                    Err(e) => {
                        eprintln!("[vitalpath] FATAL: migration failed: {e}");
                        panic!("Failed to run migrations: {e}");
                    }
                }
            }
            Err(e) if attempt < MAX_ATTEMPTS => {
                eprintln!("[vitalpath] DB not ready (attempt {attempt}/{MAX_ATTEMPTS}): {e}");
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
                eprintln!("[vitalpath] FATAL: could not connect to DB after {MAX_ATTEMPTS} attempts: {e}");
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

    // See seed_params below for the actual Argon2id parameters used.
    // Use minimum Argon2id params for seed data (m=1 MiB, t=1 iter, p=1 thread).
    // This keeps peak memory at ~1 MiB per hash in memory-constrained sandboxes.
    // PHC strings store the params, so verify() reads them correctly at login.
    let seed_params = match Params::new(1024, 1, 1, None) {
        Ok(p) => p,
        Err(e) => {
            warn!("seed_initial_data: failed to build Argon2 params — {e}; skipping seed");
            return;
        }
    };
    let seed_argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, seed_params);

    let hash_seed = |password: &str| -> Option<String> {
        use argon2::password_hash::{rand_core::OsRng, PasswordHasher, SaltString};
        let salt = SaltString::generate(&mut OsRng);
        match seed_argon2.hash_password(password.as_bytes(), &salt) {
            Ok(h) => Some(h.to_string()),
            Err(e) => {
                warn!("seed_initial_data: failed to hash password — {e}; skipping seed");
                None
            }
        }
    };

    let admin_hash    = match hash_seed("Admin1234!")    { Some(h) => h, None => return };
    let coach_hash    = match hash_seed("Coach1234!")    { Some(h) => h, None => return };
    let approver_hash = match hash_seed("Approver1234!") { Some(h) => h, None => return };
    let member_hash   = match hash_seed("Member1234!")   { Some(h) => h, None => return };

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
