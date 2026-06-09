#![cfg(feature = "web-server")]

use std::{
    env, io,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    sync::Arc,
};

use axum::serve;
use log::{error, info};
use tokio::{net::TcpListener, signal};

use cc_switch_lib::{
    store::AppState,
    web_api::{create_router_with_auth_state, load_or_generate_web_credentials, SharedState},
};

fn init_logger() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
}

fn env_truthy(name: &str) -> bool {
    env::var(name).is_ok_and(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "on"))
}

fn ipv4_is_private(ip: Ipv4Addr) -> bool {
    let [a, b, _, _] = ip.octets();
    a == 10 || (a == 172 && (b & 0xf0) == 16) || (a == 192 && b == 168)
}

fn ipv4_is_loopback(ip: Ipv4Addr) -> bool {
    ip.octets()[0] == 127
}

fn ipv4_is_link_local(ip: Ipv4Addr) -> bool {
    let [a, b, _, _] = ip.octets();
    a == 169 && b == 254
}

fn ipv4_is_unspecified(ip: Ipv4Addr) -> bool {
    ip.octets() == [0, 0, 0, 0]
}

fn ipv6_is_loopback(ip: Ipv6Addr) -> bool {
    ip.segments() == [0, 0, 0, 0, 0, 0, 0, 1]
}

fn ipv6_is_unique_local(ip: Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xfe00) == 0xfc00
}

fn ipv6_is_link_local(ip: Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xffc0) == 0xfe80
}

fn ipv6_is_unspecified(ip: Ipv6Addr) -> bool {
    ip.segments() == [0, 0, 0, 0, 0, 0, 0, 0]
}

fn ip_is_loopback(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => ipv4_is_loopback(v4),
        IpAddr::V6(v6) => ipv6_is_loopback(v6),
    }
}

fn ip_is_unspecified(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => ipv4_is_unspecified(v4),
        IpAddr::V6(v6) => ipv6_is_unspecified(v6),
    }
}

fn is_lan_bind_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => ipv4_is_private(v4) || ipv4_is_loopback(v4) || ipv4_is_link_local(v4),
        IpAddr::V6(v6) => {
            ipv6_is_unique_local(v6) || ipv6_is_loopback(v6) || ipv6_is_link_local(v6)
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_logger();

    let (auth_state, password_path) = load_or_generate_web_credentials()?;
    let username = auth_state
        .read()
        .map(|guard| guard.username.clone())
        .map_err(|err| io::Error::other(err.to_string()))?;

    let state: SharedState = Arc::new(AppState::try_new()?);

    let port = env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(3000);

    let host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let addr: SocketAddr = format!("{host}:{port}").parse().unwrap_or_else(|_| {
        log::warn!("Invalid HOST `{host}`, falling back to 127.0.0.1");
        SocketAddr::from(([127, 0, 0, 1], port))
    });

    let bind_ip = addr.ip();
    let allow_insecure = env_truthy("ALLOW_HTTP_BASIC_OVER_HTTP");
    let is_public_bind = ip_is_unspecified(bind_ip) || !ip_is_loopback(bind_ip);
    if is_public_bind {
        let egress_policy = env::var("USAGE_SCRIPT_EGRESS_POLICY").ok();
        let should_set_egress = egress_policy
            .as_deref()
            .map(|value| value.trim().is_empty())
            .unwrap_or(true);
        if should_set_egress {
            env::set_var("USAGE_SCRIPT_EGRESS_POLICY", "strict");
            info!(
                "USAGE_SCRIPT_EGRESS_POLICY 未设置，已自动调整为 strict 以限制外网监听时的脚本出口"
            );
        }
    }

    let cors_origins_set = env::var("CORS_ALLOW_ORIGINS")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
    let allow_lan_cors_set = env::var("ALLOW_LAN_CORS").is_ok();
    let allow_lan_cors = env_truthy("ALLOW_LAN_CORS");
    let mut lan_cors_enabled = allow_lan_cors;

    if !cors_origins_set && !allow_lan_cors_set && is_lan_bind_ip(bind_ip) {
        lan_cors_enabled = true;
        info!("CORS_ALLOW_ORIGINS 未设置，已自动启用局域网跨域白名单");
    }

    let cors_credentials_set = env::var("CORS_ALLOW_CREDENTIALS")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
    if lan_cors_enabled {
        env::set_var("CC_SWITCH_LAN_CORS", "1");
        if !cors_credentials_set {
            env::set_var("CORS_ALLOW_CREDENTIALS", "1");
        }
    }

    cc_switch_lib::local_proxy::start_from_saved_settings(state.clone()).await;

    let app = create_router_with_auth_state(state, auth_state);

    if is_public_bind && !allow_insecure {
        if ip_is_unspecified(bind_ip) {
            error!(
                "当前以 HTTP 监听 0.0.0.0，Basic/Bearer 凭证可能被截获。如需公开，请在反向代理中终止 TLS 并设置 ALLOW_HTTP_BASIC_OVER_HTTP=1"
            );
        } else {
            error!(
                "监听非本地地址 {}，建议启用 TLS 反代或设置 ALLOW_HTTP_BASIC_OVER_HTTP=1 明确接受风险",
                addr
            );
        }
        return Err(
            io::Error::other("Refusing to start without ALLOW_HTTP_BASIC_OVER_HTTP").into(),
        );
    }

    info!(
        "Starting web server on http://{} with file-based credentials at {} (username: {}, token stored only on disk)",
        addr,
        password_path.display(),
        username
    );
    println!(
        "Web console login -> user: {}, token path: {} (content not printed for safety)",
        username,
        password_path.display()
    );

    let listener = TcpListener::bind(addr).await?;
    serve(listener, app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("Server shut down cleanly");
    Ok(())
}

async fn shutdown_signal() {
    match signal::ctrl_c().await {
        Ok(()) => info!("Shutdown signal received, terminating server ..."),
        Err(err) => error!("Failed to listen for shutdown signal: {}", err),
    }
}
