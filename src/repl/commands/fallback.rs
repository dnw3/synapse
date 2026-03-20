use std::sync::Arc;

use colored::Colorize;
use futures::StreamExt;
use synaptic::core::{ChatModel, ChatRequest, Message};

use super::CommandResult;
use crate::config::SynapseConfig;
use crate::repl::skills::{resolve_skill_slash_command, SkillSlashResult};

pub async fn handle_fallback(
    cmd: &str,
    arg: &str,
    config: &SynapseConfig,
    messages: &mut Vec<Message>,
    model: &Arc<dyn ChatModel>,
) -> CommandResult {
    let cmd_name = &cmd[1..]; // strip leading '/'

    // Check skill slash commands first
    let cwd = std::env::current_dir().unwrap_or_default();
    if let Some(skill_result) = resolve_skill_slash_command(cmd_name, arg, &cwd).await {
        eprintln!("{} Using skill /{}", "skill:".magenta().bold(), cmd_name);
        match skill_result {
            SkillSlashResult::ToolDispatch {
                tool_name,
                arguments,
                arg_mode,
            } => {
                eprintln!(
                    "  {} tool={} args={} mode={}",
                    "dispatch:".yellow().bold(),
                    tool_name,
                    arguments,
                    arg_mode
                );
                let dispatch_msg =
                    format!("Execute tool `{}` with arguments: {}", tool_name, arguments);
                messages.push(Message::human(&dispatch_msg));
            }
            SkillSlashResult::Body(skill_body) => {
                messages.push(Message::human(&skill_body));
            }
        }

        let request = ChatRequest::new(messages.clone());
        let mut stream = model.stream_chat(request);
        let mut full_response = String::new();
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(c) => {
                    print!("{}", c.content);
                    full_response.push_str(&c.content);
                }
                Err(e) => {
                    eprintln!("\n{} {}", "error:".red().bold(), e);
                    break;
                }
            }
        }
        println!();

        messages.push(Message::ai(&full_response));
        return CommandResult::Continue;
    }

    // Check custom commands from config
    if let Some(ref commands) = config.commands {
        if let Some(custom) = commands.iter().find(|c| c.name == cmd_name) {
            let prompt = custom.prompt.replace("{{input}}", arg);
            eprintln!(
                "{} Running custom command /{}",
                "command:".cyan().bold(),
                cmd_name
            );
            messages.push(Message::human(&prompt));

            let request = ChatRequest::new(messages.clone());
            let mut stream = model.stream_chat(request);
            let mut full_response = String::new();
            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(c) => {
                        print!("{}", c.content);
                        full_response.push_str(&c.content);
                    }
                    Err(e) => {
                        eprintln!("\n{} {}", "error:".red().bold(), e);
                        break;
                    }
                }
            }
            println!();

            messages.push(Message::ai(&full_response));
            return CommandResult::Continue;
        }
    }

    eprintln!(
        "{} unknown command '{}'. Type /help for available commands.",
        "warning:".yellow().bold(),
        cmd
    );
    CommandResult::Continue
}
