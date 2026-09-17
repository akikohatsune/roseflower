use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{FromRow, SqlitePool};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;
use tracing::info;

pub type DbPool = SqlitePool;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveSlotInfo {
    pub slot_id: usize,
    pub user_id: i32,
    pub username: String,
    pub status: u8,
    pub status_text: String,
    pub team: u8,
    pub mods: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DbLiveRoom {
    pub match_id: i64,
    pub name: String,
    pub host_id: i64,
    pub host_name: String,
    pub beatmap_id: i64,
    pub beatmap_name: String,
    pub beatmap_md5: String,
    pub mode: i64,
    pub scoring_type: i64,
    pub team_type: i64,
    pub mods: i64,
    pub in_progress: i64,
    pub player_count: i64,
    pub slots_json: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DbMultiGame {
    pub id: i64,
    pub match_id: i64,
    pub beatmap_id: i64,
    pub beatmap_name: String,
    pub beatmap_md5: String,
    pub mode: i64,
    pub scoring_type: i64,
    pub team_type: i64,
    pub mods: i64,
    pub played_at: i64,
    pub duration_seconds: i64,
    pub winner_id: i64,
    pub winner_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DbMultiScore {
    pub id: i64,
    pub game_id: i64,
    pub match_id: i64,
    pub user_id: i64,
    pub username: String,
    pub slot_id: i64,
    pub team: i64,
    pub score: i64,
    pub max_combo: i64,
    pub accuracy: f64,
    pub c300: i64,
    pub c100: i64,
    pub c50: i64,
    pub c_miss: i64,
    pub c_geki: i64,
    pub c_katu: i64,
    pub passed: i64,
    pub won: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiGameWithScores {
    pub game: DbMultiGame,
    pub scores: Vec<DbMultiScore>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveRoomDetails {
    pub room: DbLiveRoom,
    pub slots: Vec<LiveSlotInfo>,
    pub games: Vec<MultiGameWithScores>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiStatsSummary {
    pub active_rooms: usize,
    pub total_players: i64,
    pub total_completed_rounds: usize,
}

pub async fn init_db(db_path: &str) -> Result<DbPool, sqlx::Error> {
    if let Some(parent) = Path::new(db_path).parent() {
        if !parent.exists() {
            let _ = fs::create_dir_all(parent);
        }
    }

    let connect_opts = SqliteConnectOptions::from_str(&format!("sqlite://{}", db_path))?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(Duration::from_secs(5));

    let pool = SqlitePoolOptions::new()
        .max_connections(10)
        .connect_with(connect_opts)
        .await?;

    // Create tables
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS multi_rooms (
            match_id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            host_id INTEGER NOT NULL,
            host_name TEXT NOT NULL,
            beatmap_id INTEGER NOT NULL,
            beatmap_name TEXT NOT NULL,
            beatmap_md5 TEXT NOT NULL,
            mode INTEGER NOT NULL,
            scoring_type INTEGER NOT NULL,
            team_type INTEGER NOT NULL,
            mods INTEGER NOT NULL,
            in_progress INTEGER NOT NULL,
            player_count INTEGER NOT NULL,
            slots_json TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_multi_rooms_updated ON multi_rooms(updated_at DESC);

        CREATE TABLE IF NOT EXISTS multi_games (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            match_id INTEGER NOT NULL,
            beatmap_id INTEGER NOT NULL,
            beatmap_name TEXT NOT NULL,
            beatmap_md5 TEXT NOT NULL,
            mode INTEGER NOT NULL,
            scoring_type INTEGER NOT NULL,
            team_type INTEGER NOT NULL,
            mods INTEGER NOT NULL,
            played_at INTEGER NOT NULL,
            duration_seconds INTEGER NOT NULL,
            winner_id INTEGER NOT NULL DEFAULT 0,
            winner_name TEXT NOT NULL DEFAULT ''
        );
        CREATE INDEX IF NOT EXISTS idx_multi_games_match ON multi_games(match_id, played_at DESC);

        CREATE TABLE IF NOT EXISTS multi_scores (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            game_id INTEGER NOT NULL,
            match_id INTEGER NOT NULL,
            user_id INTEGER NOT NULL,
            username TEXT NOT NULL,
            slot_id INTEGER NOT NULL,
            team INTEGER NOT NULL,
            score INTEGER NOT NULL,
            max_combo INTEGER NOT NULL,
            accuracy REAL NOT NULL,
            c300 INTEGER NOT NULL,
            c100 INTEGER NOT NULL,
            c50 INTEGER NOT NULL,
            c_miss INTEGER NOT NULL,
            c_geki INTEGER NOT NULL,
            c_katu INTEGER NOT NULL,
            passed INTEGER NOT NULL,
            won INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_multi_scores_game ON multi_scores(game_id);
        "#,
    )
    .execute(&pool)
    .await?;

    info!("Roseflower multi database initialized at {}", db_path);
    Ok(pool)
}

pub async fn upsert_room(pool: &DbPool, room: &DbLiveRoom) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO multi_rooms (
            match_id, name, host_id, host_name, beatmap_id, beatmap_name,
            beatmap_md5, mode, scoring_type, team_type, mods, in_progress,
            player_count, slots_json, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
        ON CONFLICT(match_id) DO UPDATE SET
            name=excluded.name,
            host_id=excluded.host_id,
            host_name=excluded.host_name,
            beatmap_id=excluded.beatmap_id,
            beatmap_name=excluded.beatmap_name,
            beatmap_md5=excluded.beatmap_md5,
            mode=excluded.mode,
            scoring_type=excluded.scoring_type,
            team_type=excluded.team_type,
            mods=excluded.mods,
            in_progress=excluded.in_progress,
            player_count=excluded.player_count,
            slots_json=excluded.slots_json,
            updated_at=excluded.updated_at
        "#,
    )
    .bind(room.match_id)
    .bind(&room.name)
    .bind(room.host_id)
    .bind(&room.host_name)
    .bind(room.beatmap_id)
    .bind(&room.beatmap_name)
    .bind(&room.beatmap_md5)
    .bind(room.mode)
    .bind(room.scoring_type)
    .bind(room.team_type)
    .bind(room.mods)
    .bind(room.in_progress)
    .bind(room.player_count)
    .bind(&room.slots_json)
    .bind(room.created_at)
    .bind(room.updated_at)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn delete_room(pool: &DbPool, match_id: u16) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM multi_rooms WHERE match_id = ?1")
        .bind(match_id as i64)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_all_live_rooms(pool: &DbPool) -> Result<Vec<LiveRoomDetails>, sqlx::Error> {
    let rows: Vec<DbLiveRoom> = sqlx::query_as(
        "SELECT * FROM multi_rooms ORDER BY updated_at DESC"
    )
    .fetch_all(pool)
    .await?;

    let mut result = Vec::new();
    for room in rows {
        let slots: Vec<LiveSlotInfo> = serde_json::from_str(&room.slots_json).unwrap_or_default();
        let games = get_match_games(pool, room.match_id as u16).await.unwrap_or_default();

        result.push(LiveRoomDetails {
            room,
            slots,
            games,
        });
    }

    Ok(result)
}

pub async fn get_room_by_id(pool: &DbPool, match_id: u16) -> Result<Option<LiveRoomDetails>, sqlx::Error> {
    let row: Option<DbLiveRoom> = sqlx::query_as(
        "SELECT * FROM multi_rooms WHERE match_id = ?1"
    )
    .bind(match_id as i64)
    .fetch_optional(pool)
    .await?;

    match row {
        Some(room) => {
            let slots: Vec<LiveSlotInfo> = serde_json::from_str(&room.slots_json).unwrap_or_default();
            let games = get_match_games(pool, match_id).await.unwrap_or_default();
            Ok(Some(LiveRoomDetails {
                room,
                slots,
                games,
            }))
        }
        None => Ok(None),
    }
}

pub async fn get_match_games(pool: &DbPool, match_id: u16) -> Result<Vec<MultiGameWithScores>, sqlx::Error> {
    let games: Vec<DbMultiGame> = sqlx::query_as(
        "SELECT * FROM multi_games WHERE match_id = ?1 ORDER BY played_at DESC LIMIT 50"
    )
    .bind(match_id as i64)
    .fetch_all(pool)
    .await?;

    if games.is_empty() {
        return Ok(Vec::new());
    }

    let game_ids: Vec<i64> = games.iter().map(|g| g.id).collect();
    let query_str = format!(
        "SELECT * FROM multi_scores WHERE game_id IN ({}) ORDER BY score DESC",
        game_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",")
    );

    let all_scores: Vec<DbMultiScore> = sqlx::query_as(&query_str)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

    let mut scores_by_game: HashMap<i64, Vec<DbMultiScore>> = HashMap::new();
    for score in all_scores {
        scores_by_game.entry(score.game_id).or_default().push(score);
    }

    let result = games
        .into_iter()
        .map(|game| {
            let scores = scores_by_game.remove(&game.id).unwrap_or_default();
            MultiGameWithScores { game, scores }
        })
        .collect();

    Ok(result)
}

pub async fn record_game_and_scores(
    pool: &DbPool,
    game: &DbMultiGame,
    scores: &[DbMultiScore],
) -> Result<i64, sqlx::Error> {
    let mut tx = pool.begin().await?;

    let row = sqlx::query(
        r#"
        INSERT INTO multi_games (
            match_id, beatmap_id, beatmap_name, beatmap_md5, mode, scoring_type,
            team_type, mods, played_at, duration_seconds, winner_id, winner_name
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
        RETURNING id
        "#,
    )
    .bind(game.match_id)
    .bind(game.beatmap_id)
    .bind(&game.beatmap_name)
    .bind(&game.beatmap_md5)
    .bind(game.mode)
    .bind(game.scoring_type)
    .bind(game.team_type)
    .bind(game.mods)
    .bind(game.played_at)
    .bind(game.duration_seconds)
    .bind(game.winner_id)
    .bind(&game.winner_name)
    .fetch_one(&mut *tx)
    .await?;

    let game_id: i64 = sqlx::Row::get(&row, "id");

    for s in scores {
        sqlx::query(
            r#"
            INSERT INTO multi_scores (
                game_id, match_id, user_id, username, slot_id, team, score,
                max_combo, accuracy, c300, c100, c50, c_miss, c_geki, c_katu,
                passed, won
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
            "#,
        )
        .bind(game_id)
        .bind(s.match_id)
        .bind(s.user_id)
        .bind(&s.username)
        .bind(s.slot_id)
        .bind(s.team)
        .bind(s.score)
        .bind(s.max_combo)
        .bind(s.accuracy)
        .bind(s.c300)
        .bind(s.c100)
        .bind(s.c50)
        .bind(s.c_miss)
        .bind(s.c_geki)
        .bind(s.c_katu)
        .bind(s.passed)
        .bind(s.won)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(game_id)
}

pub async fn get_stats(pool: &DbPool) -> Result<MultiStatsSummary, sqlx::Error> {
    let rooms: Vec<DbLiveRoom> = sqlx::query_as("SELECT * FROM multi_rooms").fetch_all(pool).await?;
    let active_rooms = rooms.len();
    let total_players: i64 = rooms.iter().map(|r| r.player_count).sum();

    let total_completed_rounds: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM multi_games")
        .fetch_one(pool)
        .await
        .unwrap_or(0);

    Ok(MultiStatsSummary {
        active_rooms,
        total_players,
        total_completed_rounds: total_completed_rounds as usize,
    })
}
