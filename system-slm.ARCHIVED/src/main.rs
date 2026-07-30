use std::env;
use reqwest::Client;
use serde_json::{json, Value};
use std::process;

const SLM_SERVER_URL: &str = "http://127.0.0.1:8080/v1/chat/completions";

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!(r#"{{"error": "Missing input. Usage: system-slm '<intent>'"}}"#);
        process::exit(1);
    }

    let user_intent = &args[1];
    
    // Strict Conditional Matrix to prevent 0.5B hallucinations
    let system_prompt = "You are a strict network command translator. \
    If the user asks to check status, ping, or test, output intent: PING. \
    If the user asks to isolate, lockdown, or disconnect, output intent: ISOLATE. \
    If no target is specified, assume target: ALL. \
    Output ONLY valid JSON containing 'intent' and 'target'. No explanations.";

    let payload = json!({
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_intent}
        ],
        "temperature": 0.1,
        "max_tokens": 150
    });

    let client = Client::new();
    match client.post(SLM_SERVER_URL).json(&payload).send().await {
        Ok(res) => {
            if let Ok(json_res) = res.json::<Value>().await {
                if let Some(content) = json_res["choices"][0]["message"]["content"].as_str() {
                    let clean_content = content.replace("```json", "").replace("```", "").trim().to_string();
                    println!("{}", clean_content);
                }
            }
        }
        Err(_) => {
            eprintln!(r#"{{"error": "SLM Server unreachable."}}"#);
            process::exit(1);
        }
    }
}
