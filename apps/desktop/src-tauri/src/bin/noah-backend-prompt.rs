use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let prompt = args
        .next()
        .unwrap_or_else(|| "Help me install and config openclaw".to_string());
    let max_turns = args
        .next()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(8);

    match itman_desktop_lib::debug_runner::run_prompt_flow(&prompt, max_turns).await {
        Ok(result) => {
            println!("SESSION_ID={}", result.session_id);
            for (idx, (input, output)) in result.turns.iter().enumerate() {
                println!("--- TURN {} INPUT ---", idx + 1);
                println!("{}", input);
                println!("--- TURN {} OUTPUT ---", idx + 1);
                println!("{}", output);
            }
            println!("REACHED_DONE={}", result.reached_done);
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("ERROR: {}", err);
            ExitCode::from(1)
        }
    }
}
