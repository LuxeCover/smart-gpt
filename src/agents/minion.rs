use std::{error::Error, sync::{Arc, Mutex}};

use colored::Colorize;
use mlua::{Value, Variadic, Lua, Result as LuaResult, FromLua, ToLua, Error as LuaError};
use serde::{Deserialize, Serialize, __private::de};

use crate::{ProgramInfo, generate_commands, Message, Agents, ScriptValue, GPTRunError, Expression, Command, CommandContext, agents::{process_response, LINE_WRAP}};

use super::try_parse;

#[derive(Serialize, Deserialize, Clone)]
pub struct MinionResponse {
    pub findings: Vec<String>,
    pub changes: Vec<String>
}

pub async fn run_command(
    out: &mut String,
    name: String, command: Command, 
    context: &mut CommandContext, args: Vec<ScriptValue>
) -> Result<ScriptValue, Box<dyn Error>> {
    let result = command.run.invoke(context, args.clone()).await?;

    let args: Vec<Expression> = args.iter().map(|el| el.clone().into()).collect();
    let expr = Expression::FunctionCall(name.clone(), args);

    let json = serde_yaml::to_string(&result)
        .map_err(|_| GPTRunError("Could not parse ScriptValue as YAML.".to_string()))?;

    let text = format!("Command {:?} was successful and returned:\n{}", expr, json);
    out.push_str(&text);
    println!("{}", text);

    Ok(result)
}


pub fn run_script(program: &mut ProgramInfo, code: &str) -> Result<String, Box<dyn Error>> {
    let ProgramInfo { 
        context, plugins, .. 
    } = program;

    let lua = Lua::new();

    let out_mutex = Arc::new(Mutex::new(String::new()));

    for plugin in plugins {
        for command in &plugin.commands {
            let name = command.name.clone();
            let command = command.box_clone();
            let lua_context_mutex = context.clone();
            let lua_out_mutex = out_mutex.clone();
            let f = lua.create_function(move |lua, args: Variadic<_>| -> LuaResult<Value> {
                let args: Vec<ScriptValue> = args.iter()
                    .map(|el: &Value| el.clone())
                    .map(|el| ScriptValue::from_lua(el, lua))
                    .flat_map(|el| {
                        if let Ok(el) = el {
                            vec![ el ]
                        } else {
                            vec![]
                        }
                    })
                    .collect();
                
                let name = command.name.clone();
                let mut context = lua_context_mutex.lock().unwrap();
                let mut out= lua_out_mutex.lock().unwrap();
                let rt = tokio::runtime::Runtime::new().unwrap();
                let result = rt.block_on(async {
                    run_command(&mut out, name.clone(), command.box_clone(), &mut context, args).await
                }).map_err(|el| LuaError::RuntimeError(
                    format!("{:?}", el)
                ))?;
                
                let result = result.to_lua(&lua)?;

                Ok(result)
            })?;
            lua.globals().set(name, f)?;
            
        }
    }

    let _ = lua.load(code).exec()?;

    let out = out_mutex.lock().unwrap();
    Ok(out.clone())
}

pub fn run_minion(
    program: &mut ProgramInfo, task: &str, new_prompt: bool
) -> Result<(String, MinionResponse), Box<dyn Error>> {
    let mut last_err: Result<String, Box<dyn Error>> = Ok("".to_string());
    for i in 0..3 {
        let ProgramInfo { 
            context, plugins, personality,
            disabled_commands, .. 
        } = program;
        let mut context = context.lock().unwrap();

        let cmds = generate_commands(plugins, disabled_commands);
    
        if i == 0 {
            context.agents.minion.llm.prompt.clear();
            context.agents.minion.llm.message_history.clear();

            context.agents.minion.llm.prompt.push(Message::System(format!(
        r#"
Using these commands and ONLY these commands:
{}

Write a script to complete this task:
{}

Use the exact commands mentioned in the task.

Keep it as SIMPLE, MINIMAL, and SHORT as possible. IT MUST BE VERY SIMPLE AND SHORT.
Pay very close attention to the TYPE of each command.
Whenever you save a file, use ".txt" for the extension.

Your script will be in the LUA Scripting Language. LUA.
        "#,
                cmds, task
            )));
        }
    
        let script = context.agents.minion.llm.model.get_response_sync(
            &context.agents.minion.llm.get_messages(),
            Some(300),
            Some(0.3)
        )?;
    
        let processed_script = process_response(&script, LINE_WRAP);
    
        println!("{}", "MINION".blue());
        println!("{}", format!("The minion has created a script. Attempt {}", i + 1).white());
        println!();
        println!("{processed_script}");
        println!();
    
        drop(context);
        let out = run_script(program, &script);

        let ProgramInfo { 
            context, ..
        } = program;
        let mut context = context.lock().unwrap();
        
        last_err = match &out {
            Ok(out) => {
                break;
            }
            Err(err) => {
                println!("{:#?}", err);
                context.agents.minion.llm.message_history.push(Message::User(format!(
"Unfortunately, that did not work. 
The error was: {err:#?}\n

Please try again in the exact same format with a fixed LUA script
Respond ONLY with a LUA script. 
You may provide additional commentary on the fixes in code comments.
Ensure your response is exactly valid LUA and can be parsed as valid LUA."
                )));
                out
            }
        };
        
        drop(context);
    }

    match last_err {
        Err(err) => {
            Err(err)
        }
        Ok(result) => {
            let ProgramInfo { 
                context, plugins, personality,
                disabled_commands, .. 
            } = program;
            let mut context = context.lock().unwrap();
    
            context.agents.minion.llm.prompt.clear();
            context.agents.minion.llm.message_history.clear();

            context.agents.minion.llm.message_history.push(Message::System(format!(
r#"First, create a list of concise points about your findings from the commands.

Then, create a list of long-lasting changes that were executed (i.e. writing to a file, posting a tweet.) Use quotes when discussing specific details.

Keep your findings list very brief.

In this format:

```yml
findings:
- A
- B

changes:
- A
- B
```"#
            )));
            context.agents.minion.llm.message_history.push(Message::User(result));
            
            let (response, decision) = try_parse::<MinionResponse>(&context.agents.employee.llm, 3, Some(1000))?;
            context.agents.employee.llm.message_history.push(Message::Assistant(response.clone()));
        
            let findings = decision.findings.iter()
                .map(|el| format!("- {el}"))
                .collect::<Vec<_>>()
                .join("\n");

            let changes = decision.changes.iter()
                .map(|el| format!("- {el}"))
                .collect::<Vec<_>>()
                .join("\n");

            let letter = format!(
"Dear Boss,

I have completed the tasks you assigned to me. These are my findings:
{findings}

These are the changes I had to carry out:
{changes}

Sincerely, Your Employee."
            );

            Ok((letter, decision))
        }
    }
}