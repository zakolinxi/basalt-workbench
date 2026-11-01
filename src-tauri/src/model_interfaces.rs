use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInterfaceProfile {
    pub family: String,
    pub recommended_protocol: String,
    pub fallback_protocols: Vec<String>,
    pub cost_tier: String,
    pub speed: String,
    pub input_usd_per_mtok: Option<f64>,
    pub output_usd_per_mtok: Option<f64>,
    pub note: String,
    pub source: String,
    pub updated: String,
}

fn openai_profile(
    family: &str,
    protocol: &str,
    cost_tier: &str,
    speed: &str,
    input: Option<f64>,
    output: Option<f64>,
    note: &str,
) -> ModelInterfaceProfile {
    ModelInterfaceProfile {
        family: family.into(),
        recommended_protocol: protocol.into(),
        fallback_protocols: if protocol == "openai_responses" {
            vec!["openai_chat".into()]
        } else {
            vec!["openai_responses".into()]
        },
        cost_tier: cost_tier.into(),
        speed: speed.into(),
        input_usd_per_mtok: input,
        output_usd_per_mtok: output,
        note: note.into(),
        source: "https://platform.openai.com/docs/pricing · https://www.aiexpress.news/wiki/llm-api-pricing-2026".into(),
        updated: "2026-10-31".into(),
    }
}

pub fn profile(model: &str) -> Option<ModelInterfaceProfile> {
    let id = model.trim().to_ascii_lowercase();

    // OpenAI: Responses carries native reasoning state and better cache reuse;
    // keep Chat Completions for legacy/compatible models and gateways.
    if id.contains("gpt-6") {
        return Some(openai_profile(
            "OpenAI GPT-6",
            "openai_responses",
            "high",
            "balanced",
            Some(10.0),
            Some(50.0),
            "旗舰推理模型优先 Responses；缓存与推理状态可降低长任务重试成本。",
        ));
    }
    if id.contains("gpt-5.6") {
        let (tier, input, output) = if id.contains("luna") {
            ("low", Some(1.0), Some(6.0))
        } else if id.contains("terra") {
            ("medium", Some(2.5), Some(15.0))
        } else {
            ("high", Some(5.0), Some(30.0))
        };
        return Some(openai_profile(
            "OpenAI GPT-5.6",
            "openai_responses",
            tier,
            "balanced",
            input,
            output,
            "Responses 原生工具与缓存友好；Sol/Terra/Luna 成本档不同。",
        ));
    }
    if id.contains("gpt-5") {
        return Some(openai_profile(
            "OpenAI GPT-5",
            "openai_responses",
            "high",
            "balanced",
            Some(5.0),
            Some(30.0),
            "推理模型优先 Responses；Chat Completions 作为兼容回退。",
        ));
    }
    if id.contains("gpt-4") {
        return Some(openai_profile(
            "OpenAI GPT-4",
            "openai_chat",
            "medium",
            "balanced",
            Some(2.0),
            Some(8.0),
            "经典 Chat Completions 端点，兼容面最广。",
        ));
    }
    if id.starts_with("o1") || id.starts_with("o3") || id.starts_with("o4") {
        return Some(openai_profile(
            "OpenAI o-series",
            "openai_responses",
            "high",
            "slow",
            None,
            None,
            "推理系列优先 Responses，保留 reasoning 状态。",
        ));
    }

    if id.contains("claude") {
        let (tier, speed, input, output) = if id.contains("haiku") {
            ("low", "fast", Some(0.10), Some(0.50))
        } else if id.contains("sonnet") {
            ("medium", "balanced", Some(2.0), Some(10.0))
        } else if id.contains("fable") {
            ("high", "balanced", Some(10.0), Some(50.0))
        } else {
            ("high", "balanced", Some(4.0), Some(20.0))
        };
        return Some(ModelInterfaceProfile {
            family: "Anthropic Claude".into(),
            recommended_protocol: "anthropic".into(),
            fallback_protocols: Vec::new(),
            cost_tier: tier.into(),
            speed: speed.into(),
            input_usd_per_mtok: input,
            output_usd_per_mtok: output,
            note: "Claude 原生 Messages API；Haiku 适合低成本批量子任务。".into(),
            source: "https://claude.com/pricing · https://www.aiexpress.news/wiki/llm-api-pricing-2026".into(),
            updated: "2026-10-31".into(),
        });
    }

    if id.contains("gemini") {
        let (tier, speed, input, output, note) = if id.contains("flash") {
            (
                "low",
                "fast",
                Some(0.75),
                Some(3.75),
                "Flash 系列适合高吞吐与低成本路由。",
            )
        } else if id.contains("argon") {
            (
                "medium",
                "balanced",
                Some(2.0),
                Some(10.0),
                "Gemini 4 Argon 首发价为限时档，后续价格以官方页为准。",
            )
        } else {
            (
                "medium",
                "balanced",
                Some(2.0),
                Some(12.0),
                "Pro 系列按提示长度分档计价；generateContent 为原生端点。",
            )
        };
        return Some(ModelInterfaceProfile {
            family: "Google Gemini".into(),
            recommended_protocol: "gemini".into(),
            fallback_protocols: Vec::new(),
            cost_tier: tier.into(),
            speed: speed.into(),
            input_usd_per_mtok: input,
            output_usd_per_mtok: output,
            note: note.into(),
            source: "https://ai.google.dev/gemini-api/docs/pricing".into(),
            updated: "2026-10-31".into(),
        });
    }

    if id.contains("deepseek") {
        let (tier, input, output) = if id.contains("pro") {
            ("low", Some(0.66), Some(1.98))
        } else {
            ("low", Some(0.15), Some(0.60))
        };
        return Some(ModelInterfaceProfile {
            family: "DeepSeek".into(),
            recommended_protocol: "openai_chat".into(),
            fallback_protocols: Vec::new(),
            cost_tier: tier.into(),
            speed: "fast".into(),
            input_usd_per_mtok: input,
            output_usd_per_mtok: output,
            note: "OpenAI 兼容 Chat Completions；价格随峰谷时段变化。".into(),
            source: "https://platform.deepseek.com/pricing · https://devtk.ai/zh/blog/deepseek-api-pricing-guide-2026/".into(),
            updated: "2026-10-31".into(),
        });
    }

    if id.contains("minimax") || id.contains("kimi") || id.contains("qwen") {
        return Some(ModelInterfaceProfile {
            family: "OpenAI-compatible Asia".into(),
            recommended_protocol: "openai_chat".into(),
            fallback_protocols: Vec::new(),
            cost_tier: "low".into(),
            speed: "fast".into(),
            input_usd_per_mtok: None,
            output_usd_per_mtok: None,
            note: "OpenAI 兼容 Chat Completions，适合作为低成本执行路由。".into(),
            source: "https://platform.deepseek.com/pricing · https://help.aliyun.com/zh/model-studio/text-generation".into(),
            updated: "2026-10-31".into(),
        });
    }

    if id.contains("glm") || id.contains("grok") {
        return Some(ModelInterfaceProfile {
            family: if id.contains("grok") { "xAI Grok" } else { "智谱 GLM" }.into(),
            recommended_protocol: "openai_chat".into(),
            fallback_protocols: Vec::new(),
            cost_tier: if id.contains("grok") { "medium" } else { "low" }.into(),
            speed: "balanced".into(),
            input_usd_per_mtok: None,
            output_usd_per_mtok: None,
            note: "OpenAI 兼容 Chat Completions。".into(),
            source: "https://open.bigmodel.cn/ · https://x.ai/".into(),
            updated: "2026-10-31".into(),
        });
    }

    None
}

