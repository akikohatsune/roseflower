use crate::config::Config;
use crate::db::{self, DbLiveRoom, DbMultiGame, DbMultiScore, DbPool};
use axum::extract::{Path as AxPath, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Json, Response};
use axum::routing::{get, post};
use axum::Router;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db: DbPool,
    pub config: Arc<Config>,
}

#[derive(Serialize)]
struct ApiResponse<T> {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<T>,
}

#[derive(Deserialize)]
pub struct DisbandRequest {
    pub match_id: u16,
}

#[derive(Deserialize)]
pub struct FinishMatchRequest {
    pub game: DbMultiGame,
    pub scores: Vec<DbMultiScore>,
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(multi_page_handler))
        .route("/multi", get(multi_page_handler))
        .route("/health", get(health_check))
        .route("/api/multi/rooms", get(list_rooms_api))
        .route("/api/multi/rooms/{id}", get(get_room_api))
        .route("/api/multi/stats", get(get_stats_api))
        .route("/api/multi/sync", post(sync_room_api))
        .route("/api/multi/disband", post(disband_room_api))
        .route("/api/multi/finish", post(finish_match_api))
        .with_state(state)
}

async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "service": "roseflower",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

async fn list_rooms_api(State(state): State<AppState>) -> Response {
    match db::get_all_live_rooms(&state.db).await {
        Ok(rooms) => Json(ApiResponse {
            success: true,
            message: None,
            data: Some(rooms),
        })
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()> {
                success: false,
                message: Some(e.to_string()),
                data: None,
            }),
        )
            .into_response(),
    }
}

async fn get_room_api(
    State(state): State<AppState>,
    AxPath(match_id): AxPath<u16>,
) -> Response {
    match db::get_room_by_id(&state.db, match_id).await {
        Ok(Some(room)) => Json(ApiResponse {
            success: true,
            message: None,
            data: Some(room),
        })
        .into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<()> {
                success: false,
                message: Some("Room not found".to_string()),
                data: None,
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()> {
                success: false,
                message: Some(e.to_string()),
                data: None,
            }),
        )
            .into_response(),
    }
}

async fn get_stats_api(State(state): State<AppState>) -> Response {
    match db::get_stats(&state.db).await {
        Ok(stats) => Json(ApiResponse {
            success: true,
            message: None,
            data: Some(stats),
        })
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()> {
                success: false,
                message: Some(e.to_string()),
                data: None,
            }),
        )
            .into_response(),
    }
}

async fn sync_room_api(
    State(state): State<AppState>,
    Json(payload): Json<DbLiveRoom>,
) -> Response {
    match db::upsert_room(&state.db, &payload).await {
        Ok(_) => Json(serde_json::json!({ "success": true })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "success": false, "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn disband_room_api(
    State(state): State<AppState>,
    Json(payload): Json<DisbandRequest>,
) -> Response {
    match db::delete_room(&state.db, payload.match_id).await {
        Ok(_) => Json(serde_json::json!({ "success": true })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "success": false, "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn finish_match_api(
    State(state): State<AppState>,
    Json(payload): Json<FinishMatchRequest>,
) -> Response {
    match db::record_game_and_scores(&state.db, &payload.game, &payload.scores).await {
        Ok(game_id) => Json(serde_json::json!({ "success": true, "game_id": game_id })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "success": false, "error": e.to_string() })),
        )
            .into_response(),
    }
}

// -------------------------------------------------------------------------------------------------
// Multiplayer HTML Dashboard
// -------------------------------------------------------------------------------------------------

fn get_multi_mode_name(mode: i64) -> &'static str {
    match mode {
        0 => "osu! Standard",
        1 => "Taiko",
        2 => "Catch",
        3 => "osu!mania",
        _ => "osu!",
    }
}

fn get_multi_scoring_name(st: i64) -> &'static str {
    match st {
        0 => "Score",
        1 => "Accuracy",
        2 => "Combo",
        3 => "Score V2",
        _ => "Score",
    }
}

