use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct SmsMessage {
    pub id: String,
    pub sender: String,
    pub content: String,
    pub received_at: i64,
    pub metas: String,
}

pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn new(path: &str) -> Result<Self> {
        let conn = Connection::open(path).context(format!("Failed to open database: {}", path))?;

        // Create table if not exists
        conn.execute(
            "CREATE TABLE IF NOT EXISTS sms_messages (
                id TEXT PRIMARY KEY,
                sender TEXT NOT NULL,
                content TEXT NOT NULL,
                received_at INTEGER NOT NULL,
                metas TEXT,
                acknowledged INTEGER NOT NULL DEFAULT 0,
                ack_sent_at INTEGER,
                created_at INTEGER NOT NULL
            )",
            [],
        )
        .context("Failed to create sms_messages table")?;

        log::info!("Database initialized at: {}", path);

        Ok(Database {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn insert_sms(&self, msg: &SmsMessage) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs() as i64;

        conn.execute(
            "INSERT INTO sms_messages (id, sender, content, received_at, metas, acknowledged, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6)",
            params![
                &msg.id,
                &msg.sender,
                &msg.content,
                msg.received_at,
                &msg.metas,
                created_at,
            ],
        ).context(format!("Failed to insert SMS message: {}", msg.id))?;

        log::info!("SMS message inserted into database: {}", msg.id);
        Ok(())
    }

    pub fn mark_acknowledged(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let ack_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let rows_affected = conn
            .execute(
                "UPDATE sms_messages SET acknowledged = 1, ack_sent_at = ?1 WHERE id = ?2",
                params![ack_time, id],
            )
            .context(format!("Failed to mark message as acknowledged: {}", id))?;

        if rows_affected > 0 {
            log::info!("SMS message marked as acknowledged: {}", id);
        } else {
            log::warn!("No message found with id: {}", id);
        }

        Ok(())
    }

    pub fn get_unacknowledged(&self) -> Result<Vec<SmsMessage>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, sender, content, received_at, metas FROM sms_messages WHERE acknowledged = 0"
        ).context("Failed to prepare query for unacknowledged messages")?;

        let messages = stmt
            .query_map([], |row| {
                Ok(SmsMessage {
                    id: row.get(0)?,
                    sender: row.get(1)?,
                    content: row.get(2)?,
                    received_at: row.get(3)?,
                    metas: row.get(4)?,
                })
            })
            .context("Failed to query unacknowledged messages")?;

        let result: Result<Vec<_>, _> = messages.collect();
        result.context("Failed to collect unacknowledged messages")
    }

    pub fn count_total(&self) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM sms_messages", [], |row| row.get(0))
            .context("Failed to count total messages")?;

        Ok(count)
    }

    pub fn count_unacknowledged(&self) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sms_messages WHERE acknowledged = 0",
                [],
                |row| row.get(0),
            )
            .context("Failed to count unacknowledged messages")?;

        Ok(count)
    }
}

// Implement Clone manually since Connection isn't Clone
impl Clone for Database {
    fn clone(&self) -> Self {
        Database {
            conn: Arc::clone(&self.conn),
        }
    }
}
