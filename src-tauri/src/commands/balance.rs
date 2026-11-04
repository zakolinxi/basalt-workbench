use reqwest::Method;
use serde::{Deserialize, Serialize};
use tauri::State;
use url::Url;

use crate::db::now;
use crate::error::{AppError, AppResult};
use crate::providers::{http, load_runtime, ProviderRuntime};
use crate::state::AppState;

/// Account credit returned by a provider's balance endpoint.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderBalance {
    pub provider_id: String,
    pub provider: String,
    pub source: String,
    pub available: bool,
    pub currency: String,
    pub total: f64,
    pub granted: f64,
    pub topped_up: f64,
    pub checked_at: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum BalanceSource {
    DeepSeek,
    Moonshot,
}

impl BalanceSource {
    fn key(self) -> &'static str {
        match self {
            Self::DeepSeek => "deepseek",
            Self::Moonshot => "moonshot",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
struct BalanceAmounts {
    available: bool,
    currency: String,
    total: f64,
    granted: f64,
    topped_up: f64,
}

/// Only providers with a documented, key-authenticated balance endpoint are
/// queried. Everything else returns `None` and the UI simply hides the chip.
fn balance_source(rt: &ProviderRuntime) -> Option<BalanceSource> {
    let url = Url::parse(&rt.provider.base_url).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    if host == "api.deepseek.com" || host.ends_with(".deepseek.com") {
        Some(BalanceSource::DeepSeek)
    } else if host == "api.moonshot.cn" || host.ends_with(".moonshot.cn") {
        Some(BalanceSource::Moonshot)
    } else {
        None
    }
}

fn endpoint(rt: &ProviderRuntime, source: BalanceSource) -> AppResult<String> {
    match source {
        BalanceSource::DeepSeek => {
            let url = Url::parse(&rt.provider.base_url)
                .map_err(|error| AppError::msg(format!("Base URL 无效：{error}")))?;
            // DeepSeek's balance endpoint lives at the origin, not under /v1.
            Ok(format!(
                "{}/user/balance",
                url.origin().ascii_serialization()
            ))
        }
        BalanceSource::Moonshot => http::join_url(&rt.provider.base_url, "users/me/balance"),
    }
}

#[derive(Debug, Deserialize)]
struct DeepSeekResponse {
    #[serde(default)]
    is_available: bool,
    #[serde(default)]
    balance_infos: Vec<DeepSeekInfo>,
}

#[derive(Debug, Deserialize)]
struct DeepSeekInfo {
    #[serde(default)]
    currency: String,
    #[serde(default)]
    total_balance: String,
    #[serde(default)]
    granted_balance: String,
    #[serde(default)]
    topped_up_balance: String,
}

fn amount(raw: &str) -> f64 {
    raw.trim().parse::<f64>().unwrap_or(0.0)
}

fn parse_deepseek(body: &str) -> AppResult<BalanceAmounts> {
    let parsed: DeepSeekResponse = serde_json::from_str(body)?;
    let info = parsed
        .balance_infos
        .iter()
        .find(|info| info.currency.eq_ignore_ascii_case("CNY"))
        .or_else(|| parsed.balance_infos.first())
        .ok_or_else(|| AppError::msg("余额接口没有返回余额信息"))?;
    Ok(BalanceAmounts {
        available: parsed.is_available,
        currency: if info.currency.is_empty() {
            "CNY".to_string()
        } else {
            info.currency.clone()
        },
        total: amount(&info.total_balance),
        granted: amount(&info.granted_balance),
        topped_up: amount(&info.topped_up_balance),
    })
}

#[derive(Debug, Deserialize)]
struct MoonshotResponse {
    #[serde(default)]
    code: i64,
    #[serde(default)]
    status: Option<bool>,
    data: Option<MoonshotInfo>,
}

#[derive(Debug, Deserialize)]
struct MoonshotInfo {
    #[serde(default)]
    available_balance: f64,
    #[serde(default)]
    voucher_balance: f64,
    #[serde(default)]
    cash_balance: f64,
}

fn parse_moonshot(body: &str) -> AppResult<BalanceAmounts> {
    let parsed: MoonshotResponse = serde_json::from_str(body)?;
    if parsed.code != 0 || parsed.status == Some(false) {
        return Err(AppError::msg("余额接口返回了错误状态"));
    }
    let info = parsed
        .data
        .ok_or_else(|| AppError::msg("余额接口没有返回余额信息"))?;
    Ok(BalanceAmounts {
        available: true,
        // Moonshot's balance endpoint does not include a currency; the
        // platform bills in CNY.
        currency: "CNY".to_string(),
        total: info.available_balance,
        granted: info.voucher_balance,
        topped_up: info.cash_balance,
    })
}

async fn fetch_balance(
    state: &AppState,
    rt: &ProviderRuntime,
    source: BalanceSource,
) -> AppResult<BalanceAmounts> {
    if !rt.has_key() {
        return Err(AppError::msg("未设置 API Key"));
    }
    let url = endpoint(rt, source)?;
    let response = http::with_bearer(
        http::request(&state.http, Method::GET, &url, rt)
            .header(reqwest::header::ACCEPT, "application/json"),
        rt,
    )
    .send()
    .await?;
    let response = http::ensure_ok(response).await?;
    let body = response.text().await?;
    match source {
        BalanceSource::DeepSeek => parse_deepseek(&body),
        BalanceSource::Moonshot => parse_moonshot(&body),
    }
}

/// Looks up the current credit for the provider that owns `provider_id`.
/// The API key never leaves the backend: only the parsed balance is returned.
#[tauri::command]
pub async fn provider_balance(
    state: State<'_, AppState>,
    provider_id: String,
) -> AppResult<Option<ProviderBalance>> {
    let rt = load_runtime(&state, &provider_id)?;
    if rt.provider.is_template {
        return Ok(None);
    }
    let Some(source) = balance_source(&rt) else {
        return Ok(None);
    };
    let parsed = fetch_balance(&state, &rt, source).await?;
    Ok(Some(ProviderBalance {
        provider_id: rt.provider.id.clone(),
        provider: rt.provider.name.clone(),
        source: source.key().to_string(),
        available: parsed.available,
        currency: parsed.currency,
        total: parsed.total,
        granted: parsed.granted,
        topped_up: parsed.topped_up,
        checked_at: now(),
    }))
}

#[cfg(test)]
mod tests {
    use super::{parse_deepseek, parse_moonshot, BalanceAmounts};

    #[test]
    fn reads_deepseek_balance() {
        let body = r#"{"is_available":true,"balance_infos":[{"currency":"CNY","total_balance":"2.01","granted_balance":"0.00","topped_up_balance":"2.01"}]}"#;
        let parsed = parse_deepseek(body).unwrap();
        assert_eq!(
            parsed,
            BalanceAmounts {
                available: true,
                currency: "CNY".into(),
                total: 2.01,
                granted: 0.0,
                topped_up: 2.01,
            }
        );
    }

    #[test]
    fn prefers_cny_when_multiple_currencies_are_returned() {
        let body = r#"{"is_available":true,"balance_infos":[{"currency":"USD","total_balance":"1.00","granted_balance":"0","topped_up_balance":"1.00"},{"currency":"CNY","total_balance":"7.20","granted_balance":"1.20","topped_up_balance":"6.00"}]}"#;
        let parsed = parse_deepseek(body).unwrap();
        assert_eq!(parsed.currency, "CNY");
        assert_eq!(parsed.total, 7.2);
        assert_eq!(parsed.granted, 1.2);
    }

    #[test]
    fn rejects_deepseek_response_without_balances() {
        assert!(parse_deepseek(r#"{"is_available":true,"balance_infos":[]}"#).is_err());
        assert!(parse_deepseek("not json").is_err());
    }

    #[test]
    fn reads_moonshot_balance() {
        let body = r#"{"code":0,"data":{"available_balance":24.996382,"voucher_balance":14.998191,"cash_balance":9.998191},"scode":"0x0","status":true}"#;
        let parsed = parse_moonshot(body).unwrap();
        assert_eq!(
            parsed,
            BalanceAmounts {
                available: true,
                currency: "CNY".into(),
                total: 24.996382,
                granted: 14.998191,
                topped_up: 9.998191,
            }
        );
    }

    #[test]
    fn rejects_moonshot_error_status() {
        assert!(parse_moonshot(r#"{"code":401,"status":false,"data":null}"#).is_err());
        assert!(parse_moonshot(r#"{"code":0,"status":true}"#).is_err());
    }
}