fn is_openai_protocol(protocol: &str) -> bool {
    matches!(protocol, "openai_chat" | "openai_responses")
}

pub fn protocol_candidates(provider_protocol: &str, model: &str) -> Vec<String> {
    if !is_openai_protocol(provider_protocol) {
        return vec![provider_protocol.to_string()];
    }
    let mut candidates = Vec::new();
    if let Some(profile) = profile(model) {
        for protocol in std::iter::once(profile.recommended_protocol)
            .chain(profile.fallback_protocols)
        {
            if is_openai_protocol(&protocol) && !candidates.contains(&protocol) {
                candidates.push(protocol);
            }
        }
    }
    for protocol in [
        provider_protocol.to_string(),
        "openai_responses".to_string(),
        "openai_chat".to_string(),
    ] {
        if is_openai_protocol(&protocol) && !candidates.contains(&protocol) {
            candidates.push(protocol);
        }
    }
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recommends_protocols_by_model_family() {
        assert_eq!(
            profile("gpt-6-astra").unwrap().recommended_protocol,
            "openai_responses"
        );
        assert_eq!(
            profile("gpt-4.1").unwrap().recommended_protocol,
            "openai_chat"
        );
        assert_eq!(
            profile("claude-opus-5-5").unwrap().recommended_protocol,
            "anthropic"
        );
        assert_eq!(
            profile("gemini-4-argon").unwrap().recommended_protocol,
            "gemini"
        );
    }

    #[test]
    fn restricts_candidates_to_provider_family() {
        let openai = protocol_candidates("openai_chat", "gpt-6-astra");
        assert_eq!(openai.first().map(String::as_str), Some("openai_responses"));
        assert!(openai.contains(&"openai_chat".to_string()));
        assert_eq!(
            protocol_candidates("anthropic", "claude-opus-5-5"),
            vec!["anthropic".to_string()]
        );
        assert_eq!(
            protocol_candidates("ollama", "deepseek-v4-flash"),
            vec!["ollama".to_string()]
        );
    }
}
