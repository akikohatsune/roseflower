use crate::config::Config;
use crate::db::{self, DbLiveRoom, DbMultiGame, DbMultiScore, DbPool};
use axum::extract::{Path as AxPath, State};
use axum::http::{HeaderMap, StatusCode};
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
        .route("/favicon.ico", get(favicon_handler))
        .route("/health", get(health_check))
        .route("/api/multi/rooms", get(list_rooms_api))
        .route("/api/multi/rooms/{id}", get(get_room_api))
        .route("/api/multi/stats", get(get_stats_api))
        .route("/api/multi/sync", post(sync_room_api))
        .route("/api/multi/disband", post(disband_room_api))
        .route("/api/multi/finish", post(finish_match_api))
        .with_state(state)
}

async fn favicon_handler(State(state): State<AppState>) -> impl IntoResponse {
    let target = if !state.config.server.base_url.is_empty() {
        format!("{}/static/favicon.png", state.config.server.base_url.trim_end_matches('/'))
    } else {
        "https://hatsuneakiko.io.vn/static/favicon.png".to_string()
    };
    axum::response::Redirect::temporary(&target)
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

async fn multi_page_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Html<String> {
    let host = headers
        .get("x-forwarded-host")
        .or_else(|| headers.get("host"))
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .to_lowercase();
    let base_url = if host.starts_with("roseflower.") {
        let domain = host.trim_start_matches("roseflower.");
        format!("https://{}", domain)
    } else if !state.config.server.base_url.is_empty() {
        state.config.server.base_url.trim_end_matches('/').to_string()
    } else {
        "https://hatsuneakiko.io.vn".to_string()
    };

    let rooms = db::get_all_live_rooms(&state.db).await.unwrap_or_default();
    let stats = db::get_stats(&state.db).await.unwrap_or(db::MultiStatsSummary {
        active_rooms: rooms.len(),
        total_players: rooms.iter().map(|r| r.room.player_count).sum(),
        total_completed_rounds: 0,
    });

    let mut rooms_html = String::new();
    if rooms.is_empty() {
        rooms_html.push_str(&format!(
            r###"
            <div class="empty-state">
                <div class="empty-icon">
                    <svg width="30" height="30" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <rect x="2" y="6" width="20" height="12" rx="2"></rect>
                        <path d="M6 12h4m-2-2v4m7-2h.01m3 0h.01"></path>
                    </svg>
                </div>
                <h3>No Active Multiplayer Rooms</h3>
                <p>
                    There are currently no active lobbies on the server. Connect via your osu! client, create or join a room in the multiplayer lobby, and watch live tracking here!
                </p>
                <div style="margin-top: 1.5rem;">
                    <a href="{base_url}/connect" class="btn btn-primary">How to Connect & Play</a>
                </div>
            </div>
            "###,
            base_url = base_url
        ));
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
                    r###"<a href="https://osu.ppy.sh/b/{}" target="_blank" rel="noopener noreferrer" class="map-link">{}</a>"###,
                    r.beatmap_id, html_escape(&r.beatmap_name)
                )
            } else {
                format!(r###"<span class="map-muted">{}</span>"###, html_escape(&r.beatmap_name))
            };

            let mut slots_html = String::new();
            for slot in &item.slots {
                let status_pill = match slot.status {
                    1 => r#"<span class="slot-status not-ready">Not Ready</span>"#,
                    2 => r#"<span class="slot-status ready">✓ Ready</span>"#,
                    4 => r#"<span class="slot-status playing">Playing</span>"#,
                    8 => r#"<span class="slot-status completed">Completed</span>"#,
                    _ => r#"<span class="slot-status open">Open</span>"#,
                };

                let team_dot = match slot.team {
                    1 => r#"<span class="team-dot blue" title="Blue Team"></span>"#,
                    2 => r#"<span class="team-dot red" title="Red Team"></span>"#,
                    _ => "",
                };

                slots_html.push_str(&format!(
                    r###"
                    <div class="slot-card">
                        <div class="slot-user">
                            <span class="slot-num">#{:02}</span>
                            {team_dot}
                            <span class="slot-name">{}</span>
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
                <div style="margin-top: 1.2rem; border-top: 1px solid var(--card-border-subtle); padding-top: 1rem;">
                    <div class="section-lbl">Recent Rounds Played</div>
                    <div style="display: flex; flex-direction: column; gap: 0.6rem;">
                "###);

                for (idx, game_with_scores) in item.games.iter().enumerate() {
                    let g = &game_with_scores.game;
                    let winner_badge = if !g.winner_name.is_empty() {
                        format!(r###"<span style="color: var(--amber); font-weight: 700; font-size: 0.8rem;">👑 Winner: {}</span>"###, html_escape(&g.winner_name))
                    } else {
                        String::new()
                    };

                    let mut scores_summary = String::new();
                    for s in &game_with_scores.scores {
                        let pass_str = if s.passed == 1 { "✓" } else { "✗" };
                        scores_summary.push_str(&format!(
                            r###"<span style="font-size: 0.78rem; color: var(--text-main); background: var(--bg-surface-hover); border: 1px solid var(--card-border-subtle); padding: 0.2rem 0.5rem; border-radius: 4px;">{}: <b>{}</b> ({:.1}%) {}</span> "###,
                            html_escape(&s.username),
                            format_number(s.score),
                            s.accuracy,
                            pass_str
                        ));
                    }

                    match_history_html.push_str(&format!(
                        r###"
                        <div style="background: var(--bg-surface-hover); border: 1px solid var(--card-border-subtle); border-radius: 6px; padding: 0.6rem 0.8rem;">
                            <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 0.4rem;">
                                <div style="font-size: 0.82rem; font-weight: 600; color: var(--text-main);">Round #{} - {}</div>
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
                <div class="room-card">
                    <div class="room-header">
                        <div>
                            <div class="room-title-group">
                                <h3 class="room-title">{}</h3>
                                <span class="room-id-tag">Room #{}</span>
                            </div>
                            <div class="room-host">Host: <b>{}</b></div>
                        </div>
                        <div>{status_badge}</div>
                    </div>

                    <div class="meta-grid">
                        <div class="meta-item"><span class="meta-lbl">Beatmap</span><div class="meta-val">{map_link}</div></div>
                        <div class="meta-item"><span class="meta-lbl">Game Mode</span><div class="meta-val">{}</div></div>
                        <div class="meta-item"><span class="meta-lbl">Ruleset</span><div class="meta-val">{} &bull; {}</div></div>
                        <div class="meta-item"><span class="meta-lbl">Slots Filled</span><div class="meta-val" style="color: var(--primary); font-weight: 700;">{}/16 Players</div></div>
                    </div>

                    <div class="section-lbl">Player Slots</div>
                    <div class="slots-grid">
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
    <link rel="icon" type="image/png" href="{base_url}/static/favicon.png">
    <link rel="shortcut icon" href="{base_url}/favicon.ico">
    <title>Live Multiplayer Rooms - {server_name}</title>

    <!-- Discord & Open Graph / SEO Meta Tags -->
    <meta name="theme-color" content="#f472b6">
    <meta name="description" content="Live monitoring of osu! multiplayer lobbies, player slots, beatmaps, and round match results on {server_name}.">
    <meta property="og:site_name" content="{server_name}">
    <meta property="og:title" content="Live Multiplayer Rooms - {server_name}">
    <meta property="og:description" content="Live monitoring of osu! multiplayer lobbies, player slots, beatmaps, and round match results on {server_name}.">
    <meta property="og:type" content="website">
    <meta property="og:url" content="https://roseflower.hatsuneakiko.io.vn/multi">
    <meta property="og:image" content="{base_url}/static/logo.png">

    <!-- Twitter Card -->
    <meta name="twitter:card" content="summary_large_image">
    <meta name="twitter:title" content="Live Multiplayer Rooms - {server_name}">
    <meta name="twitter:description" content="Live monitoring of osu! multiplayer lobbies, player slots, beatmaps, and round match results on {server_name}.">
    <meta name="twitter:image" content="{base_url}/static/logo.png">

    <link rel="preconnect" href="https://fonts.googleapis.com">
    <link href="https://fonts.googleapis.com/css2?family=Plus+Jakarta+Sans:wght@400;500;600;700;800;900&family=JetBrains+Mono:wght@500;700&display=swap" rel="stylesheet">
    <style>
        :root {{
            --bg-base: #11141a;
            --bg-surface: #181b22;
            --bg-surface-hover: #1f232c;
            --card-border: #282d38;
            --card-border-subtle: #1e222b;
            --primary: #e0558e;
            --primary-hover: #c9447a;
            --accent: #705df2;
            --text-main: #f1f5f9;
            --text-muted: #94a3b8;
            --text-sub: #64748b;
            --emerald: #10b981;
            --rose: #ef4444;
            --cyan: #0ea5e9;
            --amber: #f59e0b;
        }}
        * {{
            box-sizing: border-box;
            margin: 0;
            padding: 0;
            font-family: 'Plus Jakarta Sans', system-ui, -apple-system, sans-serif;
        }}
        body {{
            background-color: var(--bg-base);
            color: var(--text-main);
            min-height: 100vh;
            display: flex;
            flex-direction: column;
            overflow-x: hidden;
            -webkit-font-smoothing: antialiased;
        }}
        a {{ color: inherit; text-decoration: none; }}

        /* Flat Navbar */
        nav {{
            position: sticky;
            top: 0;
            z-index: 1000;
            background: var(--bg-surface);
            border-bottom: 1px solid var(--card-border);
        }}
        .nav-container {{
            max-width: 1440px;
            margin: 0 auto;
            display: flex;
            align-items: center;
            justify-content: space-between;
            gap: 1.25rem;
            padding: 0.85rem 1.5rem;
        }}
        .nav-brand {{
            display: flex;
            align-items: center;
            gap: 0.6rem;
            font-size: 1.35rem;
            font-weight: 800;
            letter-spacing: -0.5px;
            color: var(--primary);
        }}
        .nav-brand-badge {{
            font-size: 0.72rem;
            font-weight: 700;
            letter-spacing: 0.5px;
            text-transform: uppercase;
            background: var(--bg-surface-hover);
            border: 1px solid var(--card-border);
            color: var(--text-muted);
            padding: 0.2rem 0.5rem;
            border-radius: 4px;
        }}
        .nav-links {{
            display: flex;
            align-items: center;
            justify-content: center;
            gap: clamp(0.75rem, 1.35vw, 1.5rem);
            list-style: none;
            font-size: 0.92rem;
            font-weight: 600;
        }}
        .nav-links a {{
            color: var(--text-muted);
            padding: 0.4rem 0;
            white-space: nowrap;
            transition: color 0.15s ease;
        }}
        .nav-links a:hover, .nav-links a.active {{
            color: var(--text-main);
        }}
        .nav-links a.active {{
            border-bottom: 2px solid var(--primary);
        }}
        .nav-actions {{
            display: flex;
            align-items: center;
            gap: 0.75rem;
        }}
        .btn {{
            display: inline-flex;
            align-items: center;
            justify-content: center;
            padding: 0.5rem 1.1rem;
            font-size: 0.88rem;
            font-weight: 700;
            border-radius: 6px;
            cursor: pointer;
            transition: all 0.15s ease;
            text-decoration: none;
            border: 1px solid transparent;
        }}
        .btn-primary {{
            background: var(--primary);
            color: #fff;
        }}
        .btn-primary:hover {{
            background: var(--primary-hover);
        }}
        .btn-outline {{
            background: var(--bg-surface-hover);
            color: var(--text-main);
            border-color: var(--card-border);
        }}
        .btn-outline:hover {{
            background: var(--card-border);
        }}

        /* Container & Intro */
        .main-container {{
            max-width: 1200px;
            margin: 0 auto;
            padding: 2.5rem 1.5rem;
            flex: 1;
            width: 100%;
        }}
        .page-intro {{
            text-align: center;
            max-width: 820px;
            margin: 0 auto 2.5rem auto;
        }}
        .hero-tag {{
            display: inline-flex;
            align-items: center;
            background: var(--bg-surface-hover);
            border: 1px solid var(--card-border);
            color: var(--primary);
            padding: 4px 14px;
            border-radius: 4px;
            font-size: 0.82rem;
            font-weight: 700;
            letter-spacing: 0.5px;
            margin-bottom: 0.8rem;
        }}
        .page-title {{
            font-size: clamp(2rem, 3.5vw, 2.6rem);
            font-weight: 900;
            letter-spacing: -0.8px;
            line-height: 1.2;
            color: var(--text-main);
        }}
        .page-subtitle {{
            margin-top: 0.6rem;
            color: var(--text-muted);
            font-size: clamp(0.95rem, 1.8vw, 1.05rem);
            line-height: 1.6;
        }}

        /* Flat Stat Chips */
        .stat-chips {{
            display: flex;
            justify-content: center;
            gap: 0.75rem;
            flex-wrap: wrap;
            margin-top: 1.4rem;
        }}
        .stat-chip {{
            background: var(--bg-surface);
            border: 1px solid var(--card-border);
            padding: 0.45rem 1rem;
            border-radius: 6px;
            font-size: 0.88rem;
            color: var(--text-muted);
            display: inline-flex;
            align-items: center;
            gap: 0.4rem;
        }}
        .stat-chip b {{
            color: var(--text-main);
        }}
        .pulse-dot {{
            width: 8px;
            height: 8px;
            border-radius: 50%;
            display: inline-block;
        }}
        .pulse-dot.green {{
            background: var(--emerald);
        }}
        .pulse-dot.red {{
            background: var(--rose);
        }}

        /* Flat Empty State */
        .empty-state {{
            background: var(--bg-surface);
            border: 1px solid var(--card-border);
            border-radius: 8px;
            padding: 4rem 2rem;
            text-align: center;
            max-width: 820px;
            margin: 0 auto;
        }}
        .empty-icon {{
            width: 60px;
            height: 60px;
            margin: 0 auto 1.2rem auto;
            border-radius: 8px;
            background: var(--bg-surface-hover);
            border: 1px solid var(--card-border);
            display: flex;
            align-items: center;
            justify-content: center;
            color: var(--primary);
        }}
        .empty-state h3 {{
            font-size: 1.35rem;
            font-weight: 800;
            color: var(--text-main);
            margin-bottom: 0.5rem;
        }}
        .empty-state p {{
            color: var(--text-muted);
            font-size: 0.95rem;
            max-width: 540px;
            margin: 0 auto;
            line-height: 1.6;
        }}

        /* Flat Room Card */
        .room-card {{
            background: var(--bg-surface);
            border: 1px solid var(--card-border);
            border-radius: 8px;
            padding: 1.5rem;
            margin-bottom: 1.5rem;
        }}
        .room-header {{
            display: flex;
            justify-content: space-between;
            align-items: flex-start;
            flex-wrap: wrap;
            gap: 0.8rem;
            margin-bottom: 1.2rem;
        }}
        .room-title-group {{
            display: flex;
            align-items: center;
            gap: 0.6rem;
            flex-wrap: wrap;
        }}
        .room-title {{
            font-size: 1.3rem;
            font-weight: 800;
            color: var(--text-main);
        }}
        .room-id-tag {{
            font-family: 'JetBrains Mono', monospace;
            font-size: 0.8rem;
            background: var(--bg-surface-hover);
            border: 1px solid var(--card-border);
            padding: 0.2rem 0.5rem;
            border-radius: 4px;
            color: var(--text-muted);
        }}
        .room-host {{
            color: var(--text-muted);
            font-size: 0.88rem;
            margin-top: 0.3rem;
        }}
        .room-host b {{ color: var(--text-main); }}

        .multi-badge {{
            display: inline-flex;
            align-items: center;
            gap: 0.4rem;
            padding: 0.3rem 0.65rem;
            border-radius: 4px;
            font-size: 0.75rem;
            font-weight: 700;
            letter-spacing: 0.5px;
        }}
        .multi-badge.waiting {{
            background: rgba(16, 185, 129, 0.12);
            color: var(--emerald);
            border: 1px solid rgba(16, 185, 129, 0.25);
        }}
        .multi-badge.playing {{
            background: rgba(239, 68, 68, 0.12);
            color: var(--rose);
            border: 1px solid rgba(239, 68, 68, 0.25);
        }}

        .meta-grid {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
            gap: 0.8rem;
            margin-bottom: 1.2rem;
            background: var(--bg-surface-hover);
            padding: 0.8rem 1rem;
            border-radius: 6px;
            border: 1px solid var(--card-border-subtle);
        }}
        .meta-item .meta-lbl {{
            color: var(--text-sub);
            font-size: 0.75rem;
            text-transform: uppercase;
            font-weight: 700;
            letter-spacing: 0.5px;
        }}
        .meta-item .meta-val {{
            font-size: 0.9rem;
            font-weight: 600;
            color: var(--text-main);
            margin-top: 0.2rem;
            white-space: nowrap;
            overflow: hidden;
            text-overflow: ellipsis;
        }}
        .map-link {{
            color: var(--cyan);
            font-weight: 600;
        }}
        .map-link:hover {{ text-decoration: underline; }}
        .map-muted {{ color: var(--text-muted); }}

        .section-lbl {{
            font-size: 0.82rem;
            font-weight: 700;
            color: var(--text-muted);
            text-transform: uppercase;
            letter-spacing: 0.5px;
            margin-bottom: 0.6rem;
        }}
        .slots-grid {{
            display: grid;
            grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
            gap: 0.6rem;
        }}
        .slot-card {{
            background: var(--bg-surface-hover);
            padding: 0.6rem 0.8rem;
            border-radius: 6px;
            border: 1px solid var(--card-border-subtle);
            display: flex;
            align-items: center;
            justify-content: space-between;
            gap: 0.5rem;
        }}
        .slot-user {{
            display: flex;
            align-items: center;
            gap: 0.5rem;
            overflow: hidden;
        }}
        .slot-num {{
            font-family: 'JetBrains Mono', monospace;
            font-size: 0.78rem;
            color: var(--text-sub);
        }}
        .slot-name {{
            font-weight: 600;
            font-size: 0.85rem;
            color: var(--text-main);
            white-space: nowrap;
            overflow: hidden;
            text-overflow: ellipsis;
            max-width: 110px;
        }}
        .slot-status {{
            font-size: 0.75rem;
            font-weight: 700;
        }}
        .slot-status.ready {{ color: var(--emerald); }}
        .slot-status.playing {{ color: var(--primary); }}
        .slot-status.completed {{ color: var(--cyan); }}
        .slot-status.not-ready {{ color: var(--text-muted); }}
        .slot-status.open {{ color: var(--text-sub); }}
        .team-dot {{
            width: 8px;
            height: 8px;
            border-radius: 50%;
            display: inline-block;
        }}
        .team-dot.blue {{ background: #3b82f6; }}
        .team-dot.red {{ background: #ef4444; }}

        /* Flat Footer */
        footer {{
            background: var(--bg-surface);
            border-top: 1px solid var(--card-border);
            padding: 2.5rem 1.5rem;
            margin-top: auto;
            text-align: center;
            color: var(--text-muted);
            font-size: 0.88rem;
        }}
        .footer-links {{
            display: flex;
            justify-content: center;
            gap: 1.5rem;
            flex-wrap: wrap;
            margin-top: 0.8rem;
            font-weight: 600;
        }}
        .footer-links a:hover {{ color: var(--text-main); }}
    </style>
</head>
<body>
    <nav>
        <div class="nav-container">
            <a href="{base_url}/" class="nav-brand" style="display: flex; align-items: center; gap: 0.75rem;">
                <img src="{base_url}/static/logo.png" alt="{server_name}" style="height: 38px; width: auto; max-width: 220px; object-fit: contain; filter: drop-shadow(0 2px 6px rgba(0,0,0,0.4));">
                <span class="nav-brand-badge">Multiplayer</span>
            </a>
            <ul class="nav-links">
                <li><a href="{base_url}/">Home</a></li>
                <li><a href="{base_url}/leaderboard">Leaderboard</a></li>
                <li><a href="/multi" class="active">Multiplayer</a></li>
                <li><a href="{base_url}/rule">Rules</a></li>
                <li><a href="{base_url}/staff">Staff & Credits</a></li>
                <li><a href="{base_url}/connect">Connect</a></li>
            </ul>
            <div class="nav-actions">
                <a href="{base_url}/" class="btn btn-outline">Back to Website</a>
            </div>
        </div>
    </nav>

    <main class="main-container">
        <div class="page-intro">
            <div class="hero-tag">ROSEFLOWER REAL-TIME MULTIPLAYER</div>
            <h1 class="page-title">Live Multiplayer Rooms</h1>
            <p class="page-subtitle">
                Live monitoring of osu! lobbies, player slots, beatmaps, and round match results on <b>{server_name}</b>.
            </p>

            <div class="stat-chips">
                <div class="stat-chip">Active Rooms: <b style="color: var(--primary);">{active_rooms}</b></div>
                <div class="stat-chip">Players in Multi: <b>{total_players}</b></div>
                <div class="stat-chip">Completed Rounds: <b>{total_rounds}</b></div>
                <div class="stat-chip"><span class="pulse-dot green"></span> Live Sync Active</div>
            </div>
        </div>

        <div id="roomsContainer">
            {rooms_html}
        </div>
    </main>

    <footer>
        <div style="display: flex; align-items: center; justify-content: center; margin-bottom: 0.8rem;">
            <img src="{base_url}/static/logo.png" alt="{server_name}" style="height: 32px; width: auto; object-fit: contain; opacity: 0.85;">
        </div>
        <div style="font-weight: 500; color: var(--text-muted);">
            © 2026 <b style="color: var(--text-main);">{server_name}</b> • Roseflower Dedicated Multiplayer Microservice
        </div>
        <div class="footer-links">
            <a href="{base_url}/">Home</a>
            <a href="{base_url}/leaderboard">Leaderboard</a>
            <a href="/multi">Multiplayer</a>
            <a href="{base_url}/rule">Rules</a>
            <a href="{base_url}/changelog">Changelog</a>
            <a href="{base_url}/staff">Staff & Credits</a>
            <a href="{base_url}/connect">Connect</a>
            <a href="https://status.hatsuneakiko.io.vn/" target="_blank" rel="noopener noreferrer">Status</a>
        </div>
    </footer>
</body>
</html>
"###,
        base_url = base_url,
        server_name = html_escape(&state.config.server.name),
        active_rooms = stats.active_rooms,
        total_players = stats.total_players,
        total_rounds = stats.total_completed_rounds,
        rooms_html = rooms_html,
    );

    Html(full_html)
}