fn get_multi_team_name(tt: i64) -> &'static str {
    match tt {
        0 => "Head to Head",
        1 => "Tag Co-op",
        2 => "Team VS",
        3 => "Tag Team VS",
        _ => "Head to Head",
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

fn format_number(n: i64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    let mut count = 0;
    for c in s.chars().rev() {
        if count > 0 && count % 3 == 0 {
            result.push(',');
        }
        result.push(c);
        count += 1;
    }
    result.chars().rev().collect()
}

async fn multi_page_handler(State(state): State<AppState>) -> Html<String> {
    let rooms = db::get_all_live_rooms(&state.db).await.unwrap_or_default();
    let stats = db::get_stats(&state.db).await.unwrap_or(db::MultiStatsSummary {
        active_rooms: rooms.len(),
        total_players: rooms.iter().map(|r| r.room.player_count).sum(),
        total_completed_rounds: 0,
    });

    let mut rooms_html = String::new();
    if rooms.is_empty() {
        rooms_html.push_str(r###"
        <div style="text-align: center; padding: 4rem 2rem; background: rgba(30, 41, 59, 0.45); border: 1px solid rgba(255, 255, 255, 0.1); border-radius: 12px; margin-top: 1rem; backdrop-filter: blur(12px);">
            <div style="width: 64px; height: 64px; margin: 0 auto 1.2rem auto; border-radius: 50%; background: rgba(244, 114, 182, 0.15); display: flex; align-items: center; justify-content: center; border: 1px solid rgba(244, 114, 182, 0.3); color: #f472b6;">
                <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M6 11h4M8 9v4M15 12h.01M18 10h.01M17.32 5H6.68a4 4 0 0 0-3.978 3.59c-.006.052-.01.101-.017.152C2.604 9.416 2 14.456 2 16a3 3 0 0 0 3 3c1 0 1.5-.5 2-1l1.414-1.414A2 2 0 0 1 9.828 16h4.344a2 2 0 0 1 1.414.586L17 18c.5.5 1 1 2 1a3 3 0 0 0 3-3c0-1.545-.604-6.584-.685-7.258-.007-.05-.011-.1-.017-.151A4 4 0 0 0 17.32 5z"></path></svg>
            </div>
            <h3 style="font-size: 1.4rem; font-weight: 800; color: #fff; margin-bottom: 0.5rem;">No Active Multiplayer Rooms</h3>
            <p style="color: #94a3b8; font-size: 0.95rem; max-width: 520px; margin: 0 auto 1.5rem auto; line-height: 1.6;">
                There are currently no active lobbies on the server. Connect via your osu! client, create or join a room in the multiplayer lobby, and watch live tracking here!
            </p>
        </div>
        "###);
    } else {
        for item in &rooms {
            let r = &item.room;
            let status_badge = if r.in_progress == 1 {
                r###"<span class="multi-badge playing"><span class="pulse-dot red"></span> IN MATCH</span>"###
            } else {
                r###"<span class="multi-badge waiting"><span class="pulse-dot green"></span> WAITING IN LOBBY</span>"###
            };

            let map_link = if r.beatmap_id > 0 {
                format!(
                    r###"<a href="https://osu.ppy.sh/b/{}" target="_blank" rel="noopener noreferrer" style="color: #38bdf8; text-decoration: none; font-weight: 600;">{}</a>"###,
                    r.beatmap_id, html_escape(&r.beatmap_name)
                )
            } else {
                format!(r###"<span style="color: #94a3b8;">{}</span>"###, html_escape(&r.beatmap_name))
            };

            let mut slots_html = String::new();
            for slot in &item.slots {
                let status_pill = match slot.status {
                    1 => r#"<span style="color: #94a3b8; font-size: 0.75rem;">Not Ready</span>"#,
                    2 => r#"<span style="color: #4ade80; font-weight: 700; font-size: 0.75rem;">✓ Ready</span>"#,
                    4 => r#"<span style="color: #f472b6; font-weight: 700; font-size: 0.75rem;">Playing</span>"#,
                    8 => r#"<span style="color: #38bdf8; font-weight: 700; font-size: 0.75rem;">Completed</span>"#,
                    _ => r#"<span style="color: #64748b; font-size: 0.75rem;">Open</span>"#,
                };

                let team_dot = match slot.team {
                    1 => r#"<span style="display:inline-block; width:8px; height:8px; border-radius:50%; background:#ef4444; margin-right:4px;" title="Blue Team"></span>"#,
                    2 => r#"<span style="display:inline-block; width:8px; height:8px; border-radius:50%; background:#3b82f6; margin-right:4px;" title="Red Team"></span>"#,
                    _ => "",
                };

                slots_html.push_str(&format!(
                    r###"
                    <div style="background: rgba(15, 23, 42, 0.6); padding: 0.6rem 0.8rem; border-radius: 8px; border: 1px solid rgba(255,255,255,0.06); display: flex; align-items: center; justify-content: space-between; gap: 0.5rem;">
                        <div style="display: flex; align-items: center; gap: 0.5rem; overflow: hidden;">
                            <span style="font-family: monospace; font-size: 0.8rem; color: #64748b;">#{:02}</span>
                            {team_dot}
                            <span style="font-weight: 600; font-size: 0.85rem; color: #f8fafc; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 130px;">{}</span>
                        </div>
                        <div>{status_pill}</div>
                    </div>
                    "###,
                    slot.slot_id + 1,
                    html_escape(&slot.username),
                ));
            }

            let mut match_history_html = String::new();
            if !item.games.is_empty() {
                match_history_html.push_str(r###"
                <div style="margin-top: 1.2rem; border-top: 1px solid rgba(255,255,255,0.08); padding-top: 1rem;">
                    <div style="font-size: 0.85rem; font-weight: 700; color: #94a3b8; text-transform: uppercase; letter-spacing: 0.5px; margin-bottom: 0.6rem;">Recent Rounds Played</div>
                    <div style="display: flex; flex-direction: column; gap: 0.6rem;">
                "###);

                for (idx, game_with_scores) in item.games.iter().enumerate() {
                    let g = &game_with_scores.game;
                    let winner_badge = if !g.winner_name.is_empty() {
                        format!(r###"<span style="color: #fbbf24; font-weight: 700; font-size: 0.8rem;">👑 Winner: {}</span>"###, html_escape(&g.winner_name))
                    } else {
                        String::new()
                    };

                    let mut scores_summary = String::new();
                    for s in &game_with_scores.scores {
                        let pass_str = if s.passed == 1 { "✓" } else { "✗" };
                        scores_summary.push_str(&format!(
                            r###"<span style="font-size: 0.78rem; color: #cbd5e1; background: rgba(0,0,0,0.3); padding: 0.2rem 0.5rem; border-radius: 4px;">{}: <b>{}</b> ({:.1}%) {}</span> "###,
                            html_escape(&s.username),
                            format_number(s.score),
                            s.accuracy,
                            pass_str
                        ));
                    }

                    match_history_html.push_str(&format!(
                        r###"
                        <div style="background: rgba(15, 23, 42, 0.4); border: 1px solid rgba(255,255,255,0.05); border-radius: 6px; padding: 0.6rem 0.8rem;">
                            <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 0.4rem;">
                                <div style="font-size: 0.82rem; font-weight: 600; color: #e2e8f0;">Round #{} - {}</div>
                                {winner_badge}
                            </div>
                            <div style="display: flex; flex-wrap: wrap; gap: 0.4rem;">
                                {scores_summary}
                            </div>
                        </div>
                        "###,
                        item.games.len() - idx,
                        html_escape(&g.beatmap_name)
                    ));
                }
                match_history_html.push_str("</div></div>");
            }

            rooms_html.push_str(&format!(
                r###"
                <div class="glass-card" style="padding: 1.5rem; border-radius: 12px; margin-bottom: 1.5rem; background: rgba(30, 41, 59, 0.5); border: 1px solid rgba(255, 255, 255, 0.1); backdrop-filter: blur(12px);">
                    <div style="display: flex; justify-content: space-between; align-items: flex-start; flex-wrap: wrap; gap: 0.8rem; margin-bottom: 1rem;">
                        <div>
                            <div style="display: flex; align-items: center; gap: 0.6rem;">
                                <h3 style="font-size: 1.3rem; font-weight: 800; color: #fff; margin: 0;">{}</h3>
                                <span style="font-family: monospace; font-size: 0.8rem; background: rgba(255,255,255,0.08); padding: 0.2rem 0.5rem; border-radius: 4px; color: #94a3b8;">Room #{}</span>
                            </div>
                            <div style="color: #94a3b8; font-size: 0.88rem; margin-top: 0.3rem;">Host: <b style="color: #e2e8f0;">{}</b></div>
                        </div>
                        <div>{status_badge}</div>
                    </div>

                    <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 0.8rem; margin-bottom: 1.2rem; background: rgba(15, 23, 42, 0.4); padding: 0.8rem; border-radius: 8px; border: 1px solid rgba(255,255,255,0.05);">
                        <div><span style="color:#64748b; font-size:0.75rem; text-transform:uppercase;">Beatmap</span><div style="font-size:0.9rem; margin-top:0.2rem;">{map_link}</div></div>
                        <div><span style="color:#64748b; font-size:0.75rem; text-transform:uppercase;">Game Mode</span><div style="font-size:0.9rem; font-weight:600; color:#cbd5e1; margin-top:0.2rem;">{}</div></div>
                        <div><span style="color:#64748b; font-size:0.75rem; text-transform:uppercase;">Ruleset</span><div style="font-size:0.9rem; font-weight:600; color:#cbd5e1; margin-top:0.2rem;">{} &bull; {}</div></div>
                        <div><span style="color:#64748b; font-size:0.75rem; text-transform:uppercase;">Slots Filled</span><div style="font-size:0.9rem; font-weight:700; color:#f472b6; margin-top:0.2rem;">{}/16 Players</div></div>
                    </div>

                    <div style="font-size: 0.85rem; font-weight: 700; color: #94a3b8; text-transform: uppercase; letter-spacing: 0.5px; margin-bottom: 0.6rem;">Player Slots</div>
                    <div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(180px, 1fr)); gap: 0.6rem;">
                        {slots_html}
                    </div>

                    {match_history_html}
                </div>
                "###,
                html_escape(&r.name),
                r.match_id,
                html_escape(&r.host_name),
                get_multi_mode_name(r.mode),
                get_multi_scoring_name(r.scoring_type),
                get_multi_team_name(r.team_type),
                r.player_count,
            ));
        }
    }

    let full_html = format!(
        r###"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Multiplayer Tracker - Roseflower</title>
    <link rel="preconnect" href="https://fonts.googleapis.com">
    <link href="https://fonts.googleapis.com/css2?family=Plus+Jakarta+Sans:wght@400;500;600;700;800;900&family=JetBrains+Mono:wght@500;700&display=swap" rel="stylesheet">
    <style>
        :root {{
            --bg-base: #0b0f19;
            --primary: #f472b6;
            --primary-hover: #ec4899;
            --text-main: #f8fafc;
            --text-muted: #94a3b8;
        }}
        * {{ box-sizing: border-box; margin: 0; padding: 0; }}
        body {{
            background: radial-gradient(circle at 50% 0%, #1e1b4b 0%, #0b0f19 75%);
            color: var(--text-main);
            font-family: 'Plus Jakarta Sans', sans-serif;
            min-height: 100vh;
            padding: 2rem 1rem;
        }}
        .container {{
            max-width: 1050px;
            margin: 0 auto;
        }}
        .hero {{
            text-align: center;
            margin-bottom: 2.5rem;
        }}
        .hero-tag {{
            display: inline-block;
            padding: 0.35rem 0.8rem;
            background: rgba(244, 114, 182, 0.15);
            color: #f472b6;
            border: 1px solid rgba(244, 114, 182, 0.3);
            border-radius: 9999px;
            font-size: 0.75rem;
            font-weight: 700;
            letter-spacing: 1px;
            margin-bottom: 0.8rem;
        }}
        .hero h1 {{
            font-size: 2.5rem;
            font-weight: 900;
            letter-spacing: -0.5px;
            background: linear-gradient(135deg, #fff 30%, #f472b6 100%);
            -webkit-background-clip: text;
            -webkit-text-fill-color: transparent;
        }}
        .hero p {{
            color: var(--text-muted);
            font-size: 1.05rem;
            margin-top: 0.5rem;
        }}
        .chip-bar {{
            display: flex;
            justify-content: center;
            gap: 0.8rem;
            flex-wrap: wrap;
            margin-top: 1.2rem;
        }}
        .chip {{
            background: rgba(30, 41, 59, 0.6);
            border: 1px solid rgba(255, 255, 255, 0.1);
            padding: 0.45rem 0.9rem;
            border-radius: 8px;
            font-size: 0.85rem;
            color: #cbd5e1;
            display: inline-flex;
            align-items: center;
            gap: 0.4rem;
        }}
        .chip b {{ color: #fff; }}
        .pulse-dot {{
            width: 8px;
            height: 8px;
            border-radius: 50%;
            display: inline-block;
        }}
        .pulse-dot.green {{
            background: #4ade80;
            box-shadow: 0 0 8px #4ade80;
        }}
        .pulse-dot.red {{
            background: #ef4444;
            box-shadow: 0 0 8px #ef4444;
        }}
        .multi-badge {{
            display: inline-flex;
            align-items: center;
            gap: 0.4rem;
            padding: 0.3rem 0.65rem;
            border-radius: 6px;
            font-size: 0.75rem;
            font-weight: 700;
            letter-spacing: 0.5px;
        }}
        .multi-badge.waiting {{
            background: rgba(34, 197, 94, 0.15);
            color: #4ade80;
            border: 1px solid rgba(34, 197, 94, 0.3);
        }}
        .multi-badge.playing {{
            background: rgba(239, 68, 68, 0.15);
            color: #f87171;
            border: 1px solid rgba(239, 68, 68, 0.3);
        }}
        footer {{
            text-align: center;
            margin-top: 3rem;
            padding-top: 1.5rem;
            border-top: 1px solid rgba(255, 255, 255, 0.08);
            color: #64748b;
            font-size: 0.85rem;
        }}
        footer a {{
            color: #f472b6;
            text-decoration: none;
        }}
    </style>
</head>
<body>
    <div class="container">
        <div class="hero">
            <div class="hero-tag">ROSEFLOWER REAL-TIME MULTIPLAYER</div>
            <h1>Live Multiplayer Rooms</h1>
            <p>Live monitoring of osu! lobbies, player slots, beatmaps, and round match results on <b>{}</b>.</p>

            <div class="chip-bar">
                <div class="chip">Active Rooms: <b style="color: #f472b6;">{}</b></div>
                <div class="chip">Players in Multi: <b>{}</b></div>
                <div class="chip">Completed Rounds: <b>{}</b></div>
                <div class="chip"><span class="pulse-dot green"></span> Live Sync Active</div>
            </div>
        </div>

        <div id="roomsContainer">
            {}
        </div>

        <footer>
            Roseflower &bull; Dedicated Multiplayer Microservice for <a href="https://hatsuneakiko.io.vn">{}</a>
        </footer>
    </div>
</body>
</html>
"###,
        html_escape(&state.config.server.name),
        stats.active_rooms,
        stats.total_players,
        stats.total_completed_rounds,
        rooms_html,
        html_escape(&state.config.server.name)
    );

    Html(full_html)
}
