use std::error::Error;

use crate::{LLM, Message, auto::try_parse_json, AgentInfo};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct FindingsReport {
    pub findings: Vec<String>,
    pub changes: Vec<String>
}

pub fn create_findings_prompt() -> String {
    format!(
r#"First, create a list of concise points about your findings from the commands.

Then, create a list of long-lasting changes that were executed (i.e. writing to a file, posting a tweet.) Use quotes when discussing specific details.

Keep your findings list very brief.

Respond in this exact format:

```json
{{
    "findings": [
      "A",
      "B"
    ],
    "changes": [
      "A",
      "B"
    ]
}}
```

Ensure your response is fully valid JSON."#)
}

pub fn ask_for_findings(agent: &mut AgentInfo) -> Result<FindingsReport, Box<dyn Error>> {
    agent.llm.message_history.push(Message::User(create_findings_prompt()));

    let report = try_parse_json::<FindingsReport>(&agent.llm, 2, Some(300))?.data;
    for finding in report.findings.iter().chain(report.changes.iter()) {
        agent.observations.store_memory_sync(&agent.llm, finding)?;
    }

    Ok(report)
}

pub fn to_points(points: &[String]) -> String {
    points.iter()                
        .map(|el| format!("- {el}"))
        .collect::<Vec<_>>()
        .join("\n")
}