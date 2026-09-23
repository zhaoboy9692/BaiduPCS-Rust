use crate::error::Error;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::RngCore;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{path::Path, sync::Mutex, time::Duration};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateToken {
    pub name: String,
    #[serde(default)]
    pub note: String,
    pub expires_at: Option<i64>,
    #[serde(default = "default_rate")]
    pub rate_per_minute: i64,
}
fn default_rate() -> i64 {
    10
}
// Missing preserves current value; explicit JSON null sets permanent validity.
fn expiry<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Option<i64>>, D::Error> {
    Option::<i64>::deserialize(d).map(Some)
}
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditToken {
    pub name: Option<String>,
    pub note: Option<String>,
    pub enabled: Option<bool>,
    #[serde(default, deserialize_with = "expiry")]
    pub expires_at: Option<Option<i64>>,
    pub rate_per_minute: Option<i64>,
}
#[derive(Debug, Serialize, Clone)]
pub struct TokenRecord {
    pub id: String,
    pub name: String,
    pub note: String,
    pub display_prefix: String,
    pub enabled: bool,
    pub revoked_at: Option<i64>,
    pub expires_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_used_at: Option<i64>,
    pub rate_per_minute: i64,
    pub success_count: i64,
    pub failure_count: i64,
}
#[derive(Debug, Serialize)]
pub struct IssuedToken {
    pub token: String,
    pub record: TokenRecord,
}
pub struct Store {
    connection: Mutex<Connection>,
}
const COLUMNS:&str="id,name,note,display_prefix,enabled,revoked_at,expires_at,created_at,updated_at,last_used_at,rate_per_minute,success_count,failure_count";
fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<TokenRecord> {
    Ok(TokenRecord {
        id: r.get(0)?,
        name: r.get(1)?,
        note: r.get(2)?,
        display_prefix: r.get(3)?,
        enabled: r.get(4)?,
        revoked_at: r.get(5)?,
        expires_at: r.get(6)?,
        created_at: r.get(7)?,
        updated_at: r.get(8)?,
        last_used_at: r.get(9)?,
        rate_per_minute: r.get(10)?,
        success_count: r.get(11)?,
        failure_count: r.get(12)?,
    })
}
fn digest(secret: &str) -> String {
    format!("{:x}", Sha256::digest(secret.as_bytes()))
}
fn validate(name: &str, note: &str, rate: i64) -> Result<(), Error> {
    if name.trim().is_empty()
        || name.chars().count() > 80
        || note.chars().count() > 500
        || !(1..=60).contains(&rate)
    {
        return Err(Error::Input);
    }
    Ok(())
}
fn get(conn: &Connection, id: &str) -> Result<TokenRecord, Error> {
    conn.query_row(
        &format!("SELECT {COLUMNS} FROM api_tokens WHERE id=?1"),
        [id],
        row,
    )
    .optional()?
    .ok_or(Error::NotFound)
}
fn authorize(conn: &Connection, secret: &str, now: i64) -> Result<TokenRecord, Error> {
    if secret.len() != 46 || !secret.starts_with("dl_") {
        return Err(Error::InvalidToken);
    }
    let t = conn
        .query_row(
            &format!("SELECT {COLUMNS} FROM api_tokens WHERE secret_hash=?1"),
            [digest(secret)],
            row,
        )
        .optional()?
        .ok_or(Error::InvalidToken)?;
    if t.revoked_at.is_some() {
        return Err(Error::InvalidToken);
    }
    if !t.enabled {
        return Err(Error::Disabled);
    }
    if t.expires_at.is_some_and(|expires| now >= expires) {
        return Err(Error::Expired);
    }
    Ok(t)
}
impl Store {
    pub fn record_result(&self, id: &str, success: bool) -> Result<(), Error> {
        let conn = self.connection.lock().map_err(|_| Error::Storage)?;
        let count=conn.execute("UPDATE api_tokens SET success_count=success_count+?1,failure_count=failure_count+?2 WHERE id=?3",params![i64::from(success),i64::from(!success),id])?;
        if count != 1 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        Self::initialize(Connection::open(path)?)
    }
    pub fn open_memory() -> Result<Self, Error> {
        Self::initialize(Connection::open_in_memory()?)
    }
    fn initialize(conn: Connection) -> Result<Self, Error> {
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;
        CREATE TABLE IF NOT EXISTS api_tokens(
          id TEXT PRIMARY KEY,name TEXT NOT NULL,note TEXT NOT NULL,display_prefix TEXT NOT NULL,
          secret_hash TEXT UNIQUE NOT NULL,enabled INTEGER NOT NULL DEFAULT 1,
          revoked_at INTEGER,expires_at INTEGER,created_at INTEGER NOT NULL,updated_at INTEGER NOT NULL,
          last_used_at INTEGER,rate_per_minute INTEGER NOT NULL CHECK(rate_per_minute BETWEEN 1 AND 60),
          success_count INTEGER NOT NULL DEFAULT 0,failure_count INTEGER NOT NULL DEFAULT 0);
        CREATE TABLE IF NOT EXISTS rate_windows(
          token_id TEXT NOT NULL REFERENCES api_tokens(id),bucket TEXT NOT NULL,window INTEGER NOT NULL,
          count INTEGER NOT NULL,PRIMARY KEY(token_id,bucket));")?;
        Ok(Self {
            connection: Mutex::new(conn),
        })
    }
    pub fn create(&self, input: CreateToken, now: i64) -> Result<IssuedToken, Error> {
        validate(&input.name, &input.note, input.rate_per_minute)?;
        if input.expires_at.is_some_and(|v| v <= now) {
            return Err(Error::Input);
        }
        let mut bytes = [0u8; 32];
        rand::rngs::OsRng
            .try_fill_bytes(&mut bytes)
            .map_err(|_| Error::Storage)?;
        let token = format!("dl_{}", URL_SAFE_NO_PAD.encode(bytes));
        let id = uuid::Uuid::new_v4().to_string();
        let c = self.connection.lock().map_err(|_| Error::Storage)?;
        c.execute("INSERT INTO api_tokens(id,name,note,display_prefix,secret_hash,expires_at,created_at,updated_at,rate_per_minute) VALUES(?1,?2,?3,?4,?5,?6,?7,?7,?8)",params![id,input.name.trim(),input.note,&token[..11],digest(&token),input.expires_at,now,input.rate_per_minute])?;
        Ok(IssuedToken {
            token,
            record: get(&c, &id)?,
        })
    }
    pub fn list(&self, offset: i64, limit: i64) -> Result<Vec<TokenRecord>, Error> {
        if offset < 0 || !(1..=100).contains(&limit) {
            return Err(Error::Input);
        }
        let c = self.connection.lock().map_err(|_| Error::Storage)?;
        let mut stmt = c.prepare(&format!(
            "SELECT {COLUMNS} FROM api_tokens ORDER BY created_at DESC,id LIMIT ?1 OFFSET ?2"
        ))?;
        let result = stmt
            .query_map(params![limit, offset], row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(result)
    }
    pub fn edit(&self, id: &str, edit: EditToken, now: i64) -> Result<TokenRecord, Error> {
        let mut c = self.connection.lock().map_err(|_| Error::Storage)?;
        let tx = c.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let old = get(&tx, id)?;
        if old.revoked_at.is_some() {
            return Err(Error::Revoked);
        }
        let name = edit.name.unwrap_or(old.name);
        let note = edit.note.unwrap_or(old.note);
        let rate = edit.rate_per_minute.unwrap_or(old.rate_per_minute);
        validate(&name, &note, rate)?;
        if edit.expires_at.flatten().is_some_and(|v| v <= now) {
            return Err(Error::Input);
        }
        let expires = edit.expires_at.unwrap_or(old.expires_at);
        tx.execute("UPDATE api_tokens SET name=?1,note=?2,rate_per_minute=?3,expires_at=?4,enabled=?5,updated_at=?6 WHERE id=?7",params![name.trim(),note,rate,expires,edit.enabled.unwrap_or(old.enabled),now,id])?;
        let result = get(&tx, id)?;
        tx.commit()?;
        Ok(result)
    }
    pub fn revoke(&self, id: &str, now: i64) -> Result<TokenRecord, Error> {
        let c = self.connection.lock().map_err(|_| Error::Storage)?;
        let changed=c.execute("UPDATE api_tokens SET revoked_at=COALESCE(revoked_at,?1),enabled=0,updated_at=?1 WHERE id=?2",params![now,id])?;
        if changed == 0 {
            return Err(Error::NotFound);
        }
        get(&c, id)
    }
    pub fn authorize(&self, secret: &str, now: i64) -> Result<TokenRecord, Error> {
        let c = self.connection.lock().map_err(|_| Error::Storage)?;
        authorize(&c, secret, now)
    }
    pub fn admit(&self, secret: &str, now: i64, read: bool) -> Result<TokenRecord, Error> {
        let mut c = self.connection.lock().map_err(|_| Error::Storage)?;
        let tx = c.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut t = authorize(&tx, secret, now)?;
        let bucket = if read { "read" } else { "resolve" };
        let window = now / 60;
        let limit = if read { 60 } else { t.rate_per_minute };
        let current: Option<(i64, i64)> = tx
            .query_row(
                "SELECT window,count FROM rate_windows WHERE token_id=?1 AND bucket=?2",
                params![t.id, bucket],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let count = current.filter(|(w, _)| *w == window).map_or(0, |(_, n)| n);
        if count >= limit {
            return Err(Error::Limited);
        }
        tx.execute("INSERT INTO rate_windows VALUES(?1,?2,?3,?4) ON CONFLICT(token_id,bucket) DO UPDATE SET window=excluded.window,count=excluded.count",params![t.id,bucket,window,count+1])?;
        tx.execute(
            "UPDATE api_tokens SET last_used_at=?1 WHERE id=?2",
            params![now, t.id],
        )?;
        tx.commit()?;
        t.last_used_at = Some(now);
        Ok(t)
    }
}
